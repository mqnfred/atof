use anyhow::Result;
use mumble_protocol::{
    control::{ControlPacket,ServerControlCodec},
    voice::{Clientbound,Serverbound},
};
use std::collections::{HashMap,HashSet};
use tokio::{
    net::TcpStream,
    sync::mpsc::{
        unbounded_channel,
        UnboundedSender as USender,
        UnboundedReceiver as UReceiver,
    },
};
use tokio_util::codec::Framed;
use tokio::net::TcpListener;

pub struct Floor {
    rooms: HashMap<String, Room>,
    sessions: HashMap<u32, Session>,
}

#[derive(Default)]
struct Room {
    members: HashSet<u32>,
}

struct Session {
    version: Option<u32>,
    room_name: Option<String>,
    tx: USender<ControlPacket<Clientbound>>,
}

impl Default for Floor {
    fn default() -> Self {
        let mut rooms = HashMap::default();
        rooms.insert("Root".to_owned(), Room::default());
        Self {
            rooms,
            sessions: HashMap::default(),
        }
    }
}

impl Floor {
    pub async fn serve(&mut self, bind_addr: &str) -> Result<()> {
        eprintln!("binding to {}...", bind_addr);
        let mut listener = TcpListener::bind(bind_addr).await?;

        // used by sessions to communicate with the following event loop
        let (server_tx, mut server_rx) = unbounded_channel();

        loop {
            use tokio::{select,stream::StreamExt};
            select! {
                // new connection comes in, we register it and start its session handler
                stream_response = listener.next() => match stream_response {
                    None => unreachable!("tcp bind iterator never yields none"),
                    Some(Err(err)) => eprintln!("client error while connecting: {}", err),
                    Some(Ok(client_stream)) => {
                        eprintln!("received new connection...");

                        // setup input/output streams of the session
                        let client_stream = Framed::new(client_stream, ServerControlCodec::new());
                        let (session_tx, session_rx) = unbounded_channel();
                        let server_tx = server_tx.clone();

                        // add this session to our floor. we do not join any room yet,
                        // this is meant to reserve and pass the session_id to the task.
                        let session_id = match self.add_session(session_tx) {
                            Ok(session_id) => session_id,
                            Err(err) => {
                                // TODO send error reason to client, probably Reject variant
                                eprintln!("failed to add session: {}", err);
                                continue
                            }
                        };

                        // kickstart the session handler which will relay things to this server.
                        tokio::spawn(handle_session(session_id, session_rx, client_stream, server_tx));
                        eprintln!("starting session {} handling", session_id);
                    },
                },
                // we receive a control packet from a session handler
                msg = server_rx.recv() => match msg {
                    None => unreachable!("server_tx is never dropped in this context"),
                    Some((session_id, None)) => self.remove_session(session_id),
                    Some((session_id, Some(control_packet))) => self.handle_control_packet(
                        session_id,
                        control_packet,
                    ),
                },
            }
        }
    }

    fn handle_control_packet(
        &mut self,
        session_id: u32,
        control_packet: ControlPacket<Serverbound>,
    ) {
        let mut session = self.sessions.get_mut(&session_id).expect("msg from unknown session");
        match control_packet {
            ControlPacket::Version(version) => {
                eprintln!("received new version for {}: {}", session_id, version.get_version());
                session.version = Some(version.get_version());
                self.join_room(session_id, "Root".to_owned());
            }
            ControlPacket::TextMessage(msg) => {
                eprintln!("received text message from {}: {}", session_id, msg.get_message());
            }
            _ => eprintln!("got invalid packet from session {}: {:?}", session_id, control_packet),
        }
    }

    fn add_session(&mut self, session_tx: USender<ControlPacket<Clientbound>>) -> Result<u32> {
        if self.sessions.len() >= 500 {
            use anyhow::Error;
            Err(Error::msg("max user count of 500 reached"))
        } else {
            // find an available session id
            use rand::{thread_rng,Rng};
            let mut session_id = thread_rng().gen_range(0, 10_000);
            while self.sessions.contains_key(&session_id) {
                session_id = thread_rng().gen_range(0, 10_000);
            }

            // add session to our list.  we do not add to
            // any rooms yet, as they are not identified.
            self.sessions.insert(session_id, Session{
                version: None,
                room_name: None,
                tx: session_tx,
            });

            eprintln!("registering session {}", session_id);
            Ok(session_id)
        }
    }

    fn remove_session(&mut self, session_id: u32) {
        eprintln!("removing session {}", session_id);
        self.leave_room(session_id);
        self.sessions.remove(&session_id);
    }

    fn leave_room(&mut self, session_id: u32) {
        let session = self.sessions.get_mut(&session_id).expect("move of unknown session");
        if let Some(room_name) = session.room_name.take() {
            eprintln!("session {} left room {}", session_id, room_name);
            let room = self.rooms.get_mut(&room_name).expect("session in non-existant room");
            room.members.remove(&session_id);
        }
    }

    fn join_room(&mut self, session_id: u32, room_name: String) {
        eprintln!("session {} joined room {}", session_id, room_name);
        self.rooms.entry(room_name.clone()).or_insert(Room::default()).members.insert(session_id);
        let session = self.sessions.get_mut(&session_id).expect("move of unknown session");
        session.room_name = Some(room_name);
    }
}

async fn handle_session(
    session_id: u32,
    mut session_rx: UReceiver<ControlPacket<Clientbound>>,
    client_stream: Framed<TcpStream, ServerControlCodec>,
    mut server_tx: USender<(u32, Option<ControlPacket<Serverbound>>)>,
) {
    use futures::stream::StreamExt;
    let (mut client_tx, mut client_rx) = client_stream.split();

    use futures::sink::SinkExt;
    use mumble_protocol::control::msgs;
    let mut version = msgs::Version::new();
    version.set_version(1u32 << 16 | 2u32 << 8 | 4u32); // TODO: check this
    if let Err(err) = client_tx.send(version.into()).await {
        eprintln!("session {} failed to send version to client: {} (abort)", session_id, err);
        return
    }

    // forward control packets from client socket to server channel
    // TODO: gotta be a better way. send_all?
    tokio::spawn(async move {
        while let Some(msg) = client_rx.next().await {
            match msg {
                Err(err) => eprintln!("read failed for session {}: {}", session_id, err),
                Ok(msg) => if let Err(err) = server_tx.send((session_id, Some(msg))) {
                    eprintln!("encountered error while sending to server: {}", err);
                    // the server stopped listening, we're done here
                    break
                },
            };
        }

        eprintln!("session {} connection closed", session_id);
        // signal to the server that this session is a goner,
        // the server should drop the session_rx sender.
        server_tx.send((session_id, None))
    });

    // forward control packets received from session channel to client socket
    // TODO: gotta be a better way. send_all?
    tokio::spawn(async move {
        // this session_rx will return None whenever the server drops the sender.
        while let Some(msg) = session_rx.next().await {
            if let Err(err) = client_tx.send(msg).await {
                eprintln!("write failed to session {}: {}", session_id, err);
                break;
            }
        }
    });
}
