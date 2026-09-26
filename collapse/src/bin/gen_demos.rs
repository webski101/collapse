//! Generate demo IRs + short wet renders for Moth Hack submission packaging.

use collapse::{run_collapse, write_ir_wav, CollapseRequest};
use std::f32::consts::TAU;
use std::path::PathBuf;

fn render_loop(ir: &[f32], seconds: f32, sr: f32, kind: &str) -> Vec<f32> {
    let n = (seconds * sr) as usize;
    let mut out = vec![0.0; n];
    let mut hist = vec![0.0; ir.len().max(1)];
    let mut pos = 0usize;
    for i in 0..n {
        let t = i as f32 / sr;
        let dry = match kind {
            "drums" => {
                let kick = (-t * 12.0).exp() * (t * 80.0 * TAU).sin();
                let hat = if (i / (sr as usize / 8)) % 2 == 0 {
                    (rand_noise(i) * (-((t * 8.0) % 0.125) * 40.0).exp()) * 0.3
                } else {
                    0.0
                };
                kick * 0.8 + hat
            }
            "vocal" => {
                let f = 180.0 + 40.0 * (t * 2.0).sin();
                (t * f * TAU).sin() * 0.35 * (0.5 + 0.5 * (t * 3.0 * TAU).sin())
            }
            _ => {
                let f = 220.0 * (1.0 + 0.02 * (t * 0.5).sin());
                (t * f * TAU).sin() * 0.4
            }
        };
        hist[pos] = dry;
        let mut acc = 0.0;
        let mut idx = pos;
        for tap in ir {
            acc += hist[idx] * *tap;
            if idx == 0 {
                idx = hist.len() - 1;
            } else {
                idx -= 1;
            }
        }
        pos = (pos + 1) % hist.len();
        let mix = 0.55;
        out[i] = (dry * (1.0 - mix) + acc * mix).tanh() * 0.9;
    }
    out
}

fn rand_noise(i: usize) -> f32 {
    let x = i.wrapping_mul(374761393).wrapping_add(668265263);
    let x = (x ^ (x >> 13)).wrapping_mul(1274126177);
    ((x >> 9) as f32 / 8388608.0) - 1.0
}

fn write_mono_wav(path: PathBuf, samples: &[f32], sr: u32) -> Result<(), String> {
    use hound::{SampleFormat, WavSpec, WavWriter};
    let spec = WavSpec {
        channels: 1,
        sample_rate: sr,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut w = WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        w.write_sample(v).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

fn main() -> Result<(), String> {
    std::env::set_var("ATLAS_MODE", "mock");
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets/demos");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let sr = 44_100.0;

    let presets = [
        ("vocal", 11u32, 0.4, 0.7, 0.45),
        ("drums", 42, 0.55, 0.35, 0.6),
        ("pad", 99, 0.3, 0.8, 0.85),
    ];

    for (name, seed, drive, tone, size) in presets {
        let result = run_collapse(&CollapseRequest {
            seed,
            drive,
            tone,
            size,
            sample_rate: sr,
        })
        .map_err(|e| e.to_string())?;
        let ir_path = out.join(format!("collapse_ir_{name}.wav"));
        write_ir_wav(ir_path.clone(), &result.ir, sr as u32)?;
        let wet = render_loop(&result.ir, 4.0, sr, name);
        let wet_path = out.join(format!("collapse_demo_{name}.wav"));
        write_mono_wav(wet_path.clone(), &wet, sr as u32)?;
        println!("wrote {} and {}", ir_path.display(), wet_path.display());
    }

    std::fs::write(
        out.join("README.txt"),
        "COLLAPSE demos for Moth Hack track 07\n\
         - collapse_demo_*.wav : source through COLLAPSE wet path (mock Atlas IR)\n\
         - collapse_ir_*.wav   : the impulse responses used\n\
         Set ATLAS_MODE=live and ATLAS_API_KEY for real Atlas engines in the plugin.\n",
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}
