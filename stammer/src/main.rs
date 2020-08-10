use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Notify;
use log::{warn,error,info};

#[tokio::main]
async fn main() {
    if let Err(err) = setup_logging().await {
        eprintln!("failed to setup logging: {}", err);
    } else if let Err(err) = run_stammer().await {
        error!("error while running stammer: {}", err);
    }
}

async fn run_stammer() -> Result<()> {
    // load the stammer config and bind on tcp
    use stammer::StammerConfig;
    use tokio::net::TcpListener;
    let stammer_cfg = StammerConfig::from_env()?;
    let listener = TcpListener::bind(&stammer_cfg.bind_addr).await?;

    // enable stopping stammer using ctrl-c
    let stop = Arc::new(Notify::new());
    let cancel_fut = handle_ctrl_c(stop.clone());

    // kickstart the stammer task
    use tokio::join;
    use stammer::run_stammer_task;
    join!(cancel_fut, run_stammer_task(stammer_cfg, listener, stop));

    Ok(())
}

async fn handle_ctrl_c(stop: Arc<Notify>) {
    use tokio::signal::ctrl_c;
    if let Err(err) = ctrl_c().await {
        warn!("failed to listen to ctrl-c signal, no graceful shutdown for us: {}", err);
    } else {
        info!("received ctrl-c signal, initiating graceful shutdown...");
        stop.notify();
    }
}

async fn setup_logging() -> Result<()> {
    use std::io::stderr;
    use fern::Dispatch;
    use log::LevelFilter;
    Dispatch::new().level(LevelFilter::Trace).chain(stderr()).apply()?;
    info!("logging to stderr setup successfully");
    Ok(())
}
