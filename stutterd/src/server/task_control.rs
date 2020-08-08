use super::routing_table::RoutingTable;
use tokio::sync::mpsc::{
    UnboundedSender as USender,
    UnboundedReceiver as UReceiver,
};
use std::collections::HashMap;
use mumble_protocol::{
    control::{ControlPacket,msgs},
    voice::{Serverbound,Clientbound},
};
use log::{warn,info,debug};

#[derive(Debug)]
pub enum ControlMessage {
    Packet(u32, ControlPacket<Serverbound>),
    AddSession(u32, UnAuthSession),
    RemoveSession(u32),

    Shutdown,
}

// incoming sessions sent by the accept task to the control task. those are not
// authenticated yet. once they are, they will be enrolled in the routing table
#[derive(Debug)]
pub struct UnAuthSession {
    pub version: msgs::Version,
    pub send: USender<ControlPacket<Clientbound>>,
}

use super::task_media::MediaMessage;
pub async fn run_control_task(
    mut control_recv: UReceiver<ControlMessage>,
    mut media_send: USender<MediaMessage>,
) {
    // where sessions are stored before they authenticate
    let mut unauth: HashMap<u32, UnAuthSession> = HashMap::new();
    // once authenticated, sessions are routable
    let mut rtbl = RoutingTable::default();

    use tokio::stream::StreamExt;
    while let Some(msg) = control_recv.next().await {
        match msg {
            // sent by session tasks upon receiving a control packet from client
            ControlMessage::Packet(id, packet) => {
                let res = handle_packet(id, packet, &mut unauth, &mut media_send, &mut rtbl);
                if let Err(err) = res {
                    warn!("packet handling: {}", err);
                }
            },

            // sent by session tasks after proper version handshake
            ControlMessage::AddSession(session_id, unauth_session) => {
                unauth.insert(session_id, unauth_session);
            },

            // sent by session tasks whenever they die ungracefully
            ControlMessage::RemoveSession(session_id) => {
                if unauth.remove(&session_id).is_none() {
                    if let Err(err) = rtbl.expel_session(session_id) {
                        warn!("failed to expel session {}: {}", session_id, err);
                    } else {
                        info!("expelled session {} from routing table", session_id);
                        // forward routing table to media task
                        let msg = MediaMessage::RoutingChange(rtbl.clone());
                        media_send.send(msg).expect("media cannot be closed yet");
                    }
                }
            },

            // sent by the accept task in case of graceful shutdown
            ControlMessage::Shutdown => {
                info!("stopping control task: draining all remaining messages");
                control_recv.close();
            },
        }
    }

    info!("sending shutdown message to media task");
    media_send.send(MediaMessage::Shutdown).expect("media cannot be closed yet");

    info!("control task stopped")
}

use anyhow::{Error,Result};
fn handle_packet(
    session_id: u32,
    packet: ControlPacket<Serverbound>,
    unauth: &mut HashMap<u32, UnAuthSession>,
    media_send: &mut USender<MediaMessage>,
    rtbl: &mut RoutingTable,
) -> Result<()> {
    if rtbl.holds_session(session_id) {
        Ok(())
    } else if let Some(unauth_session) = unauth.remove(&session_id) {
        if let ControlPacket::Authenticate(_auth) = packet {
            // FIXME we ultimately need a registry of saved
            // users/rooms to auth this session against
            info!("session {} authenticated itself", session_id);

            // modify control task routing table
            rtbl.enroll_session(session_id, unauth_session.version, unauth_session.send);
            debug!("control task updated its routing table");

            // propagate routing table change to media task
            let msg = MediaMessage::RoutingChange(rtbl.clone());
            media_send.send(msg).expect("channel closes only upon later shutdown msg");

            // TODO send all the cryptsetup/channel states/user states/server sync to complete
            // https://mumble-protocol.readthedocs.io/en/latest/establishing_connection.html#
            Ok(())
        } else {
            Err(Error::msg(format!("unauth session {} sent bad packet {:?}", session_id, packet)))
        }
    } else {
        Err(Error::msg(format!("unknown session {} sent packet {:?}", session_id, packet)))
    }
}
