use ringbuffer::RingBuffer;
use rodio::Sample;
use rodio::Source;

/// Number of samples in the buffer. Must be a power of 2.
const SAMPLES: usize = 2048;

pub struct InspectableSource {
    source: Box<dyn Source + Send>,
    /// one for each of the channels in the source. Assumes that, upon initialization, the number
    /// of channels in source doesn't change.
    channel_buffers: Vec<ringbuffer::ConstGenericRingBuffer<Sample, SAMPLES>>,
    next_channel_index: usize,

    cached_current_span_len: Option<usize>,
    cached_channels: rodio::ChannelCount,
}

impl InspectableSource {
    pub fn new(source: Box<dyn Source + Send>) -> Self {
        Self {
            source,
            channel_buffers: Vec::new(),
            next_channel_index: 0,
            cached_current_span_len: Some(0), // force checking immediately
            cached_channels: 0,
        }
    }

    pub fn snapshot(&self, out: &mut [Sample; SAMPLES]) {
        for buffer in self.channel_buffers.iter() {
            for i in 0..SAMPLES {
                out[i] += buffer[i];
            }
        }
    }
}

impl Iterator for InspectableSource {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(remaining) = self.cached_current_span_len {
            match remaining {
                // Recalculate number of channels we need to interleave
                0 => {
                    assert_eq!(self.next_channel_index, 0);

                    self.cached_current_span_len = self.current_span_len();
                    let new_channels = self.channels();
                    if new_channels != self.cached_channels {
                        self.cached_channels = new_channels;
                        // There's not really a great way to interpolate between different numbers
                        // of channels, so let's just start over.
                        self.channel_buffers = (0..new_channels)
                            .map(|_| {
                                let mut out = ringbuffer::ConstGenericRingBuffer::new();
                                out.fill_default();
                                out
                            })
                            .collect();
                    }
                }
                n => {
                    self.cached_current_span_len = Some(n - 1);
                }
            }
        }

        // output of a source is specified to always be interleaved
        let out = self.source.next();
        if let Some(v) = out {
            self.channel_buffers[self.next_channel_index].enqueue(v);
            self.next_channel_index =
                (self.next_channel_index + 1) % usize::from(self.cached_channels);
        }
        out
    }
}

impl Source for InspectableSource {
    fn current_span_len(&self) -> Option<usize> {
        self.source.current_span_len()
    }

    fn channels(&self) -> rodio::ChannelCount {
        let out = self.source.channels();
        println!("number of channels: {out}");
        out
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.source.sample_rate()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.source.total_duration()
    }
}
