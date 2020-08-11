use anyhow::{Error,Result};
use tokio::sync::mpsc::{
    UnboundedReceiver as UReceiver,
    UnboundedSender as USender,
};
use log::{error,debug,warn,trace};
use super::task_audio_codec::AudioCodecMessage;

pub enum AudioIOMessage {
    RecordingPlay,
    RecordingPause,

    PlaybackPlay,
    PlaybackPause,
    PlaybackInbound(Vec<f32>),
}

pub async fn run_audio_io_task(
    mut audio_io_recver: UReceiver<AudioIOMessage>,
    audio_codec_sender: USender<AudioCodecMessage>,
) {
    trace!("audio io task started");
    use cpal::traits::StreamTrait;

    // setup input stream
    use cpal::default_host;
    let host = default_host();

    // initialize the input stream handler, with its callback behind it which reads data
    // from the audio drivers' buffers and forwards it into the audio_codec_sender pipeline
    let input_stream = match init_input_stream(&host, audio_codec_sender.clone()).await {
        Err(err) => { eprintln!("failed to init input stream: {}", err); return },
        Ok(input_stream) => input_stream,
    };

    // buffering of inbound audio content
    // for consumption by playback callback
    use ringbuf::RingBuffer;
    let ring = RingBuffer::new(48_000 * 2);
    let (mut producer, consumer) = ring.split();
    // initialize the output stream handler, with its callback behind it
    // which writes data from the ringbuffer onto the audio drivers' buffers.
    let output_stream = match init_output_stream(&host, consumer).await {
        Err(err) => { eprintln!("failed to init output stream: {}", err); return },
        Ok(output_stream) => output_stream,
    };

    use tokio::stream::StreamExt;
    while let Some(msg) = audio_io_recver.next().await {
        match msg {
            // write any packets to output ringbuffer, which is read by the output stream callback
            AudioIOMessage::PlaybackInbound(buffer) => { producer.push_slice(&buffer); },
            // TODO we should handle receiving data from multiple peers. this can be
            // done by looking at the origin session_id of the request in the decoder
            // request maintaining a ringbuffer by source.
            //
            // all ringbuffers would be read by the output callback which would combine
            // all the streams using a simple digital audio processing technique.

            // we enable driving of the input/output streams from outside
            // any failure is considered final to the program for now
            AudioIOMessage::RecordingPlay => {
                if let Err(err) = input_stream.play() {
                    error!("failed to start recording of input stream (microphone): {}", err);
                    break
                }
                debug!("started recording of input stream (microphone)");
            },
            AudioIOMessage::RecordingPause => {
                if let Err(err) = input_stream.pause() {
                    error!("failed to stop recording of input stream (microphone): {}", err);
                    break
                }
                debug!("stopped recording of input stream (microphone)");
            },
            AudioIOMessage::PlaybackPlay => {
                if let Err(err) = output_stream.play() {
                    error!("failed to start playback of output stream (speakers): {}", err);
                    break
                }
                debug!("started playback of output stream (speakers)");
            }
            AudioIOMessage::PlaybackPause => {
                if let Err(err) = output_stream.pause() {
                    error!("failed to stop playback on output stream (speakers): {}", err);
                    break
                }
                debug!("stopped playback of output stream (speakers)");
            },
        }
    }

    trace!("audio io task stopped");
}

pub async fn init_input_stream(
    host: &cpal::Host,
    audio_codec_sender: USender<AudioCodecMessage>,
) -> Result<cpal::Stream> {
    // setup cpal input device
    use cpal::traits::HostTrait;
    let device = host.default_input_device().ok_or_else(|| Error::msg("no input device found"))?;
    trace!("input device selected: {}", device.name()?);

    // setup input config
    use cpal::traits::DeviceTrait;
    use cpal::{BufferSize,StreamConfig};
    let mut config: StreamConfig = device.default_input_config()?.into();
    config.buffer_size = BufferSize::Fixed(48_000 / 50);
    trace!("input stream config: {:?}", config);

    // setup data and error callbacks
    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        trace!("reading {} f32 samples from input (microphone)", data.len());
        if let Err(err) = audio_codec_sender.send(AudioCodecMessage::Outbound(Vec::from(data))) {
            // here we can ignore errors stemming from codec task closing as the thread
            // calling this callback will be stopped when the audio io task stops
            trace!("failed to send audio data to codec task: {}", err);
        }
    };
    fn err_fn(err: cpal::StreamError) {
        warn!("an error occured while sampling input (microphone): {}", err);
    }

    Ok(device.build_input_stream(&config, input_data_fn, err_fn)?)
}

pub async fn init_output_stream(
    host: &cpal::Host,
    mut consumer: ringbuf::Consumer<f32>,
) -> Result<cpal::Stream> {
    // setup cpal output device
    use cpal::traits::HostTrait;
    let device = host.default_output_device().ok_or_else(|| Error::msg("no output device found"))?;
    trace!("output device selected: {}", device.name()?);

    // setup output config
    use cpal::traits::DeviceTrait;
    use cpal::{BufferSize,StreamConfig};
    let mut config: StreamConfig = device.default_output_config()?.into();
    config.buffer_size = BufferSize::Fixed(48_000 / 50);
    trace!("output stream config: {:?}", config);

    // setup data and error callbacks
    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        trace!("writing {} f32 samples to output (speakers)", data.len());
        for dp in data.iter_mut() {
            *dp = consumer.pop().unwrap_or(0.0f32);
        }
    };
    fn err_fn(err: cpal::StreamError) {
        panic!("an error occured while writing samples to output (speakers): {}", err);
    }

    Ok(device.build_output_stream(&config, output_data_fn, err_fn)?)
}
