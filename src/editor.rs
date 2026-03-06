use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use rfd::AsyncFileDialog;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, AtomicUsize, Ordering},
        Arc,
    },
};

use crate::{presets, GoldsrcPluginParams, CUSTOM_ROOM};
use goldsrc_dsp::{PRESETS, ROOM_NAMES};
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
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        assets::register_noto_sans_light(cx);
        assets::register_noto_sans_bold(cx);

        // Load our custom dark theme on top of the defaults
        if let Err(e) = cx.add_stylesheet(include_style!("src/editor.css")) {
            nih_plug::debug::nih_error!("Failed to load editor stylesheet: {e:?}");
        }

        // Data model for parameter binding
        let mut room_options = ROOM_NAMES
            .iter()
            .enumerate()
            .map(|(i, name)| format!("{i} - {name}"))
            .collect::<Vec<_>>();
        room_options.push(format!("{} - Custom", CUSTOM_ROOM));

        let (user_preset_options, user_preset_paths) = available_user_presets();
        let user_preset_max_index = user_preset_options.len().saturating_sub(1);
        let selected_user_preset_idx = Arc::new(AtomicUsize::new(0));
        let window_size_options = window_size_labels();
        let window_size_max_index = window_size_options.len().saturating_sub(1);
        let selected_window_size_idx = Arc::new(AtomicUsize::new(0));

        Data {
            params: params.clone(),
            room_options,
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
            })
            .class("header");

            // ── Room Selection ─────────────────────────────────────────
            room_section(cx, "ROOM", |cx| {
                param_row(cx, "Room Type", |cx| {
                    PickList::new(
                        cx,
                        Data::room_options,
                        Data::params.map(|p| effective_room_index(p.as_ref())),
                        true,
                    )
                    .on_select({
                        let params = params.clone();
                        move |cx, room_idx| {
                            let room = room_idx as i32;
                            cx.emit(ParamEvent::BeginSetParameter(&params.room).upcast());
                            cx.emit(ParamEvent::SetParameter(&params.room, room).upcast());
                            cx.emit(ParamEvent::EndSetParameter(&params.room).upcast());

                            if room_idx >= PRESETS.len() {
                                return;
                            }

                            let preset = PRESETS[room_idx];

                            cx.emit(ParamEvent::BeginSetParameter(&params.enable_amplp).upcast());
                            cx.emit(
                                ParamEvent::SetParameter(&params.enable_amplp, preset[0] >= 0.5)
                                    .upcast(),
                            );
                            cx.emit(ParamEvent::EndSetParameter(&params.enable_amplp).upcast());

                            cx.emit(ParamEvent::BeginSetParameter(&params.enable_ampmod).upcast());
                            cx.emit(
                                ParamEvent::SetParameter(&params.enable_ampmod, preset[1] >= 0.5)
                                    .upcast(),
                            );
                            cx.emit(ParamEvent::EndSetParameter(&params.enable_ampmod).upcast());

                            cx.emit(ParamEvent::BeginSetParameter(&params.reverb_size).upcast());
                            cx.emit(
                                ParamEvent::SetParameter(&params.reverb_size, preset[2]).upcast(),
                            );
                            cx.emit(ParamEvent::EndSetParameter(&params.reverb_size).upcast());

                            cx.emit(
                                ParamEvent::BeginSetParameter(&params.reverb_feedback).upcast(),
                            );
                            cx.emit(
                                ParamEvent::SetParameter(&params.reverb_feedback, preset[3])
                                    .upcast(),
                            );
                            cx.emit(ParamEvent::EndSetParameter(&params.reverb_feedback).upcast());

                            cx.emit(ParamEvent::BeginSetParameter(&params.enable_revlp).upcast());
                            cx.emit(
                                ParamEvent::SetParameter(&params.enable_revlp, preset[4] >= 0.5)
                                    .upcast(),
                            );
                            cx.emit(ParamEvent::EndSetParameter(&params.enable_revlp).upcast());

                            cx.emit(ParamEvent::BeginSetParameter(&params.delay_time).upcast());
                            cx.emit(
                                ParamEvent::SetParameter(&params.delay_time, preset[5]).upcast(),
                            );
                            cx.emit(ParamEvent::EndSetParameter(&params.delay_time).upcast());

                            cx.emit(ParamEvent::BeginSetParameter(&params.delay_feedback).upcast());
                            cx.emit(
                                ParamEvent::SetParameter(&params.delay_feedback, preset[6])
                                    .upcast(),
                            );
                            cx.emit(ParamEvent::EndSetParameter(&params.delay_feedback).upcast());

                            cx.emit(ParamEvent::BeginSetParameter(&params.enable_dellp).upcast());
                            cx.emit(
                                ParamEvent::SetParameter(&params.enable_dellp, preset[7] == 0.0)
                                    .upcast(),
                            );
                            cx.emit(ParamEvent::EndSetParameter(&params.enable_dellp).upcast());

                            cx.emit(ParamEvent::BeginSetParameter(&params.haas_time).upcast());
                            cx.emit(
                                ParamEvent::SetParameter(&params.haas_time, preset[8]).upcast(),
                            );
                            cx.emit(ParamEvent::EndSetParameter(&params.haas_time).upcast());
                        }
                    })
                    .class("widget");
                });

                param_row(cx, "Window", |cx| {
                    PickList::new(
                        cx,
                        Data::window_size_options,
                        {
                            let selected_window_size_idx = selected_window_size_idx.clone();
                            Data::params.map(move |_| {
                                selected_window_size_idx
                                    .load(Ordering::Relaxed)
                                    .min(window_size_max_index)
                            })
                        },
                        true,
                    )
                    .on_select({
                        let selected_window_size_idx = selected_window_size_idx.clone();
                        move |cx, size_idx| {
                            selected_window_size_idx.store(size_idx, Ordering::Relaxed);
                            apply_window_size_preset(size_idx);
                            cx.emit(GuiContextEvent::Resize);
                        }
                    })
                    .class("widget");
                });
                param_row(cx, "Presets", |cx| {
                    Button::new(
                        cx,
                        {
                            let params = params.clone();
                            move |_| {
                                let params = params.clone();
                                std::thread::spawn(move || {
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

                                        if let Err(err) =
                                            presets::save_params_snapshot_to_path(&params, &path)
                                        {
                                            nih_plug::debug::nih_error!(
                                                "Failed to save JSON preset to {}: {err}",
                                                path.display()
                                            );
                                        }
                                    }
                                });
                            }
                        },
                        |cx| Label::new(cx, "Save"),
                    )
                    .class("preset-button widget");
                });

                param_row(cx, "Load User Presets", |cx| {
                    PickList::new(
                        cx,
                        Data::user_preset_options,
                        {
                            let selected_user_preset_idx = selected_user_preset_idx.clone();
                            Data::params.map(move |_| {
                                selected_user_preset_idx
                                    .load(Ordering::Relaxed)
                                    .min(user_preset_max_index)
                            })
                        },
                        true,
                    )
                    .on_select({
                        let params = params.clone();
                        let user_preset_paths = user_preset_paths.clone();
                        let selected_user_preset_idx = selected_user_preset_idx.clone();
                        move |cx, preset_idx| {
                            selected_user_preset_idx.store(preset_idx, Ordering::Relaxed);
                            if let Some(path) = user_preset_paths.get(preset_idx) {
                                match presets::load_snapshot_from_path(path) {
                                    Ok(snapshot) => apply_snapshot_to_params(cx, &params, &snapshot),
                                    Err(err) => nih_plug::debug::nih_error!(
                                        "Failed to load user preset from {}: {err}",
                                        path.display()
                                    ),
                                }
                            }
                        }
                    })
                    .class("widget");
                });
            });
            section(cx, "MIX", |cx| {
                param_row(cx, "Reverb Mix", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.reverb_mix).class("widget");
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

            // ── Modulation ─────────────────────────────────────────────
            section(cx, "MODULATION", |cx| {
                param_row(cx, "Amp Mod", |cx| {
                    ParamButton::new(cx, Data::params, |p| &p.enable_ampmod).with_label("Amp Mod");
                });
                param_row(cx, "Amp LPF", |cx| {
                    ParamButton::new(cx, Data::params, |p| &p.enable_amplp)
                        .with_label("Amp Mod LPF");
                });
            });

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

fn apply_snapshot_to_params(
    cx: &mut EventContext,
    params: &Arc<GoldsrcPluginParams>,
    snapshot: &presets::PluginParamsSnapshot,
) {
    cx.emit(ParamEvent::BeginSetParameter(&params.reverb_mix).upcast());
    cx.emit(ParamEvent::SetParameter(&params.reverb_mix, snapshot.reverb_mix).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.reverb_mix).upcast());

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

    let target_room = snapshot_target_room(snapshot);
    cx.emit(ParamEvent::BeginSetParameter(&params.room).upcast());
    cx.emit(ParamEvent::SetParameter(&params.room, target_room).upcast());
    cx.emit(ParamEvent::EndSetParameter(&params.room).upcast());
}
fn available_user_presets() -> (Vec<String>, Arc<Vec<PathBuf>>) {
    match presets::list_snapshot_files() {
        Ok(paths) => {
            let mut paths: Vec<PathBuf> = paths;
            paths.sort_by_key(|path: &PathBuf| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_ascii_lowercase())
                    .unwrap_or_default()
            });

            let labels = paths
                .iter()
                .map(|path: &PathBuf| {
                    path.file_stem()
                        .or_else(|| path.file_name())
                        .and_then(|s| s.to_str())
                        .unwrap_or("Unnamed preset")
                        .to_string()
                })
                .collect::<Vec<String>>();

            if labels.is_empty() {
                (vec!["No presets found".to_string()], Arc::new(Vec::new()))
            } else {
                (labels, Arc::new(paths))
            }
        }
        Err(err) => {
            nih_plug::debug::nih_error!("Failed to list user presets: {err}");
            (vec!["No presets found".to_string()], Arc::new(Vec::new()))
        }
    }
}
// ─── Data model ──────────────────────────────────────────────────────────────

#[derive(Lens, Clone)]
struct Data {
    params: Arc<GoldsrcPluginParams>,
    room_options: Vec<String>,
    user_preset_options: Vec<String>,
    window_size_options: Vec<String>,
}

impl Model for Data {}

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
