use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Notify;

pub async fn serve(bind_addr: &str, stop: Arc<Notify>) -> Result<()> {
    // control/media task communications, read on for more context
    use tokio::sync::mpsc::unbounded_channel;
    let (control_send, control_recv) = unbounded_channel();
    let (media_send, media_recv) = unbounded_channel();

    // bind to the provided address
    use tokio::net::TcpListener;
    let listener = TcpListener::bind(bind_addr).await?;
    eprintln!("bound to {}", bind_addr);

    // the control task owns the routing table (connected sessions, room memberships, ...) it
    // responds to multiple kinds of events (see ControlMessage). it sends/receives all
    // control packets received/sent by individual session tasks. for example, upon room
    // membership changes, it will:
    //
    //  1. modify routing table upon receiving control packets by origin session
    //  2. send updated routing table to media task for voice/media routing purposes
    //  3. declare new membership to all other sessions by sending them control packets
    use task_control::run_control_task;
    let control_fut = run_control_task(control_recv, media_send.clone());

    // the media task redirects voice/media packets from one source to N destinations
    // using a view of the world regularly updated by the control task
    use task_media::run_media_task;
    let media_fut = run_media_task(media_recv);

    // this task accepts new tcp connections and:
    //
    //  1. assign them a unique session_id
    //  2. kickstart the session task
    //  3. shuts down the control task if it receives a stop notification
    //  4. in case of a shutdown, wait for all session tasks to stop
    //
    // there is one session task per tcp connection. they terminate if
    // the connection terminates. they:
    //
    //  1. perform version handshake with the client
    //  2. declare themselves to the control task
    //  3. forward any server-bound control packets to the control task
    //  4. forward any tunneled media packets to the media task
    //  5. forward any client-bound control packets to the client
    //
    // if they encounter an error, session tasks will deregister from
    // the control task themselves.
    use task_accept::run_accept_task;
    let accept_fut = run_accept_task(stop, listener, control_send, media_send);

    // server will run until caller notifies stop
    use tokio::join;
    join!(control_fut, media_fut, accept_fut);

    Ok(())
}

mod task_accept;
mod task_control;
mod task_media;
mod task_session;

mod routing_table;
