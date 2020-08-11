use std::sync::Arc;
use tokio::sync::Notify;
use tokio::net::TcpListener;
use std::time::Duration;
use anyhow::Result;
use log::info;

#[derive(Clone)]
pub struct StammerConfig {
    pub bind_addr: String,
    pub session_timeout: Duration,
}

pub async fn run_stammer_task(
    stammer_cfg: StammerConfig,
    listener: TcpListener,
    stop: Arc<Notify>,
) {
    info!("starting stammer...");

    // control/routing task communications, read on for more context
    use tokio::sync::mpsc::unbounded_channel;
    let (control_sender, control_recver) = unbounded_channel();
    let (routing_sender, routing_recver) = unbounded_channel();

    // the control task owns the routing table (connected sessions, room memberships, ...) it
    // responds to multiple kinds of events (see ControlMessage). it sends/receives all
    // control packets received/sent by individual session tasks. for example, upon room
    // membership changes, it will:
    //
    //  1. modify routing table upon receiving control packets by origin session
    //  2. send updated routing table to routing task for routing routing purposes
    //  3. declare new membership to all other sessions by sending them control packets
    use task_control::run_control_task;
    let control_fut = run_control_task(control_recver, routing_sender.clone());

    // the routing task routes voice packets from one source to N destinations
    // using a view of the world regularly updated by the control task
    use task_routing::run_routing_task;
    let routing_fut = run_routing_task(routing_recver);

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
    //  4. forward any tunneled routing packets to the routing task
    //  5. forward any client-bound control packets to the client
    //
    // if they encounter an error, session tasks will deregister from
    // the control task themselves.
    use task_accept::run_accept_task;
    let accept_fut = run_accept_task(stammer_cfg, stop, listener, control_sender, routing_sender);

    // server will run until caller notifies stop
    use tokio::join;
    join!(control_fut, routing_fut, accept_fut);
    info!("stammer has stopped");
}

impl StammerConfig {
    pub fn from_env() -> Result<Self> {
        use std::env::var;
        let session_timeout = var("STAMMER_SESSION_TIMEOUT_SECS").unwrap_or("30".to_owned());
        Ok(Self {
            bind_addr: var("STAMMER_BIND_ADDR").unwrap_or("localhost:8792".to_owned()),
            session_timeout: Duration::from_secs(session_timeout.parse::<u64>()?),
        })
    }
}

mod task_accept;
mod task_control;
mod task_routing;
mod task_session;
mod routing_table;
