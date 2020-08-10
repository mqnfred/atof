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
pub async fn run_audio_decoder_task(
    mut media_recver: UReceiver<MediaMessage>,
    muxer_sender: USender<bool>,
) {
    trace!("audio decoder task started");

    

    trace!("audio decoder task stopped");
}
