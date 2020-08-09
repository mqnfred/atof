use std::sync::Arc;
use super::task_control::ControlMessage;
use super::task_routing::RoutingMessage;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::sync::mpsc::UnboundedSender as USender;
use tokio_util::codec::Framed;
use mumble_protocol::control::ServerControlCodec;
use log::{trace,info,error};
use super::SlaveConfig;

pub async fn run_accept_task(
    slave_cfg: SlaveConfig, // the global config of the slave task
    stop: Arc<Notify>, // listened on for stop signal (ctrl-c)
    mut listener: TcpListener, // listened on for new tcp streams
    control_send: USender<ControlMessage>, // hand to session tasks + notify about new sessions
    routing_send: USender<RoutingMessage>, // hand to session tasks
) {
    trace!("started accept task...");
    // list of session tasks for future join
    // FIXME will keep growing with sessions. unclear how to purge closed sessions atm
    let mut sessions = vec![];

    loop {
        use tokio::select;
        use tokio::stream::StreamExt;
        select! {
            // if any other task fails, program shutdown
            _ = stop.notified() => break,
            // triggered whenever a client connects
            tcp_stream = listener.next() => match tcp_stream {
                None => unreachable!("bound listener stream never ends"),
                Some(Err(err)) => {
                    // TODO: should we abort here or try again? are errors
                    // terminal or transient? assuming terminal for now
                    error!("encountered error while listening for new connections: {}", err);
                    break
                },
                Some(Ok(tcp_stream)) => {
                    // select unique id for the new session
                    let session_id = sessions.len() as u32;
                    info!("received new connection, assigning session id {}", session_id);

                    // wrap tcp stream in a mumble protocol framed codec
                    let codec_stream = Framed::new(tcp_stream, ServerControlCodec::new());

                    // kickoff the session task
                    use tokio::spawn;
                    use super::task_session::run_session_task;
                    sessions.push(spawn(run_session_task(
                        slave_cfg.clone(),
                        session_id, // identify session when sending to control/routing
                        codec_stream, // codec-ed connection to the client
                        control_send.clone(), // any control packets send there
                        routing_send.clone(), // voice packets will be sent there
                    )));
                },
            },
        }
    }

    trace!("sending shutdown message to control task");
    control_send.send(ControlMessage::Shutdown).expect("control cannot be closed yet");

    if sessions.is_empty() {
        trace!("no session tasks to wait on (#SAD!)");
    } else {
        trace!("waiting for all {} session tasks to stop...", sessions.len());
        use futures::future::join_all;
        join_all(sessions).await;
    }

    trace!("accept task stopped")
}
