# COLLAPSE — Moth Hack 2026 (Track 07 VST/AU)

Atlas-powered **audio effect** plugin for [Moth Hack](https://hack.mothquantum.com/) challenge **07 Make a VST or AU**.

Press **COLLAPSE** → an Atlas sound engine (or local mock) returns a short impulse response → the plugin convolves your track with it. Knobs: **Mix · Drive · Tone · Size · Output**.

## Why this entry

- Real **VST3** (and CLAP) insert effect — not a remake of Actias
- Uses an Atlas engine call path (`ATLAS_MODE=live`) with a working **mock** fallback for demos
- Background Collapse never blocks the audio thread
- Demo WAVs included under `assets/demos/`

## Build (Linux)

```bash
./scripts/setup.sh          # clones nih-plug once
cargo build --release -p collapse
./scripts/bundle_vst3.sh    # → dist/COLLAPSE.vst3
```

Copy `dist/COLLAPSE.vst3` into `~/.vst3/` (or bridge with [yabridge](https://github.com/robbert-vdh/yabridge) on Wine/Windows hosts).

### macOS AU / VST3

Build on a Mac with the same repo (`cargo build --release -p collapse`). Package with [cargo-nih-plug](https://github.com/robbert-vdh/nih-plug) / `cargo xtask bundle collapse --release` from the nih-plug tooling if you need a signed `.component` AU. The brief accepts **VST or AU** — Linux VST3 is enough to submit; add AU if you have a Mac.

## Atlas live mode

1. Create an account at [platform.mothquantum.com](https://platform.mothquantum.com/)
2. Copy `.env.example` → export vars in the shell that launches your DAW:

```bash
export ATLAS_MODE=live
export ATLAS_API_KEY=your_key
export ATLAS_ENGINE_ID=sound          # use the id from Atlas
export ATLAS_BASE_URL=https://platform.mothquantum.com/api/v1/engines/run
```

3. Adjust `ATLAS_BASE_URL` / response parsing in `collapse/src/atlas.rs` if Atlas’s public contract differs (the client accepts `samples`, `wav_base64`, or `audio_url`).

Default **`ATLAS_MODE=mock`** generates deterministic IRs so judges can hear the plugin without credentials.

## Demo audio

```bash
cargo run --release -p collapse --bin gen_demos
```

Writes:

- `assets/demos/collapse_demo_{vocal,drums,pad}.wav`
- `assets/demos/collapse_ir_{vocal,drums,pad}.wav`

## Submission checklist (track 07)

- [ ] `COLLAPSE.vst3` (or AU on Mac)
- [ ] Audio examples from `assets/demos/` (re-render in a DAW if possible)
- [ ] Short note: engine used, `ATLAS_MODE`, params
- [ ] Confirm Discord submission channel

## Project layout

```
collapse/           plugin crate (nih-plug + egui via git)
assets/demos/       submission audio
scripts/            setup + bundle
dist/               local VST3 bundle (gitignored; run bundle script)
```

## License

MIT — built for Moth Hack 2026.
