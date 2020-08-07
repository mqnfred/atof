use anyhow::{Error,Result};
use mumble_protocol::control::ControlPacket;
use mumble_protocol::control::ServerControlCodec;
use super::task_control::ControlMessage;
use super::task_media::MediaMessage;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tokio::sync::mpsc::{
    UnboundedSender as USender,
};
use futures::sink::SinkExt;
use futures::stream::StreamExt;

pub async fn run_session_task(
    session_id: u32,
    mut client_stream: Framed<TcpStream, ServerControlCodec>,
    control_send: USender<ControlMessage>,
    media_send: USender<MediaMessage>,
) {
    // with the client, which is the first step in the session handshaking process, see:
    // https://mumble-protocol.readthedocs.io/en/latest/establishing_connection.html
    // TODO: join client/server implems and reuse version handshake as a lib exchange versions
    let mut server_version = msgs::Version::new();
    server_version.set_version(1u32 << 16 | 2u32 << 8 | 4u32); // TODO check
    let version = match version_exchange(server_version, &mut client_stream).await {
        Ok(version) => version,
        Err(err) => {
            eprintln!("version exchange for {} failed: {}", session_id, err);
            return
        },
    };

    // setup session input/output and session handler
    use tokio::sync::mpsc::unbounded_channel;
    let (session_send, mut session_recv) = unbounded_channel();

    // register ourselves to the control task. this will not make this
    // session routable, for that we still need the authenticate packet from
    // the client. it will be handled by the control task at a later time.
    use super::task_control::UnAuthSession;
    let msg = ControlMessage::AddSession(session_id, UnAuthSession{version, send: session_send});
    if control_send.send(msg).is_err() { // control task is closed, graceful shutdown in progress
        eprintln!("session task {} stopped (graceful shutdown in progress)", session_id);
        return
    }
    eprintln!("session {} declared itself", session_id);

    // set up session timeout at 30s if no ping
    use std::time::{Duration,Instant};
    let mut last_ping = Instant::now();
    let timeout = Duration::from_secs(30);
    use tokio::time::interval;
    let mut keepalive_check = interval(timeout);

    loop {
        use tokio::select;
        select! {
            // listen to control packets from client connection
            packet = client_stream.next() => {
                let packet = match packet { // sanitize packet
                    Some(Ok(packet)) => packet,

                    // io error, for now we consider them terminal (TODO refine)
                    Some(Err(err)) => {
                        eprintln!("session {}: {}", session_id, err);
                        // might fail if control task is closed (a graceful shutdown
                        // is in progress), in which case nobody cares about our session
                        let _ = control_send.send(ControlMessage::RemoveSession(session_id));
                        break
                    },

                    // the connection with the client got closed
                    None => {
                        eprintln!("session {}: connection closed", session_id);
                        // might fail if control task is closed (a graceful shutdown
                        // is in progress), in which case nobody cares about our session
                        let _ = control_send.send(ControlMessage::RemoveSession(session_id));
                        break
                    },
                };

                match packet {
                    // tunneled voice media packet, forward to media task
                    ControlPacket::UDPTunnel(voice_packet) => {
                        // might fail if media task is closed (a graceful shutdown
                        // is in progress), in which case we just drop any packets
                        let _ = media_send.send(MediaMessage::Voice(session_id, voice_packet));
                    },

                    // text messages are handled by the media task too
                    ControlPacket::TextMessage(text_message) => {
                        // might fail if media task is closed (a graceful shutdown
                        // is in progress), in which case we just drop any packets
                        let _ = media_send.send(MediaMessage::Text(session_id, text_message));
                    },

                    // ping packet, just return it directly
                    // TODO ping packets should return much more data
                    // TODO should this packet be handled by media task? that would
                    // enable the client to see full voice packet roundtrip picture?
                    ControlPacket::Ping(ts) => {
                        // register the ping
                        last_ping = Instant::now();

                        // answer with a pong
                        let packet = ControlPacket::Ping(ts).into();
                        if let Err(err) = client_stream.send(packet).await {
                            // io error, for now we consider them terminal (TODO refine)
                            eprintln!("session {}: {}", session_id, err);

                            // might fail if control task is closed (a graceful shutdown
                            // is in progress), in which case nobody cares about our session
                            let _ = control_send.send(ControlMessage::RemoveSession(session_id));
                            break
                        }
                    },

                    // normal control packet, forward to the control task
                    packet => {
                        // might fail if control task is closed (a graceful shutdown
                        // is in progress), in which case we just drop any packets
                        let _ = control_send.send(ControlMessage::Packet(session_id, packet));
                    },
                }
            },

            // listen to control packets from the control and media tasks
            packet = session_recv.next() => {
                let packet = match packet { // sanitize packet
                    Some(packet) => packet,

                    // this happens when both control/media tasks stop and
                    // drop their senders, this is a graceful shutdown event
                    None => { eprintln!("session task {} stops gracefully", session_id); return },
                };

                // handling of client-bound packet is simple: we just forward it
                if let Err(err) = client_stream.send(packet).await {
                    // io error, for now we consider them terminal (TODO refine)
                    eprintln!("session {} abort: {}", session_id, err);

                    // might fail if control task is closed (a graceful shutdown
                    // is in progress), in which case nobody cares about our session
                    let _ = control_send.send(ControlMessage::RemoveSession(session_id));
                    break
                }
            },

            // check that we recently got a ping every 30s, otherwise drop
            _ = keepalive_check.next() => {
                let since_last = last_ping.elapsed();
                if since_last > timeout {
                    // TODO we might want to send an error/whatever packet to the client here
                    eprintln!("session {} timed out ({:?} since ping)", session_id, since_last);

                    // might fail if control task is closed (a graceful shutdown
                    // is in progress), in which case nobody cares about our session
                    let _ = control_send.send(ControlMessage::RemoveSession(session_id));
                    break
                }
            },
        }
    }

    eprintln!("session task {} stopped", session_id);
}

use mumble_protocol::control::msgs;
async fn version_exchange(
    server_version: msgs::Version,
    client_stream: &mut Framed<TcpStream, ServerControlCodec>,
) -> Result<msgs::Version> {
    // send the server version to the client
    client_stream.send(server_version.into()).await?;

    // retrieve client version
    match client_stream.next().await.ok_or_else(|| Error::msg("client closed the connection"))? {
        Ok(version_packet) => match version_packet {
            ControlPacket::Version(version) => Ok(*version),
            packet => Err(Error::msg(format!("expected version packet, received: {:?}", packet))),
        },
        Err(err) => Err(err.into()),
    }
}
