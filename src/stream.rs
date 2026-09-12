//! Push-based streaming transform for real-time use.

use crate::cqt::{Cqt, CqtError, Scratch};
use crate::levels::Levels;

/// Incremental transform that consumes audio in arbitrary chunks and emits a
/// frame every `hop_size` samples.
///
/// Frames are numbered from the first sample pushed. Frame `i` is centred on
/// sample `i * hop_size`, exactly as in [`Cqt::process`], and is emitted as
/// soon as sample `i * hop_size + latency - 1` has been pushed, where
/// `latency` is [`Cqt::latency_samples`]. Pushing performs no allocation
/// once the internal buffers have grown to their steady-state size.
///
/// ```
/// use cqt_rs::{Cqt, CqtParams, CqtStream};
///
/// let cqt = Cqt::new(CqtParams::new(22_050, 110.0, 4_186.0, 12).unwrap());
/// let mut stream = CqtStream::new(&cqt, 256).unwrap();
/// let mut frames = 0;
/// for chunk in vec![0.0f32; 22_050].chunks(480) {
///     stream.push(&cqt, chunk, |_frame| frames += 1);
/// }
/// stream.flush(&cqt, |_frame| frames += 1);
/// assert_eq!(frames, 1 + 22_050 / 256);
/// ```
#[derive(Debug, Clone)]
pub struct CqtStream {
    hop_size: usize,
    latency: usize,
    levels: Levels,
    /// Samples pushed since creation or the last reset.
    samples_in: u64,
    frames_out: u64,
    scratch: Scratch,
    magnitudes: Vec<f32>,
}

impl CqtStream {
    /// Creates a stream that emits one frame every `hop_size` samples.
    pub fn new(cqt: &Cqt, hop_size: usize) -> Result<Self, CqtError> {
        cqt.check_hop_size(hop_size)?;
        Ok(Self {
            hop_size,
            latency: cqt.latency_samples(),
            levels: cqt.levels(),
            samples_in: 0,
            frames_out: 0,
            scratch: cqt.scratch(),
            magnitudes: vec![0.0; cqt.num_bins()],
        })
    }

    /// Hop between frames in samples.
    pub fn hop_size(&self) -> usize {
        self.hop_size
    }

    /// Delay between a frame's centre and its emission, in samples.
    pub fn latency_samples(&self) -> usize {
        self.latency
    }

    /// Number of frames emitted so far.
    pub fn frames_emitted(&self) -> u64 {
        self.frames_out
    }

    /// Number of samples consumed so far.
    pub fn samples_consumed(&self) -> u64 {
        self.samples_in
    }

    /// Forgets all buffered audio and restarts frame numbering.
    pub fn reset(&mut self, cqt: &Cqt) {
        self.levels = cqt.levels();
        self.samples_in = 0;
        self.frames_out = 0;
    }

    /// Feeds `samples` and calls `on_frame` with the magnitudes of every
    /// frame that became complete, in order.
    pub fn push<F>(&mut self, cqt: &Cqt, samples: &[f32], mut on_frame: F)
    where
        F: FnMut(&[f32]),
    {
        self.samples_in += samples.len() as u64;
        self.feed(cqt, samples, &mut on_frame, u64::MAX);
    }

    /// Emits the frames whose centre lies inside the audio pushed so far by
    /// zero padding the end, matching [`Cqt::process`] on the same signal.
    ///
    /// The padding stays in the buffer: pushing more audio afterwards
    /// behaves as if a short silence had been inserted. Call
    /// [`CqtStream::reset`] before reusing the stream for unrelated audio.
    pub fn flush<F>(&mut self, cqt: &Cqt, mut on_frame: F)
    where
        F: FnMut(&[f32]),
    {
        let total_frames = 1 + self.samples_in / self.hop_size as u64;
        let block = vec![0.0f32; cqt.hop_alignment()];
        while self.frames_out < total_frames {
            self.feed(cqt, &block, &mut on_frame, total_frames);
        }
    }

    fn feed<F>(&mut self, cqt: &Cqt, samples: &[f32], on_frame: &mut F, limit: u64)
    where
        F: FnMut(&[f32]),
    {
        self.levels.extend(samples);
        self.levels.propagate();
        while self.frames_out < limit {
            let centre = (self.frames_out * self.hop_size as u64) as i64;
            if !cqt.frame_ready(&self.levels, centre) {
                break;
            }
            cqt.frame_magnitudes(
                &self.levels,
                &mut self.scratch,
                centre,
                &mut self.magnitudes,
            );
            on_frame(&self.magnitudes);
            self.frames_out += 1;
        }
        self.compact(cqt);
    }

    /// Drops samples no later frame or lower level can need.
    fn compact(&mut self, cqt: &Cqt) {
        let next_centre = (self.frames_out * self.hop_size as u64) as i64;
        let num_levels = cqt.num_levels();
        let delay = cqt.decimation_taps().saturating_sub(1) as i64 / 2;
        for level in 0..num_levels {
            let mut keep_from = cqt.first_needed(level, next_centre);
            if level + 1 < num_levels {
                // The next output of level + 1 reads from 2n - delay.
                keep_from = keep_from.min(2 * self.levels.end(level + 1) - delay);
            }
            // Amortize the memmove: only drop once a sizeable prefix is dead.
            let retained = self.levels.retained(level) as i64;
            let dead = keep_from - (self.levels.end(level) - retained);
            if dead >= 4_096 && dead * 2 >= retained {
                self.levels.discard_before(level, keep_from);
            }
        }
    }
}
