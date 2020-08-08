use mumble_protocol::control::ControlPacket;
use mumble_protocol::voice::{Clientbound,Serverbound};
use tokio::sync::mpsc::{
    UnboundedReceiver as UReceiver,
    UnboundedSender as USender,
};
use crate::thread_events::Event;

use std::{error::Error, io};
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Corner, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem},
    Terminal,
};

pub fn run_ui_thread(
    ui_recver: UReceiver<Event>,
    connection_sender: USender<ControlPacket<Serverbound>>,
) {
    // setup the tui terminal object
    let mut terminal = match io::stdout().into_raw_mode().and_then(|stdout| {
        let stdout = AlternateScreen::from(MouseTerminal::from(stdout));
        let backend = TermionBackend::new(stdout);
        Terminal::new(backend)
    }) {
        Err(err) => { eprintln!("failed to start tui: {}", err); return },
        Ok(terminal) => terminal,
    };

    
}
