use goldsrc_dsp::{ClipMode, GoldSrcReverb, Preset, PRESETS, ROOM_NAMES};
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;
use std::sync::Arc;

mod editor;
mod presets;

// ─── Plugin struct ────────────────────────────────────────────────────────────

struct GoldsrcPlugin {
    params: Arc<GoldsrcPluginParams>,

    /// DSP engine — created in `initialize()` once the sample rate is known.
    reverb: Option<GoldSrcReverb>,

    /// Pre-allocated scratch buffers so `process()` is allocation-free.
    scratch_l: Vec<f32>,
    scratch_r: Vec<f32>,
    out_l: Vec<f32>,
    out_r: Vec<f32>,

    /// The preset array that the DSP engine actually uses.
    /// Updated from PRESETS when room changes, patched by individual knobs.
    current_preset: Preset,

    /// Snapshot of the individual knobs taken at the moment of a room change.
    /// Used to detect which knobs the user has since moved.
    knob_snapshot: Preset,

    /// Tracks the last room value to detect room changes.
    last_room: i32,

    /// Tracks the last seed value to detect seed changes.
    last_seed: i64,
}

// ─── Parameters ──────────────────────────────────────────────────────────────

#[derive(Params)]
struct GoldsrcPluginParams {
    /// GoldSrc room type (0 = off, 1–28 = various reverb characters).
    #[id = "room"]
    pub room: IntParam,

    /// Reverb wet/dry mix. 0.0 = fully dry, 1.0 = fully wet.
    /// GoldSrc engine default: 0.17 (17 %).
    #[id = "reverb_mix"]
    pub reverb_mix: FloatParam,

    /// Mono echo wet level. 0.0 = off, 1.0 = full.
    /// GoldSrc engine default: 0.25 (25 %).
    #[id = "delay_mix"]
    pub delay_mix: FloatParam,

    /// Output limiter mode: 0 = Off, 1 = Soft (tanh), 2 = Hard (clamp).
    #[id = "clip_soft"]
    pub clip_soft: IntParam,

    // Individual preset knobs — map 1:1 to Preset's [f32; 9]
    #[id = "enable_amplp"]
    pub enable_amplp: BoolParam,

    #[id = "enable_ampmod"]
    pub enable_ampmod: BoolParam,

    #[id = "reverb_size"]
    pub reverb_size: FloatParam,

    #[id = "reverb_feedback"]
    pub reverb_feedback: FloatParam,

    #[id = "enable_revlp"]
    pub enable_revlp: BoolParam,

    #[id = "delay_time"]
    pub delay_time: FloatParam,

    #[id = "delay_feedback"]
    pub delay_feedback: FloatParam,

    #[id = "enable_dellp"]
    pub enable_dellp: BoolParam,

    #[id = "haas_time"]
    pub haas_time: FloatParam,

    #[id = "seed"]
    pub seed: IntParam,

    /// Persisted editor window state (scale factor, etc.).
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,
}

// ─── Preset helpers ───────────────────────────────────────────────────────────

/// Reads all nine individual knobs into a `Preset` array.
/// Field order: `[lp, mod, size, refl, rvblp, delay, feedback, dlylp, left]`
fn build_preset_from_params(p: &GoldsrcPluginParams) -> Preset {
    [
        if p.enable_amplp.value() { 1.0 } else { 0.0 }, // [0] P_LP
        if p.enable_ampmod.value() { 1.0 } else { 0.0 }, // [1] P_MOD
        p.reverb_size.value(),                          // [2] P_SIZE
        p.reverb_feedback.value(),                      // [3] P_REFL
        if p.enable_revlp.value() { 1.0 } else { 0.0 }, // [4] P_RVBLP
        p.delay_time.value(),                           // [5] P_DELAY
        p.delay_feedback.value(),                       // [6] P_FEEDBACK
        if p.enable_dellp.value() { 0.0 } else { 2.0 }, // [7] P_DLYLP
        p.haas_time.value(),                            // [8] P_LEFT
    ]
}

// ─── Defaults ─────────────────────────────────────────────────────────────────

const DEFAULT_ROOM: usize = 5;

impl Default for GoldsrcPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(GoldsrcPluginParams::default()),
            reverb: None,
            scratch_l: Vec::new(),
            scratch_r: Vec::new(),
            out_l: Vec::new(),
            out_r: Vec::new(),
            current_preset: PRESETS[DEFAULT_ROOM],
            knob_snapshot: [0.0; 9], // will be set on first process() block
            last_room: -1,           // forces PRESETS load on first block
            last_seed: -1,           // forces seed apply on first block
        }
    }
}

impl Default for GoldsrcPluginParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),

            room: IntParam::new(
                "Room Type",
                DEFAULT_ROOM as i32,
                IntRange::Linear { min: 0, max: 28 },
            )
            .with_value_to_string(Arc::new(|v: i32| {
                let idx = v.max(0) as usize;
                format!("{} – {}", v, ROOM_NAMES.get(idx).copied().unwrap_or("?"))
            })),

            reverb_mix: FloatParam::new(
                "Reverb Mix",
                0.17,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            delay_mix: FloatParam::new(
                "Echo Level",
                0.25,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            clip_soft: IntParam::new("Clip Mode", 2, IntRange::Linear { min: 0, max: 2 })
                .with_value_to_string(Arc::new(|v| match v {
                    0 => "Off".to_string(),
                    1 => "Soft".to_string(),
                    _ => "Hard".to_string(),
                })),

            enable_amplp: BoolParam::new("Amp Mod LPF", false),
            enable_ampmod: BoolParam::new("Amp Mod", false),
            reverb_size: FloatParam::new(
                "Reverb Size",
                0.05,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            reverb_feedback: FloatParam::new(
                "Reverb Feedback",
                0.85,
                FloatRange::Linear {
                    min: 0.0,
                    max: 0.999,
                },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            enable_revlp: BoolParam::new("Reverb LPF", true),
            delay_time: FloatParam::new(
                "Echo Time",
                0.008,
                FloatRange::Linear { min: 0.0, max: 0.4 },
            )
            .with_unit(" ms")
            .with_value_to_string(Arc::new(|v| format!("{:.0}", v * 1000.0)))
            .with_string_to_value(Arc::new(|s| {
                s.trim_end_matches("ms").trim().parse::<f32>().ok().map(|v| v / 1000.0)
            })),
            delay_feedback: FloatParam::new(
                "Echo Feedback",
                0.96,
                FloatRange::Linear {
                    min: 0.0,
                    max: 0.999,
                },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            enable_dellp: BoolParam::new("Echo LPF", true)
                .with_value_to_string(Arc::new(|v| {
                    if v { "On".to_string() } else { "Off".to_string() }
                })),
            haas_time: FloatParam::new(
                "Haas Time",
                0.01,
                FloatRange::Linear { min: 0.0, max: 0.1 },
            )
            .with_unit(" ms")
            .with_value_to_string(Arc::new(|v| format!("{:.0}", v * 1000.0)))
            .with_string_to_value(Arc::new(|s| {
                s.trim_end_matches("ms").trim().parse::<f32>().ok().map(|v| v / 1000.0)
            })),

            seed: IntParam::new("RNG Seed", 42, IntRange::Linear { min: 0, max: 100 }),
        }
    }
}

// ─── Plugin implementation ────────────────────────────────────────────────────

impl Plugin for GoldsrcPlugin {
    const NAME: &'static str = "GoldSrc Reverb";
    const VENDOR: &'static str = "shipi";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "ariasmartin1@gmail.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.params.editor_state.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        let sr = buffer_config.sample_rate as u32;
        let max_buf = buffer_config.max_buffer_size as usize;

        self.reverb = Some(GoldSrcReverb::new(sr));
        self.last_room = -1; // force preset load on first process() block

        self.scratch_l.resize(max_buf, 0.0);
        self.scratch_r.resize(max_buf, 0.0);
        self.out_l.resize(max_buf, 0.0);
        self.out_r.resize(max_buf, 0.0);

        true
    }

    fn reset(&mut self) {
        if let Some(reverb) = &mut self.reverb {
            reverb.reset_buffers();
        }
    }

    /// Real-time audio callback — must not allocate.
    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let n = buffer.samples();

        let room = self.params.room.value();
        let reverb_mix = self.params.reverb_mix.value();
        let delay_mix = self.params.delay_mix.value();
        let clip_mode = match self.params.clip_soft.value() {
            0 => ClipMode::Off,
            1 => ClipMode::Soft,
            _ => ClipMode::Hard,
        };

        // ── Preset management ──────────────────────────────────────────────
        //
        // Room changed → load the full row from PRESETS into current_preset
        //                and snapshot the knob positions so we can detect
        //                future user tweaks.
        //
        // Room steady  → compare each knob to the snapshot; if a knob moved
        //                the user tweaked it, so patch that slot in
        //                current_preset (and update the snapshot).

        if room != self.last_room {
            self.last_room = room;
            self.current_preset = PRESETS[(room as usize).min(PRESETS.len() - 1)];
            let knobs = build_preset_from_params(&self.params);
            // Apply saved knob overrides immediately (handles .vstpreset loads
            // where all params are restored before the first process() call).
            for i in 0..9 {
                if knobs[i] != self.current_preset[i] {
                    self.current_preset[i] = knobs[i];
                }
            }
            self.knob_snapshot = knobs;
        } else {
            let knobs = build_preset_from_params(&self.params);
            for i in 0..9 {
                if knobs[i] != self.knob_snapshot[i] {
                    // This knob was moved by the user — apply it.
                    self.current_preset[i] = knobs[i];
                    self.knob_snapshot[i] = knobs[i];
                }
            }
        }

        // ── Copy host input into scratch buffers ───────────────────────────
        {
            let channels = buffer.as_slice();
            if channels.len() < 2 {
                return ProcessStatus::Normal;
            }
            self.scratch_l[..n].copy_from_slice(&channels[0][..n]);
            self.scratch_r[..n].copy_from_slice(&channels[1][..n]);
        }

        // ── Run DSP ────────────────────────────────────────────────────────
        {
            let reverb = match self.reverb.as_mut() {
                Some(r) => r,
                None => return ProcessStatus::Normal,
            };

            reverb.set_room_type(self.current_preset);
            reverb.set_reverb_mix(reverb_mix);
            reverb.set_delay_mix(delay_mix);
            reverb.set_clip_mode(clip_mode);

            let seed = self.params.seed.value() as i64;
            if seed != self.last_seed {
                self.last_seed = seed;
                reverb.set_rng_seed(seed as u64);
            }

            reverb.process(
                &self.scratch_l[..n],
                &self.scratch_r[..n],
                &mut self.out_l[..n],
                &mut self.out_r[..n],
            );
        }

        // ── Write back to host buffer ──────────────────────────────────────
        {
            let channels = buffer.as_slice();
            channels[0][..n].copy_from_slice(&self.out_l[..n]);
            channels[1][..n].copy_from_slice(&self.out_r[..n]);
        }

        ProcessStatus::Normal
    }
}

// ─── Format registration ──────────────────────────────────────────────────────

impl ClapPlugin for GoldsrcPlugin {
    const CLAP_ID: &'static str = "com.shipisnature.goldsrc-plugin";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("GoldSrc-based reverb plugin");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;

    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Reverb,
    ];
}

impl Vst3Plugin for GoldsrcPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"shipigoldsrcrevb";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Reverb];
}

nih_export_clap!(GoldsrcPlugin);
nih_export_vst3!(GoldsrcPlugin);



