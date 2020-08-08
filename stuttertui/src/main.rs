fn main() {
    use std::env::args;
    let addr = args().skip(1).next().expect("please provide addr:port to connect to");

    // establish ui<->connection inter-thread communication
    use tokio::sync::mpsc::unbounded_channel;
    let (connection_sender, connection_recver) = unbounded_channel();
    let (events_sender, events_recver) = unbounded_channel();
    let (ui_sender, ui_recver) = unbounded_channel();

    use std::thread::spawn;
    use thread_ui::run_ui_thread;
    let _ = spawn(move || { run_ui_thread(ui_recver, connection_sender) }).join();

    use std::time::Duration;
    use thread_events::run_events_thread;
    let tick_rate = Duration::from_millis(10);
    let _ = spawn(move || { run_events_thread(tick_rate, events_recver, ui_sender) }).join();

    use thread_connection::run_connection_thread;
    let _ = spawn(move || {
        run_connection_thread(&addr, connection_recver, events_sender)
    }).join();
}

mod thread_connection;
mod thread_events;
mod thread_ui;
