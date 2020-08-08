use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Notify;
use log::{LevelFilter,warn,error,info};

#[tokio::main]
async fn main() {
    if let Err(err) = setup_logging() {
        eprintln!("failed to setup logging: {}", err);
    } else if let Err(err) = run_slave().await {
        error!("error while running slave: {}", err);
    }
}

async fn run_slave() -> Result<()> {
    // load the slave config and bind on tcp
    use libstammer::SlaveConfig;
    use tokio::net::TcpListener;
    let slave_cfg = SlaveConfig::from_env()?;
    let listener = TcpListener::bind(&slave_cfg.bind_addr).await?;

    // enable stopping the slave using ctrl-c
    let stop = Arc::new(Notify::new());
    let cancel_fut = handle_ctrl_c(stop.clone());

    // kickstart the slave task
    use tokio::join;
    use libstammer::run_slave_task;
    join!(cancel_fut, run_slave_task(slave_cfg, listener, stop));

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

fn setup_logging() -> Result<()> {
    use fern::Dispatch;
    Dispatch::new().level(LevelFilter::Debug).apply()
}
