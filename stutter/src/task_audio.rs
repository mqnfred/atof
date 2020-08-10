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

use stammer::{ClientMessage,MediaMessage};
pub async fn run_audio_task(
    client_sender: USender<ClientMessage>,
    audio_recver: UReceiver<MediaMessage>,
) {
    trace!("audio task started");

    use tokio::sync::mpsc::unbounded_channel;
    let (muxer_sender, muxer_recver) = unbounded_channel();
    let (io_sender, io_recver) = unbounded_channel();

    use tokio::join;
    use super::task_audio_io::run_audio_io_task;
    use super::task_audio_muxer::run_audio_muxer_task;
    use super::task_audio_decoder::run_audio_decoder_task;
    join!(
        run_audio_decoder_task(media_recver, muxer_sender),
        run_audio_muxer_task(muxer_recver, io_sender),
        run_audio_io_task(io_sender),
    );

    /*
    use cpal::{Data, Sample, SampleFormat};
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();

    let device = host.default_input_device().expect("no output device available");
    let mut supported_input_configs_range = device.supported_input_configs()
        .expect("error while querying configs");
    panic!("{:#?}", supported_input_configs_range);

    let device = host.default_output_device().expect("no output device available");
    let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
    let mut supported_configs_range = device.supported_output_configs()
        .expect("error while querying configs");
    let supported_config = supported_configs_range.next()
        .expect("no supported config?!")
        .with_max_sample_rate();
    let sample_format = supported_config.sample_format();
    let config = supported_config.into();
    let stream = match sample_format {
        SampleFormat::F32 => device.build_output_stream(&config, |data: &mut [f32], _| {
            trace!("write_silence called my dude");
            for sample in data.iter_mut() {
                *sample = Sample::from(&0.0);
            }
        }, err_fn),
        SampleFormat::I16 => device.build_output_stream(&config, |data: &mut [i16], _| {
            trace!("write_silence called my dude");
            for sample in data.iter_mut() {
                *sample = Sample::from(&0.0);
            }
        }, err_fn),
        SampleFormat::U16 => device.build_output_stream(&config, |data: &mut [u16], _| {
            trace!("write_silence called my dude");
            for sample in data.iter_mut() {
                *sample = Sample::from(&0.0);
            }
        }, err_fn),
    }.unwrap();

    stream.play();
    */

    use std::thread::sleep;
    sleep(Duration::from_secs(3));

    trace!("audio task stopped");
}
