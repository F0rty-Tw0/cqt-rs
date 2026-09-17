//! Push-based streaming transform for real-time use.

use crate::cqt::{Cqt, CqtError, Scratch};
use crate::params::CqtParams;
use halfband_rs::Levels;

/// Incremental transform that consumes audio in arbitrary chunks and emits a
/// frame every `hop_size` samples.
///
/// Frames are numbered from the first sample pushed. Frame `i` is centred on
/// sample `i * hop_size`, exactly as in [`Cqt::process`], and is emitted as
/// soon as sample `i * hop_size + latency - 1` has been pushed, where
/// `latency` is [`Cqt::latency_samples`]. Pushing performs no allocation
/// once the internal buffers have grown to their steady-state size.
///
/// The hop can be changed mid-stream with [`set_hop`](CqtStream::set_hop),
/// and with [`retain_samples`](CqtStream::retain_samples) the recent past
/// can be re-analysed on a different grid with
/// [`replay`](CqtStream::replay). After a hop change the frame count is
/// simply the number of frames emitted; use
/// [`next_centre`](CqtStream::next_centre) to place frames in time.
///
/// A stream is bound to the transform it was created with (or last
/// [`reset`](CqtStream::reset) to): every other method panics when given
/// a transform with different parameters.
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
    /// Parameters of the transform this stream's state belongs to.
    params: CqtParams,
    hop_size: usize,
    latency: usize,
    levels: Levels,
    /// Samples pushed since creation or the last reset.
    samples_in: u64,
    /// Zeros appended by [`CqtStream::flush`] that audio has since been
    /// pushed after; they are part of the frame timeline.
    padding: u64,
    /// Zeros appended by [`CqtStream::flush`] with no audio after them
    /// yet. They join `padding` on the next push, so that repeating a
    /// flush emits nothing new.
    padding_pending: u64,
    frames_out: u64,
    /// Centre of the next frame to emit, in full-rate samples.
    next_centre: i64,
    /// Full-rate samples of history to keep behind `next_centre`.
    retain: i64,
    scratch: Scratch,
    magnitudes: Vec<f32>,
}

impl CqtStream {
    /// Creates a stream that emits one frame every `hop_size` samples.
    pub fn new(cqt: &Cqt, hop_size: usize) -> Result<Self, CqtError> {
        cqt.check_hop_size(hop_size)?;
        Ok(Self {
            params: cqt.params().clone(),
            hop_size,
            latency: cqt.latency_samples(),
            levels: cqt.levels(),
            samples_in: 0,
            padding: 0,
            padding_pending: 0,
            frames_out: 0,
            next_centre: 0,
            retain: 0,
            scratch: cqt.scratch(),
            magnitudes: vec![0.0; cqt.num_bins()],
        })
    }

    /// Parameters of the transform this stream is bound to.
    pub fn params(&self) -> &CqtParams {
        &self.params
    }

    fn check_bound(&self, cqt: &Cqt) {
        assert!(
            *cqt.params() == self.params,
            "CqtStream used with a transform it was not created for; call reset first"
        );
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
    ///
    /// Until the first [`set_hop`](CqtStream::set_hop) this is also the
    /// index of the next frame on the `i * hop_size` grid; afterwards it
    /// is only a count, see [`next_centre`](CqtStream::next_centre).
    pub fn frames_emitted(&self) -> u64 {
        self.frames_out
    }

    /// Centre of the next frame to be emitted, in samples from the first
    /// sample pushed. The frames of a push are centred on this value and
    /// every following multiple of the hop.
    pub fn next_centre(&self) -> i64 {
        self.next_centre
    }

    /// Number of samples pushed so far, not counting the padding that
    /// [`CqtStream::flush`] appends.
    pub fn samples_consumed(&self) -> u64 {
        self.samples_in
    }

    /// Number of zero samples appended by [`CqtStream::flush`] so far.
    pub fn padding_samples(&self) -> u64 {
        self.padding + self.padding_pending
    }

    /// Keeps at least `samples` samples of history behind the next frame
    /// centre, so that [`replay`](CqtStream::replay) can re-analyse them.
    ///
    /// Every level keeps its share (`samples >> level`) plus the window of
    /// the frame centred `samples` before the next one, which costs about
    /// `2 * samples * 4` bytes across the levels, up to half again more
    /// between compactions. History already dropped is not recovered. The
    /// default is zero: only what future frames need is kept.
    pub fn retain_samples(&mut self, samples: usize) {
        self.retain = samples as i64;
    }

    /// Changes the hop for the frames that follow.
    ///
    /// The next frame is centred on the smallest multiple of `hop_size`
    /// that is not before the centre the stream would have emitted next,
    /// so centres stay increasing across the change. Afterwards
    /// [`frames_emitted`](CqtStream::frames_emitted) no longer equals
    /// `centre / hop_size`; read [`next_centre`](CqtStream::next_centre)
    /// to place frames in time.
    ///
    /// # Panics
    ///
    /// If `cqt` is not the transform the stream is bound to.
    pub fn set_hop(&mut self, cqt: &Cqt, hop_size: usize) -> Result<(), CqtError> {
        self.check_bound(cqt);
        cqt.check_hop_size(hop_size)?;
        self.hop_size = hop_size;
        self.next_centre = grid_ceil(self.next_centre, hop_size);
        Ok(())
    }

    /// Forgets all buffered audio, restarts frame numbering and binds the
    /// stream to `cqt`, which may differ from the transform it was created
    /// with; the hop size and retention are kept.
    pub fn reset(&mut self, cqt: &Cqt) {
        if *cqt.params() != self.params {
            self.params = cqt.params().clone();
            self.latency = cqt.latency_samples();
            self.scratch = cqt.scratch();
            self.magnitudes = vec![0.0; cqt.num_bins()];
        }
        self.levels = cqt.levels();
        self.samples_in = 0;
        self.padding = 0;
        self.padding_pending = 0;
        self.frames_out = 0;
        self.next_centre = 0;
    }

    /// Feeds `samples` and calls `on_frame` with the magnitudes of every
    /// frame that became complete, in order.
    ///
    /// # Panics
    ///
    /// If `cqt` is not the transform the stream is bound to.
    pub fn push<F>(&mut self, cqt: &Cqt, samples: &[f32], mut on_frame: F)
    where
        F: FnMut(&[f32]),
    {
        self.check_bound(cqt);
        if !samples.is_empty() {
            self.padding += self.padding_pending;
            self.padding_pending = 0;
        }
        self.samples_in += samples.len() as u64;
        self.feed(cqt, samples, &mut on_frame, i64::MAX);
    }

    /// Emits the frames whose centre lies inside the audio pushed so far by
    /// zero padding the end, matching [`Cqt::process`] on the same signal.
    ///
    /// Flushing again without pushing emits nothing. The padding stays in
    /// the buffer, and pushing more audio afterwards behaves exactly as if
    /// that silence had been pushed: it joins the frame timeline, so the
    /// frames match [`Cqt::process`] on the audio with the silence
    /// inserted (see [`CqtStream::padding_samples`]). Call
    /// [`CqtStream::reset`] before reusing the stream for unrelated audio.
    ///
    /// # Panics
    ///
    /// If `cqt` is not the transform the stream is bound to.
    pub fn flush<F>(&mut self, cqt: &Cqt, mut on_frame: F)
    where
        F: FnMut(&[f32]),
    {
        self.check_bound(cqt);
        let last_centre = (self.samples_in + self.padding) as i64;
        let block = vec![0.0f32; cqt.hop_alignment()];
        while self.next_centre <= last_centre {
            self.padding_pending += block.len() as u64;
            self.feed(cqt, &block, &mut on_frame, last_centre);
        }
    }

    /// Re-analyses the retained history on a grid of `hop_size` samples.
    ///
    /// Calls `on_frame` with the centre and magnitudes of every frame whose
    /// centre is a multiple of `hop_size` in `from_sample..next_centre`
    /// and whose window is still buffered on every level, oldest first.
    /// Frames whose history was dropped are skipped, so the replay may
    /// start later than `from_sample`; see
    /// [`retain_samples`](CqtStream::retain_samples). Returns the centre
    /// of the first frame emitted, or `None` if there was none. The
    /// frames equal the rows of [`Cqt::process`] at the same centres.
    ///
    /// Only the scratch buffers are touched: the audio, hop and frame
    /// count are unchanged.
    ///
    /// # Panics
    ///
    /// If `cqt` is not the transform the stream is bound to.
    pub fn replay<F>(
        &mut self,
        cqt: &Cqt,
        from_sample: i64,
        hop_size: usize,
        mut on_frame: F,
    ) -> Result<Option<i64>, CqtError>
    where
        F: FnMut(i64, &[f32]),
    {
        self.check_bound(cqt);
        cqt.check_hop_size(hop_size)?;
        let mut first = None;
        let mut centre = grid_ceil(from_sample.max(0), hop_size);
        while centre < self.next_centre {
            let buffered = (0..cqt.num_levels())
                .all(|level| cqt.first_needed(level, centre) >= self.levels.history_from(level));
            if buffered && cqt.frame_ready(&self.levels, centre) {
                cqt.frame_magnitudes(
                    &self.levels,
                    &mut self.scratch,
                    centre,
                    &mut self.magnitudes,
                );
                on_frame(centre, &self.magnitudes);
                first.get_or_insert(centre);
            }
            centre += hop_size as i64;
        }
        Ok(first)
    }

    /// Emits every ready frame centred no later than `last_centre`.
    fn feed<F>(&mut self, cqt: &Cqt, samples: &[f32], on_frame: &mut F, last_centre: i64)
    where
        F: FnMut(&[f32]),
    {
        self.levels.extend(samples);
        self.levels.propagate();
        while self.next_centre <= last_centre {
            if !cqt.frame_ready(&self.levels, self.next_centre) {
                break;
            }
            cqt.frame_magnitudes(
                &self.levels,
                &mut self.scratch,
                self.next_centre,
                &mut self.magnitudes,
            );
            on_frame(&self.magnitudes);
            self.frames_out += 1;
            self.next_centre += self.hop_size as i64;
        }
        self.compact(cqt);
    }

    /// Drops samples no later frame, lower level or replay can need.
    fn compact(&mut self, cqt: &Cqt) {
        let num_levels = cqt.num_levels();
        let delay = cqt.decimation_taps().saturating_sub(1) as i64 / 2;
        for level in 0..num_levels {
            let mut keep_from = cqt.first_needed(level, self.next_centre);
            if self.retain > 0 {
                keep_from = keep_from.min(cqt.first_needed(level, self.next_centre - self.retain));
            }
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

/// Smallest multiple of `hop` that is not below `centre`.
fn grid_ceil(centre: i64, hop: usize) -> i64 {
    let hop = hop as i64;
    centre.div_euclid(hop) * hop + if centre.rem_euclid(hop) == 0 { 0 } else { hop }
}
