use mumble_protocol::voice::{VoicePacket,Clientbound,Serverbound};
use tokio::sync::mpsc::{
    UnboundedReceiver as UReceiver,
    UnboundedSender as USender,
};
use crate::task_connection::ConnectionMessage;
use crate::task_audio_io::AudioIOMessage;
use log::trace;

pub enum AudioCodecMessage {
    Inbound(VoicePacket<Clientbound>),
    Outbound(Vec<f32>),
}

pub async fn run_audio_codec_task(
    mut audio_encode_recver: UReceiver<AudioCodecMessage>,
    connection_sender: USender<ConnectionMessage>,
    audio_io_sender: USender<AudioIOMessage>,
) {
    trace!("audio encode task started");

    // setup opus encoder
    use opus::{Application,Channels,Encoder,Decoder};
    let mut encoder = Encoder::new(48_000, Channels::Stereo, Application::Voip).expect("static");
    let mut decoder = Decoder::new(48_000, Channels::Stereo).expect("static");

    // ringbuffer used to collect pcm stream from input
    // (microphone) before batching and encoding as opus.
    use ringbuf::RingBuffer;
    let ring = RingBuffer::new(48_000 * 2);
    let (mut producer, mut consumer) = ring.split();

    // loop over messages received from connection task and audio io task.
    use tokio::stream::StreamExt;
    use mumble_protocol::voice::VoicePacketPayload;
    while let Some(msg) = audio_encode_recver.next().await {
        match msg {
            // outbound packets sent by the audio io task (from audio drivers/microphone,)
            // to be encoded and sent to the connexion task for consumption by the server
            AudioCodecMessage::Outbound(buffer) => {
                producer.push_slice(&buffer);
                while producer.len() >= 48_000 / 50 {
                    // TODO the amount of samples we pack in an opus frame is 100%
                    // arbitrary for now. 48_000/50=960 samples per opus packet, which
                    // looks to be around 128 bytes on average.
                    //
                    // we could increase the size of a packet but that would increase latency.
                    let mut b = vec![0f32; 48_000 / 50];
                    assert_eq!(consumer.pop_slice(&mut b), b.len());

                    let mut output = vec![0u8; 1024];
                    let size = encoder.encode_float(&b, &mut output).unwrap();
                    output.truncate(size);
                    trace!("encoded {} f32 samples into {}b opus packet", b.len(), output.len());

                    // build the voice packet
                    use bytes::Bytes;
                    let msg = ConnectionMessage::Voice(VoicePacket::Audio{
                        _dst: std::marker::PhantomData::<Serverbound>,
                        target: 0, // 0 is the "normal speaking" to your own room
                        session_id: (),
                        seq_num: 0,
                        payload: VoicePacketPayload::Opus(Bytes::from(output), false),
                        position_info: None,
                    });

                    // if the sender stops, this must be a graceful shutdown, we follow
                    if let Err(err) = connection_sender.send(msg) {
                        trace!("connection task does not accept any new messages: {}", err);
                        break
                    }
                }
            },

            // received voice message from the connection task, to be decoded and
            // sent to the audio task for consumption by the audio driver/speakers
            AudioCodecMessage::Inbound(voice_packet) => match voice_packet {
                // any audio packet received from the server
                VoicePacket::Audio{payload, ..} => match payload {
                    // this implementation only supports the opus codec
                    VoicePacketPayload::Opus(buffer, end) => {
                        let mut b = vec![0f32; 48_000 / 50];
                        decoder.decode_float(&buffer, &mut b, end).unwrap();

                        let msg = AudioIOMessage::PlaybackInbound(b);
                        if let Err(err) = audio_io_sender.send(msg) {
                            trace!("audio io task is not accepting new messages: {}", err);
                            break
                        }
                    },

                    // any non-opus codecs will just be ignored/dropped silently
                    // this means that the user will not hear any peer sending
                    // audio encoded as speex/celt/...
                    _ => continue,
                },

                // this is a pong from the server
                VoicePacket::Ping{..} => unimplemented!("audio pings not supported yet"),
            },
        }
    }

    trace!("audio encode task stopped");
}
