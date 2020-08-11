use tokio::net::TcpStream;
use std::time::Duration;
use anyhow::Result;
use log::{error,trace,info,debug};
use tokio::sync::mpsc::{
    UnboundedReceiver as UReceiver,
    UnboundedSender as USender,
};

#[derive(Clone)]
pub struct StutterConfig {
    pub addr: String,
    pub session_timeout: Duration,
}

pub async fn run_stutter_task(
    stutter_cfg: StutterConfig,
    server_stream: TcpStream,
) {
    info!("starting stutter...");

    // wrap the tcp stream we were provided in the mumble protocol
    use tokio_util::codec::Framed;
    use mumble_protocol::control::ClientControlCodec;
    let server_stream = Framed::new(server_stream, ClientControlCodec::new());

    // connection/audio task communications, read on for more context
    use tokio::sync::mpsc::unbounded_channel;
    let (connection_sender, connection_recver) = unbounded_channel();
    let (audio_codec_sender, audio_codec_recver) = unbounded_channel();
    let (audio_io_sender, audio_io_recver) = unbounded_channel();

    // the connection task is responsible for handling the connection to stammer.
    //
    // it will redirect any messages received on connection_recver to the stammer server.
    // upon receiving a message from the server_stream, it will
    //
    //  1. if voice message, send to the audio codec task
    //  TODO incomplete doc
    use task_connection::run_connection_task;
    let connection_fut = run_connection_task(
        stutter_cfg.clone(),
        server_stream,
        connection_recver,
        audio_codec_sender.clone(),
    );

    // indicate to the audio io task, ahead of it starting, that we want
    // both the microphone and speakers sampling from the beginning.
    use task_audio_io::AudioIOMessage;
    let _ = audio_io_sender.send(AudioIOMessage::RecordingPlay);
    let _ = audio_io_sender.send(AudioIOMessage::PlaybackPlay);

    // the audio codec task acts as a gatekeeper between the connection
    // task and the audio io task. it decodes client-bound packets from opus
    // to pcm, and reverse for server-bound packets.
    use task_audio_codec::run_audio_codec_task;
    let audio_codec_fut = run_audio_codec_task(
        audio_codec_recver,
        connection_sender,
        audio_io_sender,
    );

    // the audio io task drives the cpal library callbacks, which read/write
    // to and from the audio drivers' buffers. it sends and receives raw pcm
    // streams to and from the audio codec task.
    use task_audio_io::run_audio_io_task;
    let audio_io_fut = run_audio_io_task(
        audio_io_recver,
        audio_codec_sender,
    );

    use tokio::join;
    join!(
        connection_fut,
        audio_codec_fut,
        audio_io_fut,
    );

    info!("stutter has stopped");
}

mod task_audio_codec;
mod task_audio_io;
mod task_connection;

impl StutterConfig {
    pub fn from_env() -> Result<Self> {
        use std::env::var;
        let session_timeout = var("STUTTER_SESSION_TIMEOUT_SECS").unwrap_or("30".to_owned());
        Ok(Self {
            addr: var("STUTTER_ADDR").unwrap_or("localhost:8792".to_owned()),
            session_timeout: Duration::from_secs(session_timeout.parse::<u64>()?),
        })
    }
}
