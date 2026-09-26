//! Atlas engine client for COLLAPSE.
//!
//! Default mode is `mock`: generates a deterministic impulse response locally so the
//! plugin is demoable without credentials. Set `ATLAS_MODE=live` plus `ATLAS_API_KEY`
//! to call a real Atlas HTTP endpoint (configurable via env).

use hound::{SampleFormat, WavSpec, WavWriter};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Deserialize;
use std::env;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_IR_LEN: usize = 4096;

#[derive(Debug, Clone)]
pub struct CollapseRequest {
    pub seed: u32,
    pub drive: f32,
    pub tone: f32,
    pub size: f32,
    pub sample_rate: f32,
}

#[derive(Debug, Clone)]
pub struct CollapseResult {
    pub ir: Vec<f32>,
    pub engine: String,
    pub mode: String,
    pub note: String,
}

#[derive(Debug)]
pub enum AtlasError {
    Http(String),
    Decode(String),
    Config(String),
}

impl std::fmt::Display for AtlasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(s) | Self::Decode(s) | Self::Config(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct LiveResponse {
    #[serde(default)]
    samples: Option<Vec<f32>>,
    #[serde(default)]
    wav_base64: Option<String>,
    #[serde(default)]
    audio_url: Option<String>,
    #[serde(default)]
    engine: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

pub fn run_collapse(req: &CollapseRequest) -> Result<CollapseResult, AtlasError> {
    let mode = env::var("ATLAS_MODE").unwrap_or_else(|_| "mock".into());
    match mode.to_ascii_lowercase().as_str() {
        "live" => run_live(req),
        _ => Ok(run_mock(req)),
    }
}

fn run_mock(req: &CollapseRequest) -> CollapseResult {
    let len = ((DEFAULT_IR_LEN as f32) * (0.35 + 0.65 * req.size.clamp(0.0, 1.0))) as usize;
    let len = len.clamp(256, DEFAULT_IR_LEN);
    let mut rng = StdRng::seed_from_u64(req.seed as u64);

    let brightness = req.tone.clamp(0.0, 1.0);
    let drive = req.drive.clamp(0.0, 1.0);
    let mut ir = Vec::with_capacity(len);

    for n in 0..len {
        let t = n as f32 / req.sample_rate.max(1.0);
        // Multi-rate decay + filtered noise → musically useful short IR
        let env = (-t * (3.5 + (1.0 - req.size) * 18.0)).exp();
        let noise = rng.gen::<f32>() * 2.0 - 1.0;
        let tone_hpf = if n == 0 {
            0.0
        } else {
            noise - ir[n - 1] * (0.15 + 0.7 * (1.0 - brightness))
        };
        let harmonic = (t * (120.0 + brightness * 1800.0) * std::f32::consts::TAU).sin()
            * (0.15 + 0.55 * drive);
        let sample = (tone_hpf * (0.35 + 0.4 * brightness) + harmonic) * env;
        ir.push(sample.tanh() * (0.55 + 0.45 * drive));
    }

    // Normalize
    let peak = ir.iter().map(|s| s.abs()).fold(0.0_f32, f32::max).max(1e-6);
    for s in &mut ir {
        *s /= peak;
    }

    CollapseResult {
        ir,
        engine: env::var("ATLAS_ENGINE_ID").unwrap_or_else(|_| "mock-sound".into()),
        mode: "mock".into(),
        note: format!(
            "Mock Atlas IR · seed={} · drive={:.2} · tone={:.2} · size={:.2}",
            req.seed, req.drive, req.tone, req.size
        ),
    }
}

fn run_live(req: &CollapseRequest) -> Result<CollapseResult, AtlasError> {
    let api_key = env::var("ATLAS_API_KEY")
        .map_err(|_| AtlasError::Config("ATLAS_API_KEY is not set".into()))?;
    let base = env::var("ATLAS_BASE_URL").unwrap_or_else(|_| {
        "https://platform.mothquantum.com/api/v1/engines/run".into()
    });
    let engine = env::var("ATLAS_ENGINE_ID").unwrap_or_else(|_| "sound".into());

    let body = serde_json::json!({
        "engine": engine,
        "task": "impulse_response",
        "params": {
            "seed": req.seed,
            "drive": req.drive,
            "tone": req.tone,
            "size": req.size,
            "sample_rate": req.sample_rate,
            "length": DEFAULT_IR_LEN,
        }
    });

    let response = ureq::post(&base)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .send_json(body)
        .map_err(|e| AtlasError::Http(e.to_string()))?;

    let parsed: LiveResponse = response
        .into_json()
        .map_err(|e| AtlasError::Decode(e.to_string()))?;

    if let Some(samples) = parsed.samples {
        if samples.is_empty() {
            return Err(AtlasError::Decode("empty samples array".into()));
        }
        return Ok(CollapseResult {
            ir: samples,
            engine: parsed.engine.unwrap_or(engine),
            mode: "live".into(),
            note: parsed
                .message
                .unwrap_or_else(|| "Live Atlas samples".into()),
        });
    }

    if let Some(b64) = parsed.wav_base64 {
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
            .map_err(|e| AtlasError::Decode(e.to_string()))?;
        let ir = decode_wav_mono(&bytes)?;
        return Ok(CollapseResult {
            ir,
            engine: parsed.engine.unwrap_or(engine),
            mode: "live".into(),
            note: parsed
                .message
                .unwrap_or_else(|| "Live Atlas wav_base64".into()),
        });
    }

    if let Some(url) = parsed.audio_url {
        let response = ureq::get(&url)
            .call()
            .map_err(|e| AtlasError::Http(e.to_string()))?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| AtlasError::Decode(e.to_string()))?;
        let ir = decode_wav_mono(&bytes)?;
        return Ok(CollapseResult {
            ir,
            engine: parsed.engine.unwrap_or(engine),
            mode: "live".into(),
            note: parsed
                .message
                .unwrap_or_else(|| format!("Live Atlas audio_url {url}")),
        });
    }

    Err(AtlasError::Decode(
        "Live response missing samples, wav_base64, or audio_url — check ATLAS_BASE_URL / engine contract"
            .into(),
    ))
}

fn decode_wav_mono(bytes: &[u8]) -> Result<Vec<f32>, AtlasError> {
    let mut reader =
        hound::WavReader::new(Cursor::new(bytes)).map_err(|e| AtlasError::Decode(e.to_string()))?;
    let spec = reader.spec();
    let mut mono = Vec::new();

    match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, 32) => {
            for s in reader.samples::<f32>() {
                mono.push(s.map_err(|e| AtlasError::Decode(e.to_string()))?);
            }
        }
        (SampleFormat::Int, 16) => {
            for s in reader.samples::<i16>() {
                let v = s.map_err(|e| AtlasError::Decode(e.to_string()))?;
                mono.push(v as f32 / i16::MAX as f32);
            }
        }
        (SampleFormat::Int, 24) | (SampleFormat::Int, 32) => {
            for s in reader.samples::<i32>() {
                let v = s.map_err(|e| AtlasError::Decode(e.to_string()))?;
                mono.push(v as f32 / i32::MAX as f32);
            }
        }
        _ => {
            return Err(AtlasError::Decode(format!(
                "unsupported wav format: {:?} {}-bit",
                spec.sample_format, spec.bits_per_sample
            )))
        }
    }

    if spec.channels > 1 {
        let ch = spec.channels as usize;
        let frames = mono.len() / ch;
        let mut down = Vec::with_capacity(frames);
        for i in 0..frames {
            let mut acc = 0.0;
            for c in 0..ch {
                acc += mono[i * ch + c];
            }
            down.push(acc / ch as f32);
        }
        mono = down;
    }

    if mono.len() > DEFAULT_IR_LEN {
        mono.truncate(DEFAULT_IR_LEN);
    }
    Ok(mono)
}

/// Write an IR to disk for demo packaging / judging.
pub fn write_ir_wav(path: PathBuf, ir: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &s in ir {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(v).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

pub fn next_seed() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.as_nanos() as u32) ^ (d.as_secs() as u32))
        .unwrap_or(1)
}
