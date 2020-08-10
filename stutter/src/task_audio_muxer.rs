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
pub async fn run_audio_muxer_task(
    mut muxer_recver: UReceiver<bool>,
    io_sender: USender<bool>,
) {
    trace!("audio muxer task started");

    

    trace!("audio muxer task stopped");
}
