use goldsrc_dsp::{ClipMode, GoldSrcReverb, Preset, PRESETS, ROOM_NAMES};
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;
use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering},
    Arc,
};

mod editor;
mod presets;

// ─── Plugin struct ────────────────────────────────────────────────────────────

struct GoldsrcPlugin {
    params: Arc<GoldsrcPluginParams>,
    user_preset_state: Arc<SharedUserPresetState>,

    /// DSP engine â€” created in initialize() once the sample rate is known.
    reverb: Option<GoldSrcReverb>,

    /// Pre-allocated scratch buffers so process() is allocation-free.
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

pub(crate) struct SharedUserPresetState {
    has_snapshot: AtomicBool,
    matches_current: AtomicBool,
    room: AtomicI32,
    reverb_mix: AtomicU32,
    delay_mix: AtomicU32,
    clip_soft: AtomicI32,
    enable_amplp: AtomicBool,
    enable_ampmod: AtomicBool,
    reverb_size: AtomicU32,
    reverb_feedback: AtomicU32,
    enable_revlp: AtomicBool,
    delay_time: AtomicU32,
    delay_feedback: AtomicU32,
    enable_dellp: AtomicBool,
    haas_time: AtomicU32,
    seed: AtomicI32,
    input_gain_db: AtomicU32,
    output_gain_db: AtomicU32,
}

impl SharedUserPresetState {
    fn new() -> Self {
        Self {
            has_snapshot: AtomicBool::new(false),
            matches_current: AtomicBool::new(true),
            room: AtomicI32::new(0),
            reverb_mix: AtomicU32::new(0),
            delay_mix: AtomicU32::new(0),
            clip_soft: AtomicI32::new(0),
            enable_amplp: AtomicBool::new(false),
            enable_ampmod: AtomicBool::new(false),
            reverb_size: AtomicU32::new(0),
            reverb_feedback: AtomicU32::new(0),
            enable_revlp: AtomicBool::new(false),
            delay_time: AtomicU32::new(0),
            delay_feedback: AtomicU32::new(0),
            enable_dellp: AtomicBool::new(false),
            haas_time: AtomicU32::new(0),
            seed: AtomicI32::new(0),
            input_gain_db: AtomicU32::new(0),
            output_gain_db: AtomicU32::new(0),
        }
    }

    pub(crate) fn set_snapshot(&self, snapshot: &presets::PluginParamsSnapshot) {
        self.room.store(snapshot.room, Ordering::Relaxed);
        self.reverb_mix
            .store(snapshot.reverb_mix.to_bits(), Ordering::Relaxed);
        self.delay_mix
            .store(snapshot.delay_mix.to_bits(), Ordering::Relaxed);
        self.clip_soft.store(snapshot.clip_soft, Ordering::Relaxed);
        self.enable_amplp
            .store(snapshot.enable_amplp, Ordering::Relaxed);
        self.enable_ampmod
            .store(snapshot.enable_ampmod, Ordering::Relaxed);
        self.reverb_size
            .store(snapshot.reverb_size.to_bits(), Ordering::Relaxed);
        self.reverb_feedback
            .store(snapshot.reverb_feedback.to_bits(), Ordering::Relaxed);
        self.enable_revlp
            .store(snapshot.enable_revlp, Ordering::Relaxed);
        self.delay_time
            .store(snapshot.delay_time.to_bits(), Ordering::Relaxed);
        self.delay_feedback
            .store(snapshot.delay_feedback.to_bits(), Ordering::Relaxed);
        self.enable_dellp
            .store(snapshot.enable_dellp, Ordering::Relaxed);
        self.haas_time
            .store(snapshot.haas_time.to_bits(), Ordering::Relaxed);
        self.seed.store(snapshot.seed, Ordering::Relaxed);
        self.input_gain_db
            .store(snapshot.input_gain_db.to_bits(), Ordering::Relaxed);
        self.output_gain_db
            .store(snapshot.output_gain_db.to_bits(), Ordering::Relaxed);
        self.has_snapshot.store(true, Ordering::Relaxed);
        self.matches_current.store(true, Ordering::Relaxed);
    }

    pub(crate) fn matches_params(&self, params: &GoldsrcPluginParams) -> bool {
        if !self.has_snapshot.load(Ordering::Relaxed) {
            return true;
        }

        self.room.load(Ordering::Relaxed) == params.room.value()
            && self.reverb_mix.load(Ordering::Relaxed) == params.reverb_mix.value().to_bits()
            && self.delay_mix.load(Ordering::Relaxed) == params.delay_mix.value().to_bits()
            && self.clip_soft.load(Ordering::Relaxed) == params.clip_soft.value()
            && self.enable_amplp.load(Ordering::Relaxed) == params.enable_amplp.value()
            && self.enable_ampmod.load(Ordering::Relaxed) == params.enable_ampmod.value()
            && self.reverb_size.load(Ordering::Relaxed) == params.reverb_size.value().to_bits()
            && self.reverb_feedback.load(Ordering::Relaxed)
                == params.reverb_feedback.value().to_bits()
            && self.enable_revlp.load(Ordering::Relaxed) == params.enable_revlp.value()
            && self.delay_time.load(Ordering::Relaxed) == params.delay_time.value().to_bits()
            && self.delay_feedback.load(Ordering::Relaxed)
                == params.delay_feedback.value().to_bits()
            && self.enable_dellp.load(Ordering::Relaxed) == params.enable_dellp.value()
            && self.haas_time.load(Ordering::Relaxed) == params.haas_time.value().to_bits()
            && self.seed.load(Ordering::Relaxed) == params.seed.value()
            && self.input_gain_db.load(Ordering::Relaxed) == params.input_gain_db.value().to_bits()
            && self.output_gain_db.load(Ordering::Relaxed)
                == params.output_gain_db.value().to_bits()
    }

    pub(crate) fn update_match_flag(&self, params: &GoldsrcPluginParams) {
        self.matches_current
            .store(self.matches_params(params), Ordering::Relaxed);
    }

    pub(crate) fn matches_current(&self) -> bool {
        self.matches_current.load(Ordering::Relaxed)
    }
}

// ─── Parameters ──────────────────────────────────────────────────────────────

#[derive(Params)]
struct GoldsrcPluginParams {
    /// GoldSrc room type (0-28 factory presets, 29 = Custom).
    #[id = "room"]
    pub room: IntParam,

    /// Reverb wet/dry mix. 0.0 = fully dry, 1.0 = fully wet.
    /// GoldSrc engine default: 0.17 (17 %).
    #[id = "reverb_mix"]
    pub reverb_mix: FloatParam,

    /// When true, reverb mix is preserved across preset changes.
    #[id = "lock_reverb_mix"]
    pub lock_reverb_mix: BoolParam,

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

    /// Pre-DSP input gain in dB. Applied before any DSP processing.
    #[id = "input_gain_db"]
    pub input_gain_db: FloatParam,

    /// Post-DSP output gain in dB. Applied after dry/wet mix, before clipping.
    #[id = "output_gain_db"]
    pub output_gain_db: FloatParam,

    #[id = "user_preset_idx"]
    pub user_preset_idx: IntParam,

    #[id = "preset_source_idx"]
    pub preset_source_idx: IntParam,

    #[id = "preset_display_idx"]
    pub preset_display_idx: IntParam,

    #[id = "window_size_idx"]
    pub window_size_idx: IntParam,

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
pub(crate) const CUSTOM_ROOM_INDEX: usize = PRESETS.len();
pub(crate) const CUSTOM_ROOM: i32 = CUSTOM_ROOM_INDEX as i32;

impl Default for GoldsrcPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(GoldsrcPluginParams::default()),
            user_preset_state: Arc::new(SharedUserPresetState::new()),
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
                IntRange::Linear {
                    min: 0,
                    max: CUSTOM_ROOM,
                },
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

            lock_reverb_mix: BoolParam::new("Lock Reverb Mix", false).hide(),

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
                s.trim_end_matches("ms")
                    .trim()
                    .parse::<f32>()
                    .ok()
                    .map(|v| v / 1000.0)
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
            enable_dellp: BoolParam::new("Echo LPF", true).with_value_to_string(Arc::new(|v| {
                if v {
                    "On".to_string()
                } else {
                    "Off".to_string()
                }
            })),
            haas_time: FloatParam::new(
                "Haas Time",
                0.01,
                FloatRange::Linear { min: 0.0, max: 0.1 },
            )
            .with_unit(" ms")
            .with_value_to_string(Arc::new(|v| format!("{:.0}", v * 1000.0)))
            .with_string_to_value(Arc::new(|s| {
                s.trim_end_matches("ms")
                    .trim()
                    .parse::<f32>()
                    .ok()
                    .map(|v| v / 1000.0)
            })),

            seed: IntParam::new("RNG Seed", 42, IntRange::Linear { min: 0, max: 100 }),

            input_gain_db: FloatParam::new(
                "Input Gain",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(2))
            .with_smoother(SmoothingStyle::Logarithmic(5.0)),

            output_gain_db: FloatParam::new(
                "Output Gain",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(2))
            .with_smoother(SmoothingStyle::Logarithmic(5.0)),

            user_preset_idx: IntParam::new(
                "User Preset Index",
                0,
                IntRange::Linear { min: 0, max: 8192 },
            )
            .hide(),

            preset_source_idx: IntParam::new(
                "Preset Source Index",
                1,
                IntRange::Linear { min: 0, max: 1 },
            )
            .hide(),

            preset_display_idx: IntParam::new(
                "Preset Display Index",
                -1,
                IntRange::Linear {
                    min: -1,
                    max: 16384,
                },
            )
            .hide(),

            window_size_idx: IntParam::new(
                "Window Size Index",
                0,
                IntRange::Linear { min: 0, max: 2 },
            )
            .hide(),
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
        editor::create(
            self.params.clone(),
            self.params.editor_state.clone(),
            self.user_preset_state.clone(),
        )
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
        self.user_preset_state.update_match_flag(&self.params);

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

            if room == CUSTOM_ROOM {
                self.current_preset = build_preset_from_params(&self.params);
                self.knob_snapshot = self.current_preset;
            } else {
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
            }
        } else {
            let knobs = build_preset_from_params(&self.params);

            for i in 0..9 {
                if knobs[i] != self.knob_snapshot[i] {
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

        // ── Input gain ─────────────────────────────────────────────────────
        for i in 0..n {
            let g = util::db_to_gain(self.params.input_gain_db.smoothed.next());
            self.scratch_l[i] *= g;
            self.scratch_r[i] *= g;
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

        // ── Output gain ────────────────────────────────────────────────────
        for i in 0..n {
            let g = util::db_to_gain(self.params.output_gain_db.smoothed.next());
            self.out_l[i] *= g;
            self.out_r[i] *= g;
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
