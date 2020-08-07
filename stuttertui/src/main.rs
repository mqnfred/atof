use mumble_protocol::control::{ClientControlCodec,ControlPacket};
use mumble_protocol::voice::{Clientbound,Serverbound};
use tokio::sync::mpsc::{
    UnboundedReceiver as UReceiver,
    UnboundedSender as USender,
};

fn main() {
    let addr = "localhost:5000";

    // establish ui<->connection inter-thread communication
    use tokio::sync::mpsc::unbounded_channel;
    let (connect_sender, connect_recver) = unbounded_channel();
    let (ui_sender, ui_recver) = unbounded_channel();

    use std::thread::spawn;
    let _ = spawn(move || { connect_thread(addr, connect_recver, ui_sender) }).join();
    let _ = spawn(move || { ui_thread(ui_recver, connect_sender) }).join();
}

fn ui_thread(
    _ui_recver: UReceiver<ControlPacket<Clientbound>>,
    _connect_sender: USender<ControlPacket<Serverbound>>,
) {
    
}

fn connect_thread(
    addr: &str,
    connect_recver: UReceiver<ControlPacket<Serverbound>>, // from caller to server
    ui_sender: USender<ControlPacket<Clientbound>>, // from server to caller
) {
    // we need to spawn a tokio runtime as the connect task is asynchronous
    use tokio::runtime::Runtime;
    let mut rt = match Runtime::new() {
        Err(err) => { eprintln!("failed to start runtime: {}", err); return },
        Ok(rt) => rt,
    };

    // connect to the server and spin up our connect task
    rt.block_on(async move {
        // connect to server and wrap connection in mumble control codec
        use tokio::net::TcpStream;
        use tokio_util::codec::Framed;
        let server_stream = match TcpStream::connect(&addr).await {
            Ok(server_stream) => Framed::new(server_stream, ClientControlCodec::new()),
            Err(err) => { eprintln!("failed to establish connection: {}", err); return },
        };
        eprintln!("established connection to server");

        // the connect_task babysits the connection, it:
        //
        //  1. sends and receives server pings
        //  2. forwards any server-bound packets to the server (from the ui thread)
        //  3. forwards any client-bound packets to the ui thread (from the caller)
        use task_connect::run_connect_task;
        if let Err(err) = run_connect_task(server_stream, connect_recver, ui_sender).await {
            eprintln!("connection error: {}", err);
        }
    });
}

mod task_connect;
