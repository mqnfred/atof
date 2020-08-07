use std::sync::Arc;
use tokio::sync::Notify;

#[tokio::main]
async fn main() {
    use std::env::args;
    let bind_addr = args().skip(1).next().expect("please provide addr:port to bind to");

    let stop = Arc::new(Notify::new());
    let server_fut = run_server(&bind_addr, stop.clone());
    let cancel_fut = handle_ctrl_c(stop.clone()); // server will run until sby hits ctrl-c

    use tokio::join;
    join!(server_fut, cancel_fut);
}

async fn run_server(bind_addr: &str, stop: Arc<Notify>) {
    use server::serve;
    if let Err(err) = serve(bind_addr, stop).await {
        eprintln!("server stopped with error: {}", err)
    } else {
        eprintln!("graceful shutdown complete")
    }
}

async fn handle_ctrl_c(stop: Arc<Notify>) {
    use tokio::signal::ctrl_c;
    if let Err(err) = ctrl_c().await {
        eprintln!("failed to listen to ctrl-c signal, no graceful shutdown: {}", err);
    } else {
        eprintln!("received ctrl-c signal, initiating graceful shutdown...");
        stop.notify();
    }
}

mod server;
