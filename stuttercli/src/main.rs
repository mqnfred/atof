use anyhow::Result;
use mumble_protocol::control::ClientControlCodec;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
#[macro_use]
extern crate tokio_core;

#[tokio::main]
async fn main() {
    let addr = "localhost:5000";
    if let Err(err) = run(addr).await {
        eprintln!("error: {}", err);
    }
}

mod task_async_readline;

async fn run(addr: &str) -> Result<()> {
    let server_stream = TcpStream::connect(&addr).await?;

    // setup server framed "rw stream"
    let mut server_stream = Framed::new(server_stream, ClientControlCodec::new());

    // handshake with the server (version exchange, authentication...)
    handshake(&mut server_stream).await?;
    eprintln!("server handshake successful");

    // setup stdin framed reader stream
    use tokio::io::stdin;
    use tokio_util::codec::FramedRead;
    use tokio_util::codec::LinesCodec;
    let mut stdin = FramedRead::new(stdin(), LinesCodec::new());

    loop {
        // TODO write the prompt to stdout?
        print!(">>> ");
        use std::io::{Write,stdout};
        stdout().flush()?;

        use mumble_protocol::control::ControlPacket;
        use tokio::stream::StreamExt;
        use tokio::select;
        select! {
            // command entered by the user
            text_input = stdin.next() => match text_input {
                None => { eprintln!("input closed, stopping chat..."); break },
                Some(Err(err)) => eprintln!("failed to read input: {}", err),
                Some(Ok(text_input)) => {
                    let mut tm = msgs::TextMessage::new();
                    tm.set_message(text_input);

                    use futures::sink::SinkExt;
                    if let Err(err) = server_stream.send(tm.into()).await {
                        eprintln!("failed to send our message: {}", err);
                    }
                }
            },

            // received message from server
            msg = server_stream.next() => match msg {
                // TODO: this does not close stdin properly, need to press enter
                // see: https://docs.rs/tokio/0.2.22/tokio/io/struct.Stdin.html
                None => { eprintln!("server connexion closed, stopping chat..."); break },
                Some(Err(err)) => eprintln!("error while reading from server: {}", err),
                Some(Ok(ControlPacket::TextMessage(tm))) => {
                    // beginning newline is here to move on from the prompt before printing msg
                    println!("\n<{}> {}", tm.get_actor(), tm.get_message());
                },
                Some(Ok(packet)) => eprintln!("received unknown control packet: {:?}", packet),
            },
        }
    }

    Ok(())
}

use mumble_protocol::control::msgs;
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
    use mumble_protocol::control::ControlPacket;
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
