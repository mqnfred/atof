use anyhow::Result;
use mumble_protocol::control::{ClientControlCodec,ControlPacket,msgs};
use mumble_protocol::voice::{Clientbound,Serverbound};
use tokio::sync::mpsc::{
    UnboundedReceiver as UReceiver,
    UnboundedSender as USender,
};
use tokio_util::codec::Framed;
use tokio::net::TcpStream;
use log::{error,info,debug};

/*
pub fn run_connection_thread(
    addr: &str,
    connection_recver: UReceiver<ControlPacket<Serverbound>>, // from caller to server
    ui_sender: USender<ControlPacket<Clientbound>>, // from server to caller
) {
    // we need to spawn a tokio runtime as the connection task is asynchronous
    use tokio::runtime::Runtime;
    let mut rt = match Runtime::new() {
        Err(err) => { eprintln!("failed to start runtime: {}", err); return },
        Ok(rt) => rt,
    };

    // connect to the server and spin up our connection task
    rt.block_on(async move {
        // connect to server and wrap connection in mumble control codec
        let server_stream = match TcpStream::connect(&addr).await {
            Ok(server_stream) => Framed::new(server_stream, ClientControlCodec::new()),
            Err(err) => { eprintln!("failed to establish connection: {}", err); return },
        };
        eprintln!("established connection to server");

        // the connection_task babysits the connection, it:
        //
        //  1. sends and receives server pings
        //  2. forwards any server-bound packets to the server (from the ui thread)
        //  3. forwards any client-bound packets to the ui thread (from the caller)
        if let Err(err) = run_connection_task(server_stream, connection_recver, ui_sender).await {
            eprintln!("connection error: {}", err);
        }
    });
}
*/

pub async fn run_client_task(
    mut server_stream: Framed<TcpStream, ClientControlCodec>,
    mut caller_recver: UReceiver<ControlPacket<Serverbound>>, // from caller to server
    caller_sender: USender<ControlPacket<Clientbound>>, // from server to caller
) -> Result<()> { // TODO tasks should not return results i think
    // handshake with the server (version exchange, authentication...)
    handshake(&mut server_stream).await?;
    info!("server handshake successful");

    // set up ping interval at 25s
    use std::time::Duration;
    use tokio::time::interval;
    let mut keepalive = interval(Duration::from_secs(25));

    loop {
        use tokio::select;
        use futures::sink::SinkExt;
        use tokio::stream::StreamExt;
        select! {
            // reminder to send a ping to the server
            _ = keepalive.next() => ping(&mut server_stream).await?,

            // received a message from the caller, forwarding it to the server
            serverbound_msg = caller_recver.next() => match serverbound_msg {
                // this would happen in the event of a graceful caller shutdown
                None => { info!("caller is shutting down, gracefully stopping"); break },
                Some(msg) => server_stream.send(msg.into()).await?,
            },

            // received a message from the server, forwarding it to the caller
            clientbound_msg = server_stream.next() => match clientbound_msg {
                // the server closed the connection
                None => { error!("connection closed by server, stopping"); break },

                // we consider all io errors terminal for now (TODO refine)
                Some(Err(err)) => { error!("connection error: {}, stopping", err); break },

                // the server is answering our ping with a pong!
                Some(Ok(ControlPacket::Ping(ping))) => {
                    debug!("received pong from server: {}", ping.get_timestamp());
                },

                // we forward to the caller all messages we don't understand
                Some(Ok(packet)) => {
                    // this might happen if the caller has started shutting down when we receive
                    // this packet. it's not accepting any new packets, so we just drop it.
                    let _ = caller_sender.send(packet);
                },
            },
        }
    }

    info!("client task stopped");
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
