//! Smoke-test live Atlas credentials (does not print the key).

use collapse::{run_collapse, write_ir_wav, CollapseRequest};
use std::path::PathBuf;

fn main() {
    std::env::set_var("ATLAS_MODE", "live");
    let req = CollapseRequest {
        seed: 26,
        drive: 0.45,
        tone: 0.4,
        size: 0.35,
        sample_rate: 44_100.0,
    };
    println!("Calling Atlas (retrocausal-echo-v1)…");
    match run_collapse(&req) {
        Ok(result) => {
            println!("OK · mode={} · engine={}", result.mode, result.engine);
            println!("{}", result.note);
            println!("IR samples: {}", result.ir.len());
            let out = PathBuf::from("assets/demos/collapse_live_ir.wav");
            let _ = std::fs::create_dir_all(out.parent().unwrap());
            if let Err(e) = write_ir_wav(out.clone(), &result.ir, 44_100) {
                eprintln!("warn: could not write wav: {e}");
            } else {
                println!("Wrote {}", out.display());
            }
        }
        Err(e) => {
            eprintln!("FAIL: {e}");
            std::process::exit(1);
        }
    }
}
