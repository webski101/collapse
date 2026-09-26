mod atlas;
mod dsp;

use atlas::next_seed;
use dsp::{CollapseDsp, MAX_IR};
use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Color32, RichText, Vec2},
    resizable_window::ResizableWindow,
    widgets, EguiState,
};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;

pub use atlas::{run_collapse, write_ir_wav, CollapseRequest, CollapseResult};

/// Shared UI / worker status: 0 idle, 1 running, 2 ready, 3 error
const STATUS_IDLE: u8 = 0;
const STATUS_RUNNING: u8 = 1;
const STATUS_READY: u8 = 2;
const STATUS_ERROR: u8 = 3;

struct IrSlot {
    samples: Mutex<[f32; MAX_IR]>,
    len: AtomicUsize,
    dirty: AtomicBool,
}

impl Default for IrSlot {
    fn default() -> Self {
        Self {
            samples: Mutex::new([0.0; MAX_IR]),
            len: AtomicUsize::new(0),
            dirty: AtomicBool::new(false),
        }
    }
}

impl IrSlot {
    fn publish(&self, ir: &[f32]) {
        let n = ir.len().min(MAX_IR);
        {
            let mut buf = self.samples.lock();
            buf[..n].copy_from_slice(&ir[..n]);
            if n < MAX_IR {
                buf[n..].fill(0.0);
            }
        }
        self.len.store(n, Ordering::Release);
        self.dirty.store(true, Ordering::Release);
    }

    /// Audio-thread safe apply into DSP (no heap alloc/free).
    fn try_apply(&self, dsp: &mut CollapseDsp) {
        if !self.dirty.swap(false, Ordering::AcqRel) {
            return;
        }
        let n = self.len.load(Ordering::Acquire).min(MAX_IR);
        if let Some(buf) = self.samples.try_lock() {
            dsp.set_ir(&buf[..n]);
        } else {
            // Try again next block
            self.dirty.store(true, Ordering::Release);
        }
    }
}

pub struct Collapse {
    params: Arc<CollapseParams>,
    dsp: CollapseDsp,
    sample_rate: Arc<Mutex<f32>>,
    ir_slot: Arc<IrSlot>,
    status: Arc<AtomicU8>,
    status_note: Arc<Mutex<String>>,
    collapse_busy: Arc<AtomicBool>,
}

#[derive(Params)]
pub struct CollapseParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "tone"]
    pub tone: FloatParam,
    #[id = "size"]
    pub size: FloatParam,
    #[id = "out"]
    pub output: FloatParam,
}

/// Background tasks must stay stack-only (no Vec/Box in the enum).
#[derive(Clone, Copy)]
pub enum CollapseTask {
    Run { seed: u32 },
}

impl Default for Collapse {
    fn default() -> Self {
        Self {
            params: Arc::new(CollapseParams::default()),
            dsp: CollapseDsp::default(),
            sample_rate: Arc::new(Mutex::new(44_100.0)),
            ir_slot: Arc::new(IrSlot::default()),
            status: Arc::new(AtomicU8::new(STATUS_IDLE)),
            status_note: Arc::new(Mutex::new(
                "Ready — press COLLAPSE to generate an Atlas IR (mock by default).".into(),
            )),
            collapse_busy: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Default for CollapseParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(420, 360),
            mix: FloatParam::new("Mix", 0.45, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_unit("%")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            drive: FloatParam::new("Drive", 0.35, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(30.0)),
            tone: FloatParam::new("Tone", 0.55, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(30.0)),
            size: FloatParam::new("Size", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(40.0)),
            output: FloatParam::new(
                "Output",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-24.0),
                    max: util::db_to_gain(12.0),
                    factor: FloatRange::gain_skew_factor(-24.0, 12.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(30.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
        }
    }
}

impl Plugin for Collapse {
    const NAME: &'static str = "COLLAPSE";
    const VENDOR: &'static str = "Moth Hack";
    const URL: &'static str = "https://hack.mothquantum.com/";
    const EMAIL: &'static str = "hack@mothquantum.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = CollapseTask;

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        let ir_slot = self.ir_slot.clone();
        let status = self.status.clone();
        let status_note = self.status_note.clone();
        let collapse_busy = self.collapse_busy.clone();
        let params = self.params.clone();
        let sample_rate = self.sample_rate.clone();

        Box::new(move |task| match task {
            CollapseTask::Run { seed } => {
                if collapse_busy.swap(true, Ordering::SeqCst) {
                    return;
                }
                status.store(STATUS_RUNNING, Ordering::SeqCst);
                *status_note.lock() = format!("Collapsing via Atlas… seed={seed}");

                let sr = *sample_rate.lock();
                let req = CollapseRequest {
                    seed,
                    drive: params.drive.value(),
                    tone: params.tone.value(),
                    size: params.size.value(),
                    sample_rate: sr,
                };

                match run_collapse(&req) {
                    Ok(result) => {
                        ir_slot.publish(&result.ir);
                        status.store(STATUS_READY, Ordering::SeqCst);
                        *status_note.lock() = format!(
                            "{} · engine={} · mode={}",
                            result.note, result.engine, result.mode
                        );
                    }
                    Err(err) => {
                        status.store(STATUS_ERROR, Ordering::SeqCst);
                        *status_note.lock() = format!("Atlas error: {err}");
                    }
                }
                collapse_busy.store(false, Ordering::SeqCst);
            }
        })
    }

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let egui_state = params.editor_state.clone();
        let status = self.status.clone();
        let status_note = self.status_note.clone();
        let collapse_busy = self.collapse_busy.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            |_, _| {},
            move |egui_ctx, setter, _state| {
                ResizableWindow::new("collapse-window")
                    .min_size(Vec2::new(360.0, 300.0))
                    .show(egui_ctx, egui_state.as_ref(), |ui| {
                        egui::Frame::NONE
                            .fill(Color32::from_rgb(14, 16, 20))
                            .inner_margin(16.0)
                            .show(ui, |ui| {
                                ui.colored_label(
                                    Color32::from_rgb(230, 220, 200),
                                    RichText::new("COLLAPSE").size(28.0).strong(),
                                );
                                ui.label(
                                    RichText::new("Atlas IR effect · Moth Hack 07")
                                        .size(13.0)
                                        .color(Color32::from_rgb(160, 150, 140)),
                                );
                                ui.add_space(12.0);

                                let status_code = status.load(Ordering::Relaxed);
                                let (label, color) = match status_code {
                                    STATUS_RUNNING => ("RUNNING", Color32::from_rgb(230, 180, 60)),
                                    STATUS_READY => ("READY", Color32::from_rgb(90, 200, 120)),
                                    STATUS_ERROR => ("ERROR", Color32::from_rgb(220, 80, 80)),
                                    _ => ("IDLE", Color32::from_rgb(140, 140, 150)),
                                };
                                ui.horizontal(|ui| {
                                    ui.colored_label(color, RichText::new("●").size(16.0));
                                    ui.label(RichText::new(label).color(color).strong());
                                });
                                let note = status_note.lock().clone();
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(note)
                                            .size(12.0)
                                            .color(Color32::from_rgb(180, 175, 165)),
                                    )
                                    .wrap(),
                                );

                                ui.add_space(10.0);
                                let busy = collapse_busy.load(Ordering::Relaxed);
                                let btn = egui::Button::new(
                                    RichText::new(if busy { "COLLAPSING…" } else { "COLLAPSE" })
                                        .size(18.0)
                                        .strong(),
                                )
                                .min_size(Vec2::new(ui.available_width(), 44.0))
                                .fill(if busy {
                                    Color32::from_rgb(60, 50, 40)
                                } else {
                                    Color32::from_rgb(180, 70, 40)
                                });
                                if ui.add_enabled(!busy, btn).clicked() {
                                    async_executor
                                        .execute_background(CollapseTask::Run { seed: next_seed() });
                                }

                                ui.add_space(14.0);
                                ui.label(RichText::new("Mix").color(Color32::LIGHT_GRAY));
                                ui.add(widgets::ParamSlider::for_param(&params.mix, setter));
                                ui.label(RichText::new("Drive").color(Color32::LIGHT_GRAY));
                                ui.add(widgets::ParamSlider::for_param(&params.drive, setter));
                                ui.label(RichText::new("Tone").color(Color32::LIGHT_GRAY));
                                ui.add(widgets::ParamSlider::for_param(&params.tone, setter));
                                ui.label(RichText::new("Size").color(Color32::LIGHT_GRAY));
                                ui.add(widgets::ParamSlider::for_param(&params.size, setter));
                                ui.label(RichText::new("Output").color(Color32::LIGHT_GRAY));
                                ui.add(widgets::ParamSlider::for_param(&params.output, setter));

                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(
                                        "Tip: ATLAS_MODE=mock (default) or live + ATLAS_API_KEY",
                                    )
                                    .size(11.0)
                                    .color(Color32::from_rgb(110, 110, 120)),
                                );
                            });
                    });
            },
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        *self.sample_rate.lock() = buffer_config.sample_rate;
        self.dsp.set_sample_rate(buffer_config.sample_rate);
        let default = run_collapse(&CollapseRequest {
            seed: 7,
            drive: self.params.drive.value(),
            tone: self.params.tone.value(),
            size: self.params.size.value(),
            sample_rate: buffer_config.sample_rate,
        });
        if let Ok(result) = default {
            self.dsp.set_ir(&result.ir);
            self.status.store(STATUS_READY, Ordering::SeqCst);
            *self.status_note.lock() = format!("Default IR loaded · {}", result.note);
        }
        true
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.ir_slot.try_apply(&mut self.dsp);

        for channel_samples in buffer.iter_samples() {
            let mix = self.params.mix.smoothed.next();
            let drive = self.params.drive.smoothed.next();
            let tone = self.params.tone.smoothed.next();
            let out = self.params.output.smoothed.next();

            let mut channels = channel_samples.into_iter();
            if let Some(left) = channels.next() {
                if let Some(right) = channels.next() {
                    let (l, r) = self
                        .dsp
                        .process_stereo(*left, *right, mix, drive, tone, out);
                    *left = l;
                    *right = r;
                } else {
                    let (l, _) = self.dsp.process_stereo(*left, *left, mix, drive, tone, out);
                    *left = l;
                }
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Collapse {
    const CLAP_ID: &'static str = "com.mothhack.collapse";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Atlas-powered impulse Collapse effect for Moth Hack");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Reverb,
        ClapFeature::Distortion,
    ];
}

impl Vst3Plugin for Collapse {
    const VST3_CLASS_ID: [u8; 16] = *b"MothCollapseFX07";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Reverb,
        Vst3SubCategory::Distortion,
    ];
}

nih_export_clap!(Collapse);
nih_export_vst3!(Collapse);
