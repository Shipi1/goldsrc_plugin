use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use goldsrc_dsp::{PRESETS, ROOM_NAMES};
use crate::GoldsrcPluginParams;

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (500, 580))
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
        Data {
            params: params.clone(),
            room_options: ROOM_NAMES
                .iter()
                .enumerate()
                .map(|(i, name)| format!("{i} - {name}"))
                .collect(),
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
            section(cx, "ROOM", |cx| {
                param_row(cx, "Room Type", |cx| {
                    PickList::new(
                        cx,
                        Data::room_options,
                        Data::params.map(|p| p.room.value().max(0) as usize),
                        true,
                    )
                    .on_select({
                        let params = params.clone();
                        move |cx, room_idx| {
                            let room = room_idx as i32;
                            let preset = PRESETS[room_idx.min(PRESETS.len() - 1)];
                            cx.emit(ParamEvent::BeginSetParameter(&params.room).upcast());
                            cx.emit(ParamEvent::SetParameter(&params.room, room).upcast());
                            cx.emit(ParamEvent::EndSetParameter(&params.room).upcast());

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

                            cx.emit(ParamEvent::BeginSetParameter(&params.reverb_feedback).upcast());
                            cx.emit(
                                ParamEvent::SetParameter(&params.reverb_feedback, preset[3]).upcast(),
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
                                ParamEvent::SetParameter(&params.delay_feedback, preset[6]).upcast(),
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
            });

            // ── Mix Controls ───────────────────────────────────────────
            section(cx, "MIX", |cx| {
                param_row(cx, "Reverb Mix", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.reverb_mix)
                        .class("widget");
                });
                param_row(cx, "Echo Level", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.delay_mix)
                        .class("widget");
                });
            });

            // ── Reverb ─────────────────────────────────────────────────
            section(cx, "REVERB", |cx| {
                param_row(cx, "Size", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.reverb_size)
                        .class("widget");
                });
                param_row(cx, "Feedback", |cx| {
                    ParamSlider::new(cx, Data::params, |p| &p.reverb_feedback)
                        .class("widget");
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
                    ParamSlider::new(cx, Data::params, |p| &p.delay_feedback)
                        .class("widget");
                });
                param_row(cx, "Low-Pass", |cx| {
                    ParamButton::new(cx, Data::params, |p| &p.enable_dellp)
                        .with_label("Echo LPF");
                });
            });

            // ── Modulation ─────────────────────────────────────────────
            section(cx, "MODULATION", |cx| {
                param_row(cx, "Amp Mod", |cx| {
                    ParamButton::new(cx, Data::params, |p| &p.enable_ampmod)
                        .with_label("Amp Mod");
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
        })
        .class("main");
    })
}

// ─── Data model ──────────────────────────────────────────────────────────────

#[derive(Lens, Clone)]
struct Data {
    params: Arc<GoldsrcPluginParams>,
    room_options: Vec<String>,
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

/// A single row: label on the left, widget on the right.
fn param_row(cx: &mut Context, label: &str, widget: impl FnOnce(&mut Context)) {
    HStack::new(cx, |cx| {
        Label::new(cx, label).class("label");
        widget(cx);
    })
    .class("row");
}
