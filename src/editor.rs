use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use rfd::AsyncFileDialog;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, AtomicUsize, Ordering},
        Arc,
        Mutex,
    },
};

use crate::{presets, GoldsrcPluginParams, SharedUserPresetState, CUSTOM_ROOM};
use goldsrc_dsp::{PRESETS, ROOM_NAMES};

const VERSION_LABEL: &str = concat!("v", env!("CARGO_PKG_VERSION"));
const WINDOW_SIZE_PRESETS: [(&str, u32, u32); 3] = [
    ("Compact (500x650)", 500, 650),
    ("Default (620x760)", 620, 760),
    ("Large (760x900)", 760, 900),
];

static WINDOW_WIDTH: AtomicU32 = AtomicU32::new(500);
static WINDOW_HEIGHT: AtomicU32 = AtomicU32::new(650);

fn window_size_labels() -> Vec<String> {
    WINDOW_SIZE_PRESETS
        .iter()
        .map(|(label, _, _)| (*label).to_string())
        .collect()
}

fn apply_window_size_preset(index: usize) {
    let clamped = index.min(WINDOW_SIZE_PRESETS.len().saturating_sub(1));
    let (_, width, height) = WINDOW_SIZE_PRESETS[clamped];
    WINDOW_WIDTH.store(width, Ordering::Relaxed);
    WINDOW_HEIGHT.store(height, Ordering::Relaxed);
}

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| {
        (
            WINDOW_WIDTH.load(Ordering::Relaxed),
            WINDOW_HEIGHT.load(Ordering::Relaxed),
        )
    })
}

pub(crate) fn create(
    params: Arc<GoldsrcPluginParams>,
    editor_state: Arc<ViziaState>,
    user_preset_state: Arc<SharedUserPresetState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        assets::register_noto_sans_light(cx);
        assets::register_noto_sans_bold(cx);

        // Load our custom dark theme on top of the defaults
        if let Err(e) = cx.add_stylesheet(include_style!("src/editor.css")) {
            nih_plug::debug::nih_error!("Failed to load editor stylesheet: {e:?}");
        }

        // Data model for parameter binding
        let (user_preset_options, user_preset_paths) = available_user_presets();
        let user_preset_labels = Arc::new(Mutex::new(user_preset_options.clone()));
        let user_preset_paths = Arc::new(Mutex::new(user_preset_paths));
        let selected_user_preset_idx =
            Arc::new(AtomicUsize::new(params.user_preset_idx.value().max(0) as usize));
        sync_user_preset_state_from_selection(
            &user_preset_state,
            &user_preset_paths,
            selected_user_preset_idx.load(Ordering::Relaxed),
        );
        let window_size_options = window_size_labels();
        let window_size_max_index = window_size_options.len().saturating_sub(1);
        apply_window_size_preset(params.window_size_idx.value().clamp(0, window_size_max_index as i32) as usize);

        Data {
            params: params.clone(),
            user_preset_options,
            window_size_options,
        }
        .build(cx);

        // ── Main column layout ─────────────────────────────────────────
        VStack::new(cx, |cx| {
            // ── Header ─────────────────────────────────────────────────
            VStack::new(cx, |cx| {
                Label::new(cx, "GoldSrc Reverb").class("title");
                Label::new(cx, "Half-Life engine reverb emulation").class("subtitle");
                Label::new(cx, VERSION_LABEL).class("version");
            })
            .class("header");

            // ── Room Selection ─────────────────────────────────────────
            let user_preset_labels_for_save = user_preset_labels.clone();
            let user_preset_paths_for_save = user_preset_paths.clone();
            room_section(cx, "ROOM", |cx| {
                param_row(cx, "Preset", |cx| {
                    PickList::new(
                        cx,
                        {
                            let user_preset_labels = user_preset_labels_for_save.clone();
                            let user_preset_paths = user_preset_paths_for_save.clone();
                            let user_preset_state = user_preset_state.clone();
                            Data::params.map(move |p| {
                                let user_labels = user_preset_labels
                                    .lock()
                                    .map(|shared| shared.clone())
                                    .unwrap_or_default();
                                let user_count = user_preset_paths.lock().map(|paths| paths.len()).unwrap_or(0);

                                let mut labels = if user_count == 0 {
                                    vec!["User / No presets found".to_string()]
                                } else {
                                    user_labels
                                        .into_iter()
                                        .take(user_count)
                                        .map(|label| format!("User / {label}"))
                                        .collect::<Vec<_>>()
                                };

                                labels.extend(
                                    ROOM_NAMES
                                        .iter()
                                        .enumerate()
                                        .map(|(i, name)| format!("Factory / {i} - {name}")),
                                );

                                if p.preset_source_idx.value().clamp(0, 1) == 1 {
                                    let user_display_count = if user_count == 0 { 1 } else { user_count };
                                    let selected_idx = user_display_count
                                        + p.room.value().clamp(0, (PRESETS.len() - 1) as i32) as usize;

                                    if effective_room_index(p.as_ref()) == CUSTOM_ROOM as usize {
                                        if let Some(label) = labels.get_mut(selected_idx) {
                                            if !label.ends_with(" *") {
                                                label.push_str(" *");
                                            }
                                        }
                                    }
                                } else {
                                    let selected_idx = if user_count == 0 {
                                        0
                                    } else {
                                        (p.user_preset_idx.value().max(0) as usize).min(user_count - 1)
                                    };

                                    if !user_preset_state.matches_current() {
                                        if let Some(label) = labels.get_mut(selected_idx) {
                                            if !label.ends_with(" *") {
                                                label.push_str(" *");
                                            }
                                        }
                                    }
                                }

                                labels
                            })
                        },
                        {
                            let user_preset_paths = user_preset_paths_for_save.clone();
                            Data::params.map(move |p| {
                                let user_count = user_preset_paths.lock().map(|paths| paths.len()).unwrap_or(0);
                                let user_display_count = if user_count == 0 { 1 } else { user_count };
                                let combined_count = user_display_count + PRESETS.len();
                                let stored_idx = p.preset_display_idx.value();

                                if stored_idx >= 0 && (stored_idx as usize) < combined_count {
                                    stored_idx as usize
                                } else if p.preset_source_idx.value().clamp(0, 1) == 1 {
                                    user_display_count
                                        + p.room.value().clamp(0, (PRESETS.len() - 1) as i32) as usize
                                } else if user_count == 0 {
                                    0
                                } else {
                                    (p.user_preset_idx.value().max(0) as usize).min(user_count - 1)
                                }
                            })
                        },
                        true,
                    )
                    .on_select({
                        let params = params.clone();
                        let user_preset_paths = user_preset_paths.clone();
                        let selected_user_preset_idx = selected_user_preset_idx.clone();
                        let user_preset_state = user_preset_state.clone();
                        move |cx, preset_idx| {
                            let user_count = user_preset_paths.lock().map(|paths| paths.len()).unwrap_or(0);
                            let user_display_count = if user_count == 0 { 1 } else { user_count };

                            if preset_idx >= user_display_count {
                                let factory_idx = preset_idx - user_display_count;
                                cx.emit(ParamEvent::BeginSetParameter(&params.preset_source_idx).upcast());
                                cx.emit(ParamEvent::SetParameter(&params.preset_source_idx, 1).upcast());
                                cx.emit(ParamEvent::EndSetParameter(&params.preset_source_idx).upcast());
                                cx.emit(ParamEvent::BeginSetParameter(&params.preset_display_idx).upcast());
                                cx.emit(ParamEvent::SetParameter(&params.preset_display_idx, preset_idx as i32).upcast());
                                cx.emit(ParamEvent::EndSetParameter(&params.preset_display_idx).upcast());
                                apply_factory_preset(cx, &params, factory_idx);
                                return;
                            }

                            if user_count == 0 {
                                return;
                            }

                            selected_user_preset_idx.store(preset_idx, Ordering::Relaxed);
                            cx.emit(ParamEvent::BeginSetParameter(&params.preset_source_idx).upcast());
                            cx.emit(ParamEvent::SetParameter(&params.preset_source_idx, 0).upcast());
                            cx.emit(ParamEvent::EndSetParameter(&params.preset_source_idx).upcast());
                            cx.emit(ParamEvent::BeginSetParameter(&params.preset_display_idx).upcast());
                            cx.emit(ParamEvent::SetParameter(&params.preset_display_idx, preset_idx as i32).upcast());
                            cx.emit(ParamEvent::EndSetParameter(&params.preset_display_idx).upcast());
                            cx.emit(ParamEvent::BeginSetParameter(&params.user_preset_idx).upcast());
                            cx.emit(ParamEvent::SetParameter(&params.user_preset_idx, preset_idx as i32).upcast());
                            cx.emit(ParamEvent::EndSetParameter(&params.user_preset_idx).upcast());
                            let path = user_preset_paths
                                .lock()
                                .ok()
                                .and_then(|paths| paths.get(preset_idx).cloned());
                            if let Some(path) = path {
                                match presets::load_snapshot_from_path(&path) {
                                    Ok(snapshot) => {
                                        let comparison_snapshot = snapshot_for_loaded_params(&snapshot);
                                        user_preset_state.set_snapshot(&comparison_snapshot);
                                        apply_snapshot_to_params(cx, &params, &snapshot)
                                    }
                                    Err(err) => nih_plug::debug::nih_error!(
                                        "Failed to load user preset from {}: {err}",
                                        path.display()
                                    ),
                                }
                            }
                        }
                    })
                    .class("widget");
                    {
                        let params = params.clone();
                        let selected_user_preset_idx = selected_user_preset_idx.clone();
                        let user_preset_paths = user_preset_paths.clone();
                        let user_preset_state = user_preset_state.clone();
                        Button::new(
                            cx,
                            move |cx| {
                                let default_dir = match presets::preset_root_dir() {
                                    Ok(path) => path,
                                    Err(err) => {
                                        nih_plug::debug::nih_error!(
                                            "Failed to resolve preset folder for save dialog: {err}"
                                        );
                                        return;
                                    }
                                };
                                let maybe_file = pollster::block_on(async {
                                    AsyncFileDialog::new()
                                        .set_title("Save GoldSrc Preset")
                                        .set_directory(default_dir)
                                        .set_file_name("preset.json")
                                        .add_filter("GoldSrc Preset", &["json"])
                                        .save_file()
                                        .await
                                });
                                if let Some(file) = maybe_file {
                                    let mut path = file.path().to_path_buf();
                                    if path.extension().is_none() {
                                        path.set_extension("json");
                                    }
                                    if let Err(err) = presets::save_params_snapshot_to_path(&params, &path) {
                                        nih_plug::debug::nih_error!(
                                            "Failed to save JSON preset to {}: {err}",
                                            path.display()
                                        );
                                        return;
                                    }
                                    let saved_name = path
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .map(|s| s.to_string());
                                    let (labels, paths) = available_user_presets();

                                    if let Ok(mut shared_labels) = user_preset_labels.lock() {
                                        *shared_labels = labels.clone();
                                    }
                                    if let Ok(mut shared_paths) = user_preset_paths.lock() {
                                        *shared_paths = paths;
                                    }
                                    if let Some(saved_name) = saved_name {
                                        if let Some(saved_idx) = labels.iter().position(|name| name == &saved_name) {
                                            selected_user_preset_idx.store(saved_idx, Ordering::Relaxed);
                                            cx.emit(ParamEvent::BeginSetParameter(&params.preset_source_idx).upcast());
                                            cx.emit(ParamEvent::SetParameter(&params.preset_source_idx, 0).upcast());
                                            cx.emit(ParamEvent::EndSetParameter(&params.preset_source_idx).upcast());
                                            cx.emit(ParamEvent::BeginSetParameter(&params.preset_display_idx).upcast());
                                            cx.emit(ParamEvent::SetParameter(&params.preset_display_idx, saved_idx as i32).upcast());
                                            cx.emit(ParamEvent::EndSetParameter(&params.preset_display_idx).upcast());
                                            cx.emit(ParamEvent::BeginSetParameter(&params.user_preset_idx).upcast());
                                            cx.emit(ParamEvent::SetParameter(&params.user_preset_idx, saved_idx as i32).upcast());
                                            cx.emit(ParamEvent::EndSetParameter(&params.user_preset_idx).upcast());
                                            if let Ok(saved_snapshot) = presets::load_snapshot_from_path(&path) {
                                                let comparison_snapshot = snapshot_for_loaded_params(&saved_snapshot);
                                                user_preset_state.set_snapshot(&comparison_snapshot);
                                            }
                                        }
                                    }
                                    cx.emit(DataEvent::SetUserPresetOptions(labels));
                                }
                            },
                            |cx| Label::new(cx, "Save"),
                        )
                        .class("preset-button");
                    }
                });

                param_row(cx, "Window", |cx| {
                    PickList::new(
                        cx,
                        Data::window_size_options,
                        Data::params.map(move |p| {
                            (p.window_size_idx.value().clamp(0, window_size_max_index as i32) as usize)
                                .min(window_size_max_index)
                        }),
                        true,
                    )
.on_select({
                        let params = params.clone();
                        move |cx, size_idx| {
                            let size_idx = size_idx.min(window_size_max_index) as i32;
                            cx.emit(ParamEvent::BeginSetParameter(&params.window_size_idx).upcast());
                            cx.emit(ParamEvent::SetParameter(&params.window_size_idx, size_idx).upcast());
                            cx.emit(ParamEvent::EndSetParameter(&params.window_size_idx).upcast());
                            apply_window_size_preset(size_idx as usize);
                            cx.emit(GuiContextEvent::Resize);
                        }
                    })
                    .class("widget");
                });
            });
            section(cx, "MIX", |cx| {
                param_row(cx, "Reverb Mix", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.reverb_mix).class("widget");
                    ParamButton::new(cx, Data::params, |p| &p.lock_reverb_mix)
                        .with_label("Lock")
                        .class("lock-button");
                });
                param_row(cx, "Echo Level", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.delay_mix).class("widget");
                });
            });

            // ── Reverb ─────────────────────────────────────────────────
            section(cx, "REVERB", |cx| {
                param_row(cx, "Size", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.reverb_size).class("widget");
                });
                param_row(cx, "Feedback", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.reverb_feedback).class("widget");
                });
                param_row(cx, "Low-Pass", |cx| {
                    ParamButton::new(cx, Data::params, |p| &p.enable_revlp)
                        .with_label("Reverb LPF");
                });
            });

            // ── Echo / Delay ───────────────────────────────────────────
            section(cx, "ECHO", |cx| {
                param_row(cx, "Time", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.delay_time)
                        .set_style(ParamSliderStyle::FromLeft)
                        .class("widget");
                });
                param_row(cx, "Feedback", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.delay_feedback).class("widget");
                });
                param_row(cx, "Low-Pass", |cx| {
                    ParamButton::new(cx, Data::params, |p| &p.enable_dellp).with_label("Echo LPF");
                });
            });

            // ── Modulation + I/O Gain ──────────────────────────────────
            VStack::new(cx, |cx| {
                HStack::new(cx, |cx| {
                    Label::new(cx, "MODULATION").class("section-label")
                        .width(Percentage(50.0));
                    Label::new(cx, "I / O").class("section-label")
                        .width(Percentage(50.0));
                })
                .height(Auto);
                HStack::new(cx, |cx| {
                    // ── Left: modulation controls ──────────────────────
                    VStack::new(cx, |cx| {
                        param_row(cx, "Amp Mod", |cx| {
                            ParamButton::new(cx, Data::params, |p| &p.enable_ampmod)
                                .with_label("Amp Mod");
                        });
                        param_row(cx, "Amp LPF", |cx| {
                            ParamButton::new(cx, Data::params, |p| &p.enable_amplp)
                                .with_label("Amp Mod LPF");
                        });
                    })
                    .width(Percentage(50.0));

                    // ── Right: I/O gain sliders ────────────────────────
                    VStack::new(cx, |cx| {
                        param_row(cx, "IN", |cx| {
                            ParamSlider::new(cx, Data::params, |p| &p.input_gain_db)
                                .set_style(ParamSliderStyle::Centered)
                                .class("widget");
                        });
                        param_row(cx, "OUT", |cx| {
                            ParamSlider::new(cx, Data::params, |p| &p.output_gain_db)
                                .set_style(ParamSliderStyle::Centered)
                                .class("widget");
                        });
                    })
                    .width(Percentage(50.0))
                    .class("io-gain");
                });
            })
            .class("section");

            // ── Stereo / Output ────────────────────────────────────────
            section(cx, "OUTPUT", |cx| {
                param_row(cx, "Haas Time", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.haas_time)
                        .set_style(ParamSliderStyle::FromLeft)
                        .class("widget");
                });
                param_row(cx, "Clip Mode", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.clip_soft)
                        .set_style(ParamSliderStyle::CurrentStepLabeled { even: true })
                        .class("widget");
                });
                param_row(cx, "RNG Seed", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.seed)
                        .set_style(ParamSliderStyle::FromLeft)
                        .class("widget");
                });
            });

            Element::new(cx).height(Pixels(5.0));
        })
        .class("main");
    })
}

fn params_knobs(params: &GoldsrcPluginParams) -> [f32; 9] {
    [
        if params.enable_amplp.value() {
            1.0
        } else {
            0.0
        },
        if params.enable_ampmod.value() {
            1.0
        } else {
            0.0
        },
        params.reverb_size.value(),
        params.reverb_feedback.value(),
        if params.enable_revlp.value() {
            1.0
        } else {
            0.0
        },
        params.delay_time.value(),
        params.delay_feedback.value(),
        if params.enable_dellp.value() {
            0.0
        } else {
            2.0
        },
        params.haas_time.value(),
    ]
}

fn effective_room_index(params: &GoldsrcPluginParams) -> usize {
    let room = params.room.value().clamp(0, CUSTOM_ROOM) as usize;
    if room >= PRESETS.len() {
        return CUSTOM_ROOM as usize;
    }

    if params_knobs(params) == PRESETS[room] {
        room
    } else {
        CUSTOM_ROOM as usize
    }
}
fn apply_factory_preset(cx: &mut EventContext, params: &Arc<GoldsrcPluginParams>, room_idx: usize) {
    let room = room_idx.min(PRESETS.len().saturating_sub(1)) as i32;
    let preset = PRESETS[room as usize];

    cx.emit(ParamEvent::BeginSetParameter(&params.room).upcast());
    cx.emit(ParamEvent::SetParameter(&params.room, room).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.room).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.enable_amplp).upcast());
    cx.emit(ParamEvent::SetParameter(&params.enable_amplp, preset[0] >= 0.5).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.enable_amplp).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.enable_ampmod).upcast());
    cx.emit(ParamEvent::SetParameter(&params.enable_ampmod, preset[1] >= 0.5).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.enable_ampmod).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.reverb_size).upcast());
    cx.emit(ParamEvent::SetParameter(&params.reverb_size, preset[2]).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.reverb_size).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.reverb_feedback).upcast());
    cx.emit(ParamEvent::SetParameter(&params.reverb_feedback, preset[3]).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.reverb_feedback).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.enable_revlp).upcast());
    cx.emit(ParamEvent::SetParameter(&params.enable_revlp, preset[4] >= 0.5).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.enable_revlp).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.delay_time).upcast());
    cx.emit(ParamEvent::SetParameter(&params.delay_time, preset[5]).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.delay_time).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.delay_feedback).upcast());
    cx.emit(ParamEvent::SetParameter(&params.delay_feedback, preset[6]).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.delay_feedback).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.enable_dellp).upcast());
    cx.emit(ParamEvent::SetParameter(&params.enable_dellp, preset[7] == 0.0).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.enable_dellp).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.haas_time).upcast());
    cx.emit(ParamEvent::SetParameter(&params.haas_time, preset[8]).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.haas_time).upcast());
}
fn snapshot_knobs(snapshot: &presets::PluginParamsSnapshot) -> [f32; 9] {
    [
        if snapshot.enable_amplp { 1.0 } else { 0.0 },
        if snapshot.enable_ampmod { 1.0 } else { 0.0 },
        snapshot.reverb_size,
        snapshot.reverb_feedback,
        if snapshot.enable_revlp { 1.0 } else { 0.0 },
        snapshot.delay_time,
        snapshot.delay_feedback,
        if snapshot.enable_dellp { 0.0 } else { 2.0 },
        snapshot.haas_time,
    ]
}

fn snapshot_target_room(snapshot: &presets::PluginParamsSnapshot) -> i32 {
    let room = snapshot.room.clamp(0, CUSTOM_ROOM);
    if room == CUSTOM_ROOM {
        return CUSTOM_ROOM;
    }

    let room_index = room as usize;
    if room_index >= PRESETS.len() {
        return CUSTOM_ROOM;
    }

    if snapshot_knobs(snapshot) == PRESETS[room_index] {
        room
    } else {
        CUSTOM_ROOM
    }
}

fn snapshot_for_loaded_params(
    snapshot: &presets::PluginParamsSnapshot,
) -> presets::PluginParamsSnapshot {
    let mut normalized = snapshot.clone();
    normalized.room = snapshot_target_room(snapshot);
    normalized
}
fn apply_snapshot_to_params(
    cx: &mut EventContext,
    params: &Arc<GoldsrcPluginParams>,
    snapshot: &presets::PluginParamsSnapshot,
) {
    if !params.lock_reverb_mix.value() {
        cx.emit(ParamEvent::BeginSetParameter(&params.reverb_mix).upcast());
        cx.emit(ParamEvent::SetParameter(&params.reverb_mix, snapshot.reverb_mix).upcast());
        cx.emit(ParamEvent::EndSetParameter(&params.reverb_mix).upcast());
    }

    cx.emit(ParamEvent::BeginSetParameter(&params.delay_mix).upcast());
    cx.emit(ParamEvent::SetParameter(&params.delay_mix, snapshot.delay_mix).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.delay_mix).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.clip_soft).upcast());
    cx.emit(ParamEvent::SetParameter(&params.clip_soft, snapshot.clip_soft).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.clip_soft).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.enable_amplp).upcast());
    cx.emit(ParamEvent::SetParameter(&params.enable_amplp, snapshot.enable_amplp).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.enable_amplp).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.enable_ampmod).upcast());
    cx.emit(ParamEvent::SetParameter(&params.enable_ampmod, snapshot.enable_ampmod).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.enable_ampmod).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.reverb_size).upcast());
    cx.emit(ParamEvent::SetParameter(&params.reverb_size, snapshot.reverb_size).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.reverb_size).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.reverb_feedback).upcast());
    cx.emit(ParamEvent::SetParameter(&params.reverb_feedback, snapshot.reverb_feedback).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.reverb_feedback).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.enable_revlp).upcast());
    cx.emit(ParamEvent::SetParameter(&params.enable_revlp, snapshot.enable_revlp).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.enable_revlp).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.delay_time).upcast());
    cx.emit(ParamEvent::SetParameter(&params.delay_time, snapshot.delay_time).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.delay_time).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.delay_feedback).upcast());
    cx.emit(ParamEvent::SetParameter(&params.delay_feedback, snapshot.delay_feedback).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.delay_feedback).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.enable_dellp).upcast());
    cx.emit(ParamEvent::SetParameter(&params.enable_dellp, snapshot.enable_dellp).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.enable_dellp).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.haas_time).upcast());
    cx.emit(ParamEvent::SetParameter(&params.haas_time, snapshot.haas_time).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.haas_time).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.seed).upcast());
    cx.emit(ParamEvent::SetParameter(&params.seed, snapshot.seed).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.seed).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.input_gain_db).upcast());
    cx.emit(ParamEvent::SetParameter(&params.input_gain_db, snapshot.input_gain_db).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.input_gain_db).upcast());

    cx.emit(ParamEvent::BeginSetParameter(&params.output_gain_db).upcast());
    cx.emit(ParamEvent::SetParameter(&params.output_gain_db, snapshot.output_gain_db).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.output_gain_db).upcast());

    let target_room = snapshot_target_room(snapshot);
    cx.emit(ParamEvent::BeginSetParameter(&params.room).upcast());
    cx.emit(ParamEvent::SetParameter(&params.room, target_room).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.room).upcast());
}
fn available_user_presets() -> (Vec<String>, Vec<PathBuf>) {
    match presets::list_snapshot_files() {
        Ok(paths) => {
            let root = match presets::preset_root_dir() {
                Ok(root) => root,
                Err(err) => {
                    nih_plug::debug::nih_error!("Failed to resolve preset root for labels: {err}");
                    return (vec!["No presets found".to_string()], Vec::new());
                }
            };

            let mut paths: Vec<PathBuf> = paths;
            paths.sort_by_key(|path: &PathBuf| {
                let relative = path.strip_prefix(&root).unwrap_or(path);
                let depth_group = if relative.components().count() > 1 { 1 } else { 0 };
                let relative_key = relative.to_string_lossy().to_ascii_lowercase();
                (depth_group, relative_key)
            });

            let labels = paths
                .iter()
                .map(|path: &PathBuf| user_preset_label(&root, path))
                .collect::<Vec<String>>();

            if labels.is_empty() {
                (vec!["No presets found".to_string()], Vec::new())
            } else {
                (labels, paths)
            }
        }
        Err(err) => {
            nih_plug::debug::nih_error!("Failed to list user presets: {err}");
            (vec!["No presets found".to_string()], Vec::new())
        }
    }
}

fn user_preset_label(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let mut parts = relative
        .iter()
        .map(|part| part.to_string_lossy().to_string())
        .collect::<Vec<String>>();

    if let Some(last) = parts.last_mut() {
        if let Some(stem) = Path::new(last).file_stem().and_then(|stem| stem.to_str()) {
            *last = stem.to_string();
        }
    }

    if parts.is_empty() {
        "Unnamed preset".to_string()
    } else {
        parts.join(" / ")
    }
}
fn sync_user_preset_state_from_selection(
    user_preset_state: &Arc<SharedUserPresetState>,
    user_preset_paths: &Arc<Mutex<Vec<PathBuf>>>,
    preset_idx: usize,
) {
    let path = user_preset_paths
        .lock()
        .ok()
        .and_then(|paths| paths.get(preset_idx).cloned());

    if let Some(path) = path {
        if let Ok(snapshot) = presets::load_snapshot_from_path(&path) {
            let comparison_snapshot = snapshot_for_loaded_params(&snapshot);
            user_preset_state.set_snapshot(&comparison_snapshot);
        }
    }
}
#[derive(Lens, Clone)]
struct Data {
    params: Arc<GoldsrcPluginParams>,
    user_preset_options: Vec<String>,
    window_size_options: Vec<String>,
}

enum DataEvent {
    SetUserPresetOptions(Vec<String>),
}
impl Model for Data {
    fn event(&mut self, _: &mut EventContext, event: &mut Event) {
        event.map(|app_event, _| match app_event {
            DataEvent::SetUserPresetOptions(options) => {
                self.user_preset_options = options.clone();
            }
        });
    }
}

// ─── Layout helpers ──────────────────────────────────────────────────────────

/// A titled section containing grouped parameter rows.
fn section(cx: &mut Context, title: &str, content: impl FnOnce(&mut Context)) {
    VStack::new(cx, |cx| {
        Label::new(cx, title).class("section-label");
        content(cx);
    })
    .class("section");
}

/// Compact section used for room preset selection.
fn room_section(cx: &mut Context, title: &str, content: impl FnOnce(&mut Context)) {
    VStack::new(cx, |cx| {
        Label::new(cx, title).class("section-label");
        content(cx);
    })
    .class("room-section")
    .width(Stretch(1.0))
    .height(Auto);
}
/// A single row: label on the left, widget on the right.
fn param_row(cx: &mut Context, label: &str, widget: impl FnOnce(&mut Context)) {
    HStack::new(cx, |cx| {
        Label::new(cx, label).class("label");
        widget(cx);
    })
    .class("row");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::PluginParamsSnapshot;

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Build a snapshot that mirrors PRESETS[room_index] exactly.
    /// Preset layout: [lp, mod, size, refl, rvblp, delay, feedback, dlylp, left]
    ///
    /// Key mapping (mirrors snapshot_knobs / the on_select Room handler):
    ///   [0] lp       → enable_amplp  (>= 0.5 → true)
    ///   [1] mod      → enable_ampmod (>= 0.5 → true)
    ///   [2] size     → reverb_size
    ///   [3] refl     → reverb_feedback
    ///   [4] rvblp    → enable_revlp  (>= 0.5 → true)
    ///   [5] delay    → delay_time
    ///   [6] feedback → delay_feedback
    ///   [7] dlylp    → enable_dellp  (== 0.0 → true  — inverted!)
    ///   [8] left     → haas_time
    fn snapshot_from_preset(room_index: usize, room_value: i32) -> PluginParamsSnapshot {
        let p = goldsrc_dsp::PRESETS[room_index];
        PluginParamsSnapshot {
            room: room_value,
            reverb_mix: 0.17,
            delay_mix: 0.25,
            clip_soft: 0,
            enable_amplp: p[0] >= 0.5,
            enable_ampmod: p[1] >= 0.5,
            reverb_size: p[2],
            reverb_feedback: p[3],
            enable_revlp: p[4] >= 0.5,
            delay_time: p[5],
            delay_feedback: p[6],
            enable_dellp: p[7] == 0.0, // inverted: dlylp==0.0 means LP is active
            haas_time: p[8],
            seed: 0,
            input_gain_db: 0.0,
            output_gain_db: 0.0,
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    /// snapshot.room == CUSTOM_ROOM must short-circuit to CUSTOM_ROOM,
    /// even when all knob values perfectly match a real preset.
    #[test]
    fn explicit_custom_room_returns_custom_room() {
        // Use room 5 (tunnel) values but mark it as Custom.
        let mut s = snapshot_from_preset(5, 5);
        s.room = crate::CUSTOM_ROOM;

        assert_eq!(
            snapshot_target_room(&s),
            crate::CUSTOM_ROOM,
            "snapshot.room == CUSTOM_ROOM must always return CUSTOM_ROOM"
        );
    }

    /// Snapshots whose knobs exactly mirror a real preset entry must resolve
    /// back to that room index — tested for two rooms to cover both
    /// enable_dellp=false (dlylp=2.0) and enable_dellp=true (dlylp=0.0).
    #[test]
    fn exact_knob_match_resolves_to_room_index() {
        // Room 5 — "tunnel": PRESETS[5][7] = 2.0 → enable_dellp = false
        let s = snapshot_from_preset(5, 5);
        assert_eq!(snapshot_target_room(&s), 5, "room 5 exact match must return 5");

        // Room 23 — "cavern": PRESETS[23][7] = 0.0 → enable_dellp = true
        let s = snapshot_from_preset(23, 23);
        assert_eq!(
            snapshot_target_room(&s),
            23,
            "room 23 exact match must return 23"
        );
    }

    /// A snapshot whose room field is valid but whose knob values differ from
    /// the preset table must fall back to CUSTOM_ROOM (user-modified state).
    #[test]
    fn mismatched_knobs_fall_through_to_custom_room() {
        // Start from room 1 (generic), bump reverb_size — PRESETS[1][2] = 0.0
        let mut s = snapshot_from_preset(1, 1);
        s.reverb_size = 0.99;

        assert_eq!(
            snapshot_target_room(&s),
            crate::CUSTOM_ROOM,
            "modified knobs must resolve to CUSTOM_ROOM even if room field is valid"
        );
    }

    /// Verify the enable_dellp inversion contract: snapshot_knobs must emit
    /// the value that matches PRESETS so the array comparison succeeds.
    #[test]
    fn enable_dellp_inversion_round_trips_correctly() {
        // Room 23 (cavern): PRESETS[23][7] = 0.0 → enable_dellp should be true
        let s = snapshot_from_preset(23, 23);
        assert!(s.enable_dellp, "PRESETS[23][7]=0.0 should map to enable_dellp=true");
        assert_eq!(
            snapshot_knobs(&s)[7],
            0.0,
            "snapshot_knobs must emit 0.0 when enable_dellp=true"
        );

        // Room 5 (tunnel): PRESETS[5][7] = 2.0 → enable_dellp should be false
        let s = snapshot_from_preset(5, 5);
        assert!(!s.enable_dellp, "PRESETS[5][7]=2.0 should map to enable_dellp=false");
        assert_eq!(
            snapshot_knobs(&s)[7],
            2.0,
            "snapshot_knobs must emit 2.0 when enable_dellp=false"
        );
    }
}
























