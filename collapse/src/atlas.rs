//! Moth Atlas client for COLLAPSE.
//!
//! Live mode talks to https://api.mothquantum.com using a `moth_` API key:
//!   POST /api/v1/engines/{engine}/process
//!   GET  /api/v1/jobs/{id}/status
//!   GET  /api/v1/jobs/{id}/result  → download `outputs[].url` (slot `result`)
//!
//! Default engine: `retrocausal-echo-v1` (no audio input → stereo IR WAV for convolution).
//! Docs: https://api.mothquantum.com/docs

use hound::{SampleFormat, WavSpec, WavWriter};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Deserialize;
use serde_json::json;
use std::env;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_IR_LEN: usize = 4096;
const USER_AGENT: &str = "collapse-vst/0.1.0 (moth-hack-07)";
const DEFAULT_BASE: &str = "https://api.mothquantum.com";
const DEFAULT_ENGINE: &str = "retrocausal-echo-v1";

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
    Job(String),
}

impl std::fmt::Display for AtlasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(s) | Self::Decode(s) | Self::Config(s) | Self::Job(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ProcessResponse {
    #[serde(default)]
    job_id: Option<String>,
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JobStatus {
    status: String,
    #[serde(default)]
    error: Option<JobError>,
}

#[derive(Debug, Deserialize)]
struct JobError {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    detail: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JobResultBody {
    #[serde(default)]
    outputs: Vec<JobOutput>,
}

#[derive(Debug, Deserialize)]
struct JobOutput {
    #[serde(default)]
    slot: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

pub fn run_collapse(req: &CollapseRequest) -> Result<CollapseResult, AtlasError> {
    let mode = env::var("ATLAS_MODE").unwrap_or_else(|_| {
        // If a key is already configured, prefer live so DAW users don't forget ATLAS_MODE.
        if resolve_api_key().is_ok() {
            "live".into()
        } else {
            "mock".into()
        }
    });
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

    let peak = ir.iter().map(|s| s.abs()).fold(0.0_f32, f32::max).max(1e-6);
    for s in &mut ir {
        *s /= peak;
    }

    CollapseResult {
        ir,
        engine: "mock-sound".into(),
        mode: "mock".into(),
        note: format!(
            "Mock IR · seed={} · drive={:.2} · tone={:.2} · size={:.2}",
            req.seed, req.drive, req.tone, req.size
        ),
    }
}

fn resolve_api_key() -> Result<String, AtlasError> {
    if let Ok(k) = env::var("ATLAS_API_KEY") {
        let k = k.trim().to_string();
        if !k.is_empty() {
            return Ok(k);
        }
    }
    // Fallback so DAWs launched from a GUI still find the key.
    let candidates = [
        dirs_next_home().map(|h| h.join(".config/collapse/atlas_key")),
        dirs_next_home().map(|h| h.join(".collapse_atlas_key")),
        Some(PathBuf::from(".atlas_key")),
    ];
    for path in candidates.into_iter().flatten() {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            let k = contents.trim().to_string();
            if !k.is_empty() {
                return Ok(k);
            }
        }
    }
    Err(AtlasError::Config(
        "No API key — set ATLAS_API_KEY or write key to ~/.config/collapse/atlas_key".into(),
    ))
}

fn dirs_next_home() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn run_live(req: &CollapseRequest) -> Result<CollapseResult, AtlasError> {
    let api_key = resolve_api_key()?;
    let base = env::var("ATLAS_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE.into());
    let base = base.trim_end_matches('/').to_string();
    let engine = env::var("ATLAS_ENGINE_ID").unwrap_or_else(|_| DEFAULT_ENGINE.into());

    let drive = req.drive.clamp(0.0, 1.0);
    let tone = req.tone.clamp(0.0, 1.0);
    let size = req.size.clamp(0.0, 1.0);

    // Map plugin knobs → retrocausal-echo-v1 params (no audio → IR WAV).
    let theta_x = drive * std::f32::consts::PI;
    let theta_z = tone * 1.2;
    let depth = (4.0 + size * 12.0).round() as i32; // 4..16
    let n_sites = (6.0 + size * 10.0).round() as i32; // 6..16
    let ir_seconds = (1.5 + size * 3.5).clamp(1.0, 5.0);
    let negative_mode = if tone > 0.55 { "phase" } else { "invert" };

    let body = json!({
        "params": {
            "seed": req.seed as i64,
            "n_sites": n_sites,
            "depth": depth,
            "theta_x": theta_x,
            "theta_z": theta_z,
            "negative_mode": negative_mode,
            "ir_seconds": ir_seconds,
            "lattice": "chain",
            "sr": req.sample_rate.round() as i32,
            "emit": "audio",
            "include_tap_map": false,
        }
    });

    let process_url = format!("{base}/api/v1/engines/{engine}/process");
    let process_resp = ureq::post(&process_url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .set("User-Agent", USER_AGENT)
        .send_json(body)
        .map_err(|e| AtlasError::Http(format!("process: {e}")))?;

    let status = process_resp.status();
    let process_text = process_resp
        .into_string()
        .map_err(|e| AtlasError::Decode(e.to_string()))?;
    if !(200..300).contains(&status) {
        return Err(AtlasError::Http(format!(
            "process HTTP {status}: {}",
            truncate(&process_text, 400)
        )));
    }

    let parsed: ProcessResponse =
        serde_json::from_str(&process_text).map_err(|e| AtlasError::Decode(e.to_string()))?;
    let job_id = parsed
        .job_id
        .or(parsed.id)
        .ok_or_else(|| AtlasError::Decode("process response missing job_id".into()))?;

    // Poll jobs/{id}/status
    let mut last_status = String::new();
    for _ in 0..90 {
        thread::sleep(Duration::from_millis(1500));
        let st_url = format!("{base}/api/v1/jobs/{job_id}/status");
        let st_resp = ureq::get(&st_url)
            .set("Authorization", &format!("Bearer {api_key}"))
            .set("Accept", "application/json")
            .set("User-Agent", USER_AGENT)
            .call()
            .map_err(|e| AtlasError::Http(format!("status: {e}")))?;
        let st_code = st_resp.status();
        let st_text = st_resp
            .into_string()
            .map_err(|e| AtlasError::Decode(e.to_string()))?;
        if st_code == 401 || st_code == 403 {
            return Err(AtlasError::Http(format!("status HTTP {st_code}: {st_text}")));
        }
        let st: JobStatus =
            serde_json::from_str(&st_text).map_err(|e| AtlasError::Decode(e.to_string()))?;
        last_status = st.status.clone();
        match st.status.as_str() {
            "completed" => break,
            "failed" | "cancelled" => {
                let msg = st
                    .error
                    .as_ref()
                    .and_then(|e| e.message.clone().or(e.detail.clone()))
                    .unwrap_or_else(|| st.status.clone());
                return Err(AtlasError::Job(format!("job {job_id} {msg}")));
            }
            _ => continue,
        }
    }
    if last_status != "completed" {
        return Err(AtlasError::Job(format!(
            "job {job_id} timed out (last status: {last_status})"
        )));
    }

    let result_url = format!("{base}/api/v1/jobs/{job_id}/result");
    let result_resp = ureq::get(&result_url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Accept", "application/json")
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| AtlasError::Http(format!("result: {e}")))?;
    let result_text = result_resp
        .into_string()
        .map_err(|e| AtlasError::Decode(e.to_string()))?;
    let result_body: JobResultBody =
        serde_json::from_str(&result_text).map_err(|e| AtlasError::Decode(e.to_string()))?;

    let wav_url = result_body
        .outputs
        .iter()
        .find(|o| o.slot.as_deref() == Some("result"))
        .or_else(|| result_body.outputs.first())
        .and_then(|o| o.url.clone())
        .ok_or_else(|| AtlasError::Decode("job result missing outputs[].url".into()))?;

    let wav_resp = ureq::get(&wav_url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| AtlasError::Http(format!("download wav: {e}")))?;
    let mut bytes = Vec::new();
    wav_resp
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| AtlasError::Decode(e.to_string()))?;

    let ir = decode_wav_mono(&bytes)?;
    Ok(CollapseResult {
        ir,
        engine: engine.clone(),
        mode: "live".into(),
        note: format!(
            "Atlas {engine} · job={job_id} · seed={} · θx={theta_x:.2} · depth={depth} · ir={ir_seconds:.1}s",
            req.seed
        ),
    })
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
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
        // Keep the energetic head of a long IR for the realtime convolver budget.
        mono.truncate(DEFAULT_IR_LEN);
    }
    if mono.is_empty() {
        return Err(AtlasError::Decode("decoded IR is empty".into()));
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
