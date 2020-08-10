use anyhow::Result;
use mumble_protocol::control::{ClientControlCodec,ControlPacket,msgs};
use mumble_protocol::voice::{VoicePacket,Clientbound,Serverbound};
use tokio::sync::mpsc::{
    UnboundedReceiver as UReceiver,
    UnboundedSender as USender,
};

#[tokio::main]
async fn main() {
    setup_logging().await.unwrap();
    use tokio::sync::mpsc::unbounded_channel;
    let (client_sender, client_recver) = unbounded_channel();
    let (media_sender, media_recver) = unbounded_channel();
    let (ui_sender, ui_recver) = unbounded_channel();

    use std::time::Duration;
    use stutter::StutterConfig;
    let client_cfg = StutterConfig{
        addr: "localhost:8792".to_owned(),
        session_timeout: Duration::from_secs(30),
    };

    use tokio::net::TcpStream;
    use tokio_util::codec::Framed;
    let server_stream = TcpStream::connect(&stutter_cfg.addr).await.unwrap(); // TODO

    use stutter::run_stutter_task;
    run_stutter_task(stutter_cfg, server_stream)
}

async fn setup_logging() -> Result<()> {
    use std::io::stderr;
    use fern::Dispatch;
    use log::LevelFilter;
    use log::info;
    Dispatch::new().level(LevelFilter::Trace).chain(stderr()).apply()?;
    info!("logging to stderr setup successfully");
    Ok(())
}
