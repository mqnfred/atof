use anyhow::Result;
use mumble_protocol::control::{ClientControlCodec,ControlPacket,msgs};
use mumble_protocol::voice::{VoicePacket,Serverbound};
use tokio::sync::mpsc::{
    UnboundedReceiver as UReceiver,
    UnboundedSender as USender,
};
use tokio_util::codec::Framed;
use tokio::net::TcpStream;
use log::{error,trace,info,debug};

pub enum ConnectionMessage {
    Voice(VoicePacket<Serverbound>),
    Control(ControlPacket<Serverbound>),
}

use super::StutterConfig;
use super::task_audio_codec::AudioCodecMessage;

pub async fn run_connection_task(
    stutter_cfg: StutterConfig,
    mut server_stream: Framed<TcpStream, ClientControlCodec>,
    mut connection_recver: UReceiver<ConnectionMessage>,
    audio_codec_sender: USender<AudioCodecMessage>,
) -> Result<()> {
    // handshake with the server (version exchange, authentication...)
    handshake(&mut server_stream).await?;
    info!("server handshake successful");

    // set up ping interval at half the session timeout
    use tokio::time::interval;
    let ping_interval = stutter_cfg.session_timeout / 2;
    let mut keepalive = interval(ping_interval);

    loop {
        use tokio::select;
        use futures::sink::SinkExt;
        use tokio::stream::StreamExt;
        select! {
            // reminder to send a ping to the server
            _ = keepalive.next() => ping(&mut server_stream).await?,

            // messages from audio codec or ui tasks are handled here
            connection_msg = connection_recver.next() => match connection_msg {
                // audio codec and ui tasks are goners, we are done here
                None => { trace!("other tasks are shutting down, gracefully stopping"); break },

                // received a voice message from the audio codec task
                Some(ConnectionMessage::Voice(voice_packet)) => {
                    let msg = ControlPacket::UDPTunnel(Box::new(voice_packet));
                    // we consider io errors terminal at this time (TODO refine)
                    if let Err(err) = server_stream.send(msg.into()).await {
                        error!("encountered connection error: {}", err);
                        break
                    }
                },

                // control message from the ui, we forward this to the server right away
                Some(ConnectionMessage::Control(packet)) => {
                    // we consider io errors terminal at this time (TODO refine)
                    server_stream.send(packet.into()).await?;
                },
            },

            server_msg = server_stream.next() => match server_msg {
                // the server closed the connection
                None => { error!("connection closed by server, stopping"); break },

                // we consider all io errors terminal for now (TODO refine)
                Some(Err(err)) => { error!("connection error: {}, stopping", err); break },

                // the server is sending us voice data, forward to the audio codec task
                Some(Ok(ControlPacket::UDPTunnel(voice_packet))) => {
                    let msg = AudioCodecMessage::Inbound(*voice_packet);
                    // this might happen if the caller has started shutting down when we receive
                    // this packet. it's not accepting any new packets, so we just drop it.
                    let _ = audio_codec_sender.send(msg);
                },

                // the server is answering our ping with a pong!
                Some(Ok(ControlPacket::Ping(ping))) => {
                    debug!("received pong from server: {}", ping.get_timestamp());
                },

                // we forward to the ui all other control messages
                Some(Ok(_packet)) => {
                    // this might happen if the caller has started shutting down when we receive
                    // this packet. it's not accepting any new packets, so we just drop it.
                    // let _ = ui_sender.send(UIMessage::Control(packet));
                },
            },
        }
    }

    info!("connection task stopped");
    Ok(())
}

async fn ping(server_stream: &mut Framed<TcpStream, ClientControlCodec>) -> Result<()> {
    // first we measure the current timestamp before sending it
    use std::time::{SystemTime,UNIX_EPOCH};
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards");
    let mut ping = msgs::Ping::new();
    ping.set_timestamp(timestamp.as_secs());

    // send that shit
    use futures::sink::SinkExt;
    debug!("pinging server");
    Ok(server_stream.send(ping.into()).await?)
}

async fn handshake(
    server_stream: &mut Framed<TcpStream, ClientControlCodec>,
) -> Result<()> {
    // check that ours and server's versions are compatible
    version_handshake(server_stream).await?;

    // TODO authentication process (this should wait for cryptsetup/channel states/user states/...
    // see https://mumble-protocol.readthedocs.io/en/latest/establishing_connection.html#
    use futures::sink::SinkExt;
    server_stream.send(msgs::Authenticate::new().into()).await?;

    Ok(())
}

async fn version_handshake(
    server_stream: &mut Framed<TcpStream, ClientControlCodec>,
) -> Result<()> {
    // build our own protocol version
    let mut client_version = msgs::Version::new();
    client_version.set_version(1u32 << 16 | 2u32 << 8 | 4u32); // TODO: check this

    // send our version to the server
    use futures::sink::SinkExt;
    server_stream.send(client_version.clone().into()).await?;

    // wait for version from server
    use anyhow::Error;
    use tokio::stream::StreamExt;
    let server_version = match server_stream.next().await {
        None => Err(Error::msg("connection closed by server")),
        Some(Ok(ControlPacket::Version(version))) => Ok(*version),
        Some(Ok(packet)) => Err(Error::msg(format!("expected version packet, got: {:?}", packet))),
        Some(Err(err)) => Err(err.into()),
    }?;

    if server_version != client_version {
        Err(Error::msg(format!(
            "incompatible versions: server={} != client={}",
            server_version.get_version(),
            client_version.get_version(),
        )))
    } else {
        Ok(())
    }
}
