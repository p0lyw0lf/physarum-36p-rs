use std::sync::{Arc, Mutex, mpsc};

use crate::audio::SAMPLES;
use crate::audio::collector::Collector;
use crate::audio::fft::fft_buckets;

pub struct Worker {
    /// Waits on this to start the next batch of work
    rx: mpsc::Receiver<()>,
    /// The samples collector that we are reading from
    collector: Arc<Mutex<Collector>>,
    /// The canonical most recent batch of frequency bins to display
    bins: Arc<Mutex<Vec<f32>>>,
}

impl Worker {
    /// Creates a new worker with a mpsc buffer size of 1. Threads that wish to trigger the worker
    /// simply need to attempt to put a value, and discard if the queue is full.
    pub fn new(
        collector: Arc<Mutex<Collector>>,
    ) -> (mpsc::SyncSender<()>, Arc<Mutex<Vec<f32>>>, Self) {
        let (tx, rx) = mpsc::sync_channel(1);
        let bins = Arc::new(Mutex::new(Vec::new()));
        (
            tx,
            bins.clone(),
            Self {
                rx,
                collector,
                bins,
            },
        )
    }
}

/// Notifies the worker on the other size of tx that there is more work to be done.
pub fn submit_work(tx: &mpsc::SyncSender<()>) {
    match tx.try_send(()) {
        Ok(()) => {}
        Err(mpsc::TrySendError::Full(())) => {}
        Err(mpsc::TrySendError::Disconnected(())) => {
            panic!("worker stopped unexpectedly");
        }
    }
}

impl Worker {
    /// Main loop of the worker where is processes all incoming work. Should be run in its own
    /// thread.
    pub fn work(self) {
        loop {
            self.rx.recv().expect("sender closed unexpectedly");
            self.snapshot_fft_buckets();
        }
    }

    /// Given samples collected from an audio source, take a snapshot of the most recent samples
    /// & bucket the results into pre-determined frequency ranges.
    fn snapshot_fft_buckets(&self) {
        let mut samples = [0.0f32; SAMPLES];
        let sample_rate = {
            let collector = self.collector.lock().unwrap();
            collector.snapshot(&mut samples);
            collector.sample_rate()
        };
        let new_bins = fft_buckets(&mut samples, sample_rate);
        {
            let mut bins = self.bins.lock().unwrap();
            *bins = new_bins;
        };
    }
}
