# Moth Hack 2026 — Track 07 submission notes

**Project:** COLLAPSE  
**Track:** 07 Make a VST or AU (£150)  
**Format:** VST3 (+ CLAP export in the same binary)

## What it does

COLLAPSE is a stereo/mono **insert effect**. Press **COLLAPSE** to request an impulse response from an Atlas sound engine (or the built-in mock generator). The IR is applied via fixed-size realtime convolution with Mix / Drive / Tone / Size / Output controls.

## Atlas usage

| Mode | How |
|------|-----|
| `ATLAS_MODE=mock` (default) | Local deterministic IR — no network. Used for demos below. |
| `ATLAS_MODE=live` | `POST` to `ATLAS_BASE_URL` with Bearer `ATLAS_API_KEY`, engine `ATLAS_ENGINE_ID`. Accepts JSON `samples`, `wav_base64`, or `audio_url`. |

Engine path lives in `collapse/src/atlas.rs`. Point `ATLAS_BASE_URL` / `ATLAS_ENGINE_ID` at the ids shown in [Atlas](https://platform.mothquantum.com/) once confirmed on Discord.

## Deliverables

1. Plugin: build with `./scripts/bundle_vst3.sh` → `dist/COLLAPSE.vst3`
2. Audio examples: `assets/demos/collapse_demo_{vocal,drums,pad}.wav`
3. Matching IRs: `assets/demos/collapse_ir_*.wav`

## How to demo in a DAW

1. Install `COLLAPSE.vst3` into the host VST3 folder  
2. Insert on a vocal / drum / pad track  
3. Raise Mix, press **COLLAPSE** a few times — each seed yields a new IR character  
4. A/B dry vs wet for judges
