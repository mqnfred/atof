use anyhow::{Error,Result};
use std::collections::{HashMap,HashSet};
use tokio::sync::mpsc::UnboundedSender as USender;
use mumble_protocol::control::msgs;
use mumble_protocol::control::ControlPacket;
use mumble_protocol::voice::Clientbound;
use log::debug;

type SessionID = u32;
type RoomID = u32;

#[derive(Clone, Debug, Default)]
pub struct RoutingTable {
    sessions: HashMap<SessionID, Session>,
    rooms: HashMap<RoomID, Room>,
}

#[derive(Clone, Debug)]
struct Session {
    room_id: RoomID,
    version: msgs::Version,
    sender: USender<ControlPacket<Clientbound>>,
}

#[derive(Clone, Debug, Default)]
struct Room {
    members: HashSet<SessionID>,
}

impl RoutingTable {
    pub fn holds_session(&self, session_id: SessionID) -> bool {
        self.sessions.contains_key(&session_id)
    }

    pub fn enroll_session(
        &mut self,
        session_id: SessionID,
        version: msgs::Version,
        sender: USender<ControlPacket<Clientbound>>,
    ) {
        let room_id = 0u32 as RoomID; // default room
        self.sessions.insert(session_id, Session{room_id, version, sender});
        self.rooms.entry(room_id).or_insert(Room::default()).members.insert(session_id);
    }

    pub fn expel_session(&mut self, session_id: SessionID) -> Result<()> {
        let session = self.sessions.remove(&session_id).ok_or_else(|| {
            Error::msg(format!("unknown session {}", session_id))
        })?;
        self.rooms.get_mut(&session.room_id).expect("bad memberships").members.remove(&session_id);
        Ok(())
    }

    pub fn _move_session(&mut self, session_id: SessionID, dest_room_id: RoomID) -> Result<()> {
        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            Error::msg(format!("unknown session {}", session_id))
        })?;
        let orig_room_id = session.room_id;

        // if the session is in the routing table, it necessarily belongs to a room
        debug!("session {} left room {}", session_id, orig_room_id);
        let orig_room = self.rooms.get_mut(&orig_room_id).expect("bad memberships");
        orig_room.members.remove(&session_id);

        debug!("session {} joined room {}", session_id, dest_room_id);
        self.rooms.entry(dest_room_id).or_insert(Room::default()).members.insert(session_id);
        session.room_id = dest_room_id;

        Ok(())
    }

    pub fn sender(&self, session_id: SessionID) -> Option<&USender<ControlPacket<Clientbound>>> {
        self.sessions.get(&session_id).map(|s| &s.sender)
    }

    pub fn room_senders(
        &self,
        room_id: RoomID,
    ) -> impl Iterator<Item=&USender<ControlPacket<Clientbound>>> {
        // FIXME this will panic if somebody requests senders for a room that does not
        // exist this is decided on the client side, so potential ddos from client
        // could use std::iter::empty() but then we're returning != concrete types
        //                       vvvvvvvv
        self.rooms.get(&room_id).unwrap().members.iter().filter_map(move |session_id| {
            self.sender(*session_id)
        })
    }
}
