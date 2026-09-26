# Moth Hack 2026 — Track 07 submission notes

**Project:** COLLAPSE  
**Track:** 07 Make a VST or AU (£150)  
**Format:** VST3 (+ CLAP export in the same binary)

## What it does

COLLAPSE is a stereo/mono **insert effect**. Press **COLLAPSE** to request an impulse response from an Atlas sound engine (or the built-in mock generator). The IR is applied via fixed-size realtime convolution with Mix / Drive / Tone / Size / Output controls.

## Atlas usage

| Mode | How |
|------|-----|
| `ATLAS_MODE=mock` (default) | Local deterministic IR — no network. |
| `ATLAS_MODE=live` | Real Moth API: `retrocausal-echo-v1` measures a quantum IR and returns a WAV used as the convolver. |

Live flow (`collapse/src/atlas.rs`):

1. `POST https://api.mothquantum.com/api/v1/engines/retrocausal-echo-v1/process`
2. Poll `GET /api/v1/jobs/{id}/status`
3. `GET /api/v1/jobs/{id}/result` → download `outputs[slot=result].url`

Auth: `Authorization: Bearer moth_…`  
Store key with `./scripts/set_atlas_key.sh` (do not commit the key).

## Deliverables

1. Plugin: build with `./scripts/bundle_vst3.sh` → `dist/COLLAPSE.vst3`
2. Audio examples: `assets/demos/collapse_demo_{vocal,drums,pad}.wav`
3. Matching IRs: `assets/demos/collapse_ir_*.wav`

## How to demo in a DAW

1. Install `COLLAPSE.vst3` into the host VST3 folder  
2. Insert on a vocal / drum / pad track  
3. Raise Mix, press **COLLAPSE** a few times — each seed yields a new IR character  
4. A/B dry vs wet for judges
