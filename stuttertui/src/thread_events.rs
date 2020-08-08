use mumble_protocol::control::ControlPacket;
use mumble_protocol::voice::{Clientbound,Serverbound};
use tokio::sync::mpsc::{
    UnboundedReceiver as UReceiver,
    UnboundedSender as USender,
};

use std::io;
use std::sync::mpsc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use termion::event::Key;
use termion::input::TermRead;
use std::time::Duration;

pub enum Event {
    Update(ControlPacket<Clientbound>),
    Input(Key),
    Tick,
}

pub fn run_events_thread(
    tick_rate: Duration,
    mut events_recver: UReceiver<ControlPacket<Clientbound>>,
    ui_sender: USender<Event>,
) {
    // run thread which listens to keys on stdin
    // and sends them on the events channel
    let keys_sender = ui_sender.clone();
    use std::thread::spawn;
    let _ = spawn(move || {
        use std::io::stdin;
        for key_event in stdin().keys() {
            match key_event {
                // idk when this is supposed to happen, better log
                Err(err) => { eprintln!("failed to read key: {}", err); break },

                // sending will fail whenever the other threads stop
                Ok(key) => if keys_sender.send(Event::Input(key)).is_err() { break },
            }
        }
    }).join();

    // pipe all the events_recver events (from connection thread) into the ui_sender
    // if no events are available, we also take the opportunity to send a tick.
    let _ = spawn(move || {
        loop {
            use std::thread::sleep;
            use tokio::sync::mpsc::error::TryRecvError;
            let event = match events_recver.try_recv() {
                // forward any new packets to the ui as an update event
                Ok(packet) => Event::Update(packet),

                // nothing on the channel right now, we'll send a tick instead
                Err(TryRecvError::Empty) => Event::Tick,
                // TODO no having a dedicated tick thread means we will struggle
                // to quickly update the UI on batched update events, which will
                // feel sluggish. it is simpler in the meantime (no need to stop it)

                // connection thread is dead, we die
                Err(TryRecvError::Closed) => break,
            };

            // if the ui_sender is closed, then the ui is gracefully
            // shutting down and we should shut down as well.
            if ui_sender.send(event).is_err() {
                break
            }

            sleep(tick_rate);
        }
    }).join();
}
