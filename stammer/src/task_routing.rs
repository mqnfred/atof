use super::routing_table::RoutingTable;
use tokio::sync::mpsc::UnboundedReceiver as UReceiver;
use mumble_protocol::voice::{VoicePacket,Serverbound};
use mumble_protocol::control::{
    ControlPacket,
    msgs::TextMessage,
};
use log::{warn,trace,debug};

#[derive(Debug)]
pub enum RoutingMessage {
    Voice(u32, Box<VoicePacket<Serverbound>>),
    Text(u32, Box<TextMessage>),

    Update(RoutingTable),
    Shutdown,
}

pub async fn run_routing_task(mut routing_recv: UReceiver<RoutingMessage>) {
    trace!("routing task started");
    let mut routing_table = RoutingTable::default();

    use tokio::stream::StreamExt;
    while let Some(msg) = routing_recv.next().await {
        match msg {
            // voice messages sent by session tasks
            RoutingMessage::Voice(session_id, voice_packet) => match *voice_packet {
                // an audio packet from a session we need to route to the right peers
                VoicePacket::Audio{target, seq_num, payload, position_info, ..} => {
                    // yield all senders for this 
                    let peer_senders = match routing_table.target_senders(session_id, target) {
                        Err(err) => { warn!("failed to route voice packet: {}", err); continue },
                        Ok(peer_senders) => peer_senders,
                    };

                    // reconstruct the voice packet, this time clientbound
                    use mumble_protocol::voice::Clientbound;
                    let voice_packet = Box::new(VoicePacket::Audio{
                        _dst: std::marker::PhantomData::<Clientbound>,
                        target,
                        session_id,
                        seq_num,
                        payload,
                        position_info,
                    });

                    for peer_sender in peer_senders {
                        // an error might arise in case the destination session is in the
                        // process of being dropped (for whatever reason). we just skip it then
                        let _ = peer_sender.send(ControlPacket::UDPTunnel(voice_packet.clone()));
                    }
                },

                // a session and its connection are trying to ping us. in response, we crash!
                VoicePacket::Ping{..} => unimplemented!("audio ping not supported yet"),
            },

            // text message sent by session tasks
            RoutingMessage::Text(session_id, mut text_message) => {
                text_message.set_actor(session_id); // keep client from spoofing

                // senders to all recipients of the message. any references to sessions and
                // rooms that have since then disappeared are just dropped silently
                let peer_senders = text_message.get_session().iter().filter_map(|session_id| {
                    routing_table.sender(*session_id)
                }).chain(
                    text_message.get_channel_id().iter().map(|room_id| {
                        routing_table.room_senders(*room_id, None)
                    }).flatten()
                );
                // TODO here we are ignoring the text_message tree_ids (root rooms) recipients
                // because our rooms are not organized/stored as a tree, and we happen to be lazy

                for peer_sender in peer_senders {
                    // an error might arise in case the destination session is in the
                    // process of being dropped (for whatever reason). we just skip it then
                    let _ = peer_sender.send(ControlPacket::TextMessage(text_message.clone()));
                }
            },

            // sent by the control task in case of routing table change
            RoutingMessage::Update(rtbl) => {
                debug!("routing task updated its routing table");
                routing_table = rtbl;
            },

            // sent by the control task in case of a graceful shutdown
            RoutingMessage::Shutdown => {
                trace!("stopping routing task: draining all remaining messages");
                routing_recv.close();
            },
        }
    }

    trace!("routing task stopped")
}
