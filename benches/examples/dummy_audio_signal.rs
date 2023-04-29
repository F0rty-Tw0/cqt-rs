use std::f32::consts::PI;

pub fn create_dummy_audio_signal(sample_rate: usize, frequency: f32, duration: f32) -> Vec<f32> {
  let num_samples = ((sample_rate as f32) * duration) as usize;
  let time: Vec<f32> = (0..num_samples).map(|i| (i as f32) / (sample_rate as f32)).collect();
  let final_frequency = 440.0;

  time
    .iter()
    .map(|t| {
      let progressing_frequency = frequency + (t / duration) * (final_frequency - frequency);
      (2.0 * PI * progressing_frequency * t).sin()
    })
    .collect()
}