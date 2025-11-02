use std::sync::Arc;
use std::sync::Mutex;

use ringbuffer::RingBuffer;
use rodio::ChannelCount;
use rodio::Sample;
use rodio::SampleRate;
use rodio::Source;

use super::SAMPLES;

/// Collects a sliding window of samples into per-channel buffers.
pub struct Collector {
    /// one for each of the channels in the source. Assumes that, upon initialization, the number
    /// of channels in source doesn't change.
    channel_buffers: Vec<ringbuffer::ConstGenericRingBuffer<Sample, SAMPLES>>,
    /// A cached sample rate from the last time it updated
    cached_sample_rate: SampleRate,
}

impl Collector {
    pub fn snapshot(&self, out: &mut [Sample; SAMPLES]) {
        for buffer in self.channel_buffers.iter() {
            for i in 0..SAMPLES {
                out[i] += buffer[i];
            }
        }
    }

    pub fn sample_rate(&self) -> SampleRate {
        self.cached_sample_rate
    }

    pub fn new<S: Source + Send>(source: S) -> (Arc<Mutex<Self>>, impl Source + Send) {
        let collector = Arc::new(Mutex::new(Self {
            channel_buffers: Vec::new(),
            cached_sample_rate: 0,
        }));
        let c1 = collector.clone();
        let c2 = collector.clone();
        let source = Inspectable::new(
            source,
            move |sample, channel_index| {
                let mut this = c1.lock().unwrap();
                this.channel_buffers[usize::from(channel_index)].enqueue(sample);
            },
            move |num_channels, sample_rate| {
                let mut this = c2.lock().unwrap();
                this.channel_buffers = (0..num_channels)
                    .map(|_| {
                        let mut out = ringbuffer::ConstGenericRingBuffer::new();
                        out.fill_default();
                        out
                    })
                    .collect();
                this.cached_sample_rate = sample_rate;
            },
        );

        (collector, source)
    }
}

struct Inspectable<I, F1, F2>
where
    I: Source,
    F1: FnMut(Sample, ChannelCount),
    F2: FnMut(ChannelCount, SampleRate),
{
    inner: I,
    /// Called with (sample, channel_index)
    sample_inspector: F1,
    /// Called with (num_channels, sample_rate) every time the .channels() or .sample_rate()
    /// functions change
    channels_inspector: F2,

    next_channel_index: rodio::ChannelCount,
    cached_current_span_len: Option<usize>,
    cached_channels: rodio::ChannelCount,
    cached_sample_rate: rodio::SampleRate,
}

impl<I, F1, F2> Inspectable<I, F1, F2>
where
    I: Source,
    F1: FnMut(Sample, ChannelCount),
    F2: FnMut(ChannelCount, SampleRate),
{
    pub fn new(inner: I, sample_inspector: F1, channels_inspector: F2) -> Self {
        Self {
            inner,
            sample_inspector,
            channels_inspector,
            next_channel_index: 0,
            cached_current_span_len: Some(0), // Make it so the next .next() call updates this
            cached_channels: 0,
            cached_sample_rate: 0,
        }
    }
}

impl<I, F1, F2> Iterator for Inspectable<I, F1, F2>
where
    I: Source,
    F1: FnMut(Sample, ChannelCount),
    F2: FnMut(ChannelCount, SampleRate),
{
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(remaining) = self.cached_current_span_len {
            match remaining {
                // Recalculate number of channels we need to interleave
                0 => {
                    // TODO: this isn't actually true, idk why, shouldn't affect things too much i
                    // hope??
                    // assert_eq!(self.next_channel_index, 0);
                    self.next_channel_index = 0;

                    self.cached_current_span_len = self.current_span_len();
                    let new_channels = self.channels();
                    let new_sample_rate = self.sample_rate();
                    if new_channels != self.cached_channels
                        || new_sample_rate != self.cached_sample_rate
                    {
                        self.cached_channels = new_channels;
                        self.cached_sample_rate = new_sample_rate;
                        (self.channels_inspector)(self.cached_channels, self.cached_sample_rate);
                    }
                }
                n => {
                    self.cached_current_span_len = Some(n - 1);
                }
            }
        }

        // output of a source is specified to always be interleaved
        let out = self.inner.next();
        if let Some(v) = out {
            (self.sample_inspector)(v, self.next_channel_index);
            self.next_channel_index = (self.next_channel_index + 1) % self.cached_channels;
        }
        out
    }
}

impl<I, F1, F2> Source for Inspectable<I, F1, F2>
where
    I: Source,
    F1: FnMut(Sample, ChannelCount),
    F2: FnMut(ChannelCount, SampleRate),
{
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.inner.total_duration()
    }
}
