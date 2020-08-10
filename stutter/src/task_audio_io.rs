use anyhow::Result;
use mumble_protocol::control::{ClientControlCodec,ControlPacket,msgs};
use mumble_protocol::voice::{VoicePacket,Clientbound,Serverbound};
use tokio::sync::mpsc::{
    UnboundedReceiver as UReceiver,
    UnboundedSender as USender,
};
use tokio_util::codec::Framed;
use tokio::net::TcpStream;
use std::time::Duration;
use log::trace;

use stammer::MediaMessage;
pub async fn run_audio_io_task(
    mut io_recver: UReceiver<bool>,
) {
    trace!("audio io task started");

    

    trace!("audio io task stopped");
}
