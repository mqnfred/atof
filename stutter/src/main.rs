use anyhow::Result;
use log::error;

#[tokio::main]
async fn main() {
    if let Err(err) = setup_logging().await {
        eprintln!("failed to setup logging: {}", err);
    } else if let Err(err) = run_stutter().await {
        error!("error while running stutter: {}", err);
    }
}

async fn run_stutter() -> Result<()> {
    use stutter::StutterConfig;
    let stutter_cfg = StutterConfig::from_env()?;

    use tokio::net::TcpStream;
    let server_stream = TcpStream::connect(&stutter_cfg.addr).await?;

    use stutter::run_stutter_task;
    run_stutter_task(stutter_cfg, server_stream).await;

    Ok(())
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
