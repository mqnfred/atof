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

    use stammer::ClientConfig;
    use std::time::Duration;
    let client_cfg = ClientConfig{
        connect_addr: "localhost:8792".to_owned(),
        session_timeout: Duration::from_secs(30),
    };

    use tokio::net::TcpStream;
    use tokio_util::codec::Framed;
    let server_stream = TcpStream::connect(&client_cfg.connect_addr).await.unwrap(); // TODO
    let server_stream = Framed::new(server_stream, ClientControlCodec::new());

    use tokio::join;
    use task_media::run_media_task;
    use stammer::run_client_task;
    join!(
        run_media_task(client_sender, media_recver),
        run_client_task(client_cfg, server_stream, client_recver, media_sender, ui_sender),
    );
}

mod task_media;

async fn setup_logging() -> Result<()> {
    use std::io::stderr;
    use fern::Dispatch;
    use log::LevelFilter;
    use log::info;
    Dispatch::new().level(LevelFilter::Trace).chain(stderr()).apply()?;
    info!("logging to stderr setup successfully");
    Ok(())
}
