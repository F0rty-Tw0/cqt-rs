use criterion::{ black_box, Criterion };
use cqt_rs::{ create_dummy_audio_signal, CQTParams, Cqt };

pub fn bench_cqt_process(criterion: &mut Criterion) {
  const MIN_FREQ: f32 = 14.568; // A#/Bb-1
  const MAX_FREQ: f32 = 7902.1; // B8
  const BINS_PER_OCTAVE: usize = 12;
  const SAMPLE_RATE: usize = 22000;
  const WINDOW_LENGTH: usize = 2000;

  let cqt_params = CQTParams::new(
    MIN_FREQ,
    MAX_FREQ,
    BINS_PER_OCTAVE,
    SAMPLE_RATE,
    WINDOW_LENGTH
  ).unwrap();

  let dummy_audio_signal = create_dummy_audio_signal(SAMPLE_RATE, 440.0, 30.0);
  let cqt = Cqt::new(cqt_params);

  criterion.bench_function("bench_cqt_process", |bencher| {
    bencher.iter(|| { black_box(cqt.process(&dummy_audio_signal, 1760)) })
  });
}