//! Regenerates the SVG figures in `plots/` from the public API.
//!
//! ```console
//! cargo run --release --example generate_plots
//! ```

use std::error::Error;
use std::f32::consts::TAU;

use cqt_rs::{Cqt, CqtParams, magnitude_to_db};
use plotters::prelude::*;

const SR: u32 = 22_050;

fn arpeggio() -> Vec<f32> {
    // A3, C#4, E4, A4 for 250 ms each, then a 1 s sweep from A4 to A6.
    let notes = [220.0f32, 277.18, 329.63, 440.0];
    let total = 2 * SR as usize;
    (0..total)
        .map(|n| {
            let t = n as f32 / SR as f32;
            let phase = if t < 1.0 {
                TAU * notes[((t * 4.0) as usize).min(3)] * t
            } else {
                // Exponential sweep: phase = 2π f0 (2^(k u) - 1) / (k ln 2)
                let u = t - 1.0;
                let k = 2.0f32;
                TAU * 440.0 * (2f32.powf(k * u) - 1.0) / (k * 2f32.ln())
            };
            0.5 * phase.sin()
        })
        .collect()
}

/// Five-stop approximation of the viridis colour map for `t` in `0..=1`.
fn viridis(t: f32) -> RGBColor {
    const STOPS: [(f32, f32, f32); 5] = [
        (68.0, 1.0, 84.0),
        (59.0, 82.0, 139.0),
        (33.0, 145.0, 140.0),
        (94.0, 201.0, 98.0),
        (253.0, 231.0, 37.0),
    ];
    let x = t.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
    let i = (x.floor() as usize).min(STOPS.len() - 2);
    let f = x - i as f32;
    let (a, b) = (STOPS[i], STOPS[i + 1]);
    RGBColor(
        (a.0 + (b.0 - a.0) * f) as u8,
        (a.1 + (b.1 - a.1) * f) as u8,
        (a.2 + (b.2 - a.2) * f) as u8,
    )
}

fn spectrogram(path: &str) -> Result<(), Box<dyn Error>> {
    let cqt = Cqt::new(
        CqtParams::builder(SR, 110.0, 3_520.0)
            .bins_per_octave(12)
            .build()?,
    );
    let hop = 1_024;
    let signal = arpeggio();
    let mut result = cqt.process(&signal, hop)?;
    for mut row in result.rows_mut() {
        magnitude_to_db(row.as_slice_mut().unwrap(), 1.0, 1e-5, None);
    }
    let (frames, bins) = result.dim();
    let seconds = signal.len() as f32 / SR as f32;

    let root = SVGBackend::new(path, (760, 420)).into_drawing_area();
    root.fill(&WHITE)?;
    let mut chart = ChartBuilder::on(&root)
        .caption("CQT magnitude (dB), 12 bins per octave", ("sans-serif", 18))
        .margin(10)
        .x_label_area_size(36)
        .y_label_area_size(56)
        .build_cartesian_2d(0f32..seconds, 0f32..bins as f32)?;
    chart
        .configure_mesh()
        .disable_mesh()
        .x_desc("time (s)")
        .y_desc("centre frequency (Hz)")
        .y_labels(8)
        .y_label_formatter(&|bin| format!("{:.0}", cqt.params().center_freq(*bin as usize)))
        .draw()?;

    let frame_seconds = hop as f32 / SR as f32;
    chart.draw_series((0..frames).flat_map(|frame| {
        let row = result.row(frame);
        (0..bins).map(move |bin| {
            let db = row[bin].clamp(-60.0, 0.0);
            let level = (db + 60.0) / 60.0;
            let colour = viridis(level);
            Rectangle::new(
                [
                    (
                        frame as f32 * frame_seconds - frame_seconds / 2.0,
                        bin as f32,
                    ),
                    (
                        frame as f32 * frame_seconds + frame_seconds / 2.0,
                        bin as f32 + 1.0,
                    ),
                ],
                colour.filled(),
            )
        })
    }))?;
    root.present()?;
    Ok(())
}

fn kernel_lengths(path: &str) -> Result<(), Box<dyn Error>> {
    let sr = 44_100;
    let configs = [
        (
            "constant-Q",
            CqtParams::builder(sr, 55.0, 7_040.0)
                .bins_per_octave(24)
                .build()?,
            BLUE,
        ),
        (
            "variable-Q, gamma = 20 Hz",
            CqtParams::builder(sr, 55.0, 7_040.0)
                .bins_per_octave(24)
                .gamma(20.0)
                .build()?,
            RED,
        ),
        (
            "constant-Q capped at 4096 samples",
            CqtParams::builder(sr, 55.0, 7_040.0)
                .bins_per_octave(24)
                .max_kernel_length(4_096)
                .build()?,
            GREEN,
        ),
    ];

    let root = SVGBackend::new(path, (760, 420)).into_drawing_area();
    root.fill(&WHITE)?;
    let mut chart = ChartBuilder::on(&root)
        .caption(
            "Analysis window length per bin at 44.1 kHz",
            ("sans-serif", 18),
        )
        .margin(10)
        .x_label_area_size(36)
        .y_label_area_size(64)
        .build_cartesian_2d(
            (50f32..8_000f32).log_scale(),
            (100f32..40_000f32).log_scale(),
        )?;
    chart
        .configure_mesh()
        .x_desc("centre frequency (Hz)")
        .y_desc("window length (samples)")
        .draw()?;
    for (label, params, colour) in configs {
        let points: Vec<(f32, f32)> = (0..params.num_bins())
            .map(|bin| (params.center_freq(bin), params.kernel_length(bin) as f32))
            .collect();
        chart
            .draw_series(LineSeries::new(points, colour.stroke_width(2)))?
            .label(label)
            .legend(move |(x, y)| PathElement::new([(x, y), (x + 20, y)], colour.stroke_width(2)));
    }
    chart
        .configure_series_labels()
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .position(SeriesLabelPosition::UpperRight)
        .draw()?;
    root.present()?;
    Ok(())
}

fn pitch_shift(path: &str) -> Result<(), Box<dyn Error>> {
    let cqt = Cqt::new(
        CqtParams::builder(SR, 55.0, 7_040.0)
            .bins_per_octave(24)
            .build()?,
    );
    let bins_per_octave = cqt.params().bins_per_octave() as f32;
    let shift = 5;
    let ratio = 2f32.powf(shift as f32 / bins_per_octave);
    let tone = |f0: f32| -> Vec<f32> {
        (0..SR as usize)
            .map(|n| {
                let t = n as f32 / SR as f32;
                (1..=5)
                    .map(|k| (TAU * f0 * k as f32 * t).sin() / k as f32)
                    .sum::<f32>()
            })
            .collect()
    };
    let row = |signal: &[f32]| -> Vec<f32> {
        let result = cqt.process(signal, 512).unwrap();
        let mut row = result.row(result.nrows() / 2).to_vec();
        magnitude_to_db(&mut row, 1.0, 1e-5, Some(60.0));
        row
    };
    let original = row(&tone(220.0));
    let shifted = row(&tone(220.0 * ratio));

    let root = SVGBackend::new(path, (760, 420)).into_drawing_area();
    root.fill(&WHITE)?;
    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!(
                "Pitch shift of {:.1} % moves every peak by exactly {shift} bins",
                (ratio - 1.0) * 100.0
            ),
            ("sans-serif", 18),
        )
        .margin(10)
        .x_label_area_size(36)
        .y_label_area_size(56)
        .build_cartesian_2d(0f32..cqt.num_bins() as f32, -60f32..2f32)?;
    chart
        .configure_mesh()
        .x_desc("bin (24 per octave)")
        .y_desc("magnitude (dB)")
        .draw()?;
    for (label, data, colour) in [
        ("220 Hz tone", original, BLUE),
        ("shifted tone", shifted, RED),
    ] {
        let points: Vec<(f32, f32)> = data
            .iter()
            .enumerate()
            .map(|(bin, &db)| (bin as f32, db))
            .collect();
        chart
            .draw_series(LineSeries::new(points, colour.stroke_width(2)))?
            .label(label)
            .legend(move |(x, y)| PathElement::new([(x, y), (x + 20, y)], colour.stroke_width(2)));
    }
    chart
        .configure_series_labels()
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .draw()?;
    root.present()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all("plots")?;
    spectrogram("plots/cqt_spectrogram.svg")?;
    kernel_lengths("plots/kernel_lengths.svg")?;
    pitch_shift("plots/pitch_shift.svg")?;
    println!("wrote plots/cqt_spectrogram.svg, plots/kernel_lengths.svg, plots/pitch_shift.svg");
    Ok(())
}
