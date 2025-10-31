use rodio::Sample;
use rodio::Source;

pub struct InspectableSource(Box<dyn Source + Send>);

impl InspectableSource {
    pub fn new(source: Box<dyn Source + Send>) -> Self {
        Self(source)
    }
}

impl Iterator for InspectableSource {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        let out = self.0.next();
        if let Some(s) = out {
            println!("{s}");
        }
        out
    }
}

impl Source for InspectableSource {
    fn current_span_len(&self) -> Option<usize> {
        self.0.current_span_len()
    }

    fn channels(&self) -> rodio::ChannelCount {
        let out = self.0.channels();
        println!("number of channels: {out}");
        out
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.0.sample_rate()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.0.total_duration()
    }
}
