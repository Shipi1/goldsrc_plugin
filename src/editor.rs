use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

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
                    ParamSlider::new(cx, Data::params, |p| &p.room)
                        .set_style(ParamSliderStyle::FromLeft)
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
