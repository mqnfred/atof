use tokio::net::TcpStream;
use std::time::Duration;
use anyhow::Result;

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

    let server_stream = Framed::new(server_stream, ClientControlCodec::new());

    info!("stutter has stopped");
}

mod task_audio;
mod task_audio_decoder;
mod task_audio_muxer;
mod task_audio_io;

impl StammerConfig {
    pub fn from_env() -> Result<Self> {
        use std::env::var;
        let session_timeout = var("STUTTER_SESSION_TIMEOUT_SECS").unwrap_or("30".to_owned());
        Ok(Self {
            addr: var("STUTTER_ADDR").unwrap_or("localhost:8792".to_owned()),
            session_timeout: Duration::from_secs(session_timeout.parse::<u64>()?),
        })
    }
}
