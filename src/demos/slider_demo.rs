// ============================================================
// Rutter Framework — demos/slider_demo.rs
// Demo isolada do Widget::Slider.
// Exercita: step linear (1.0), step customizado, min/max.
// FIX-6: step padrão agora é 1.0, não 5.0.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, RutterRunner, Theme, Widget};

#[derive(Default)]
pub struct SliderDemoState {
    pub volume: f32,     // step 1.0 — linear
    pub balance: f32,    // step 0.5 — semi-contínuo
    pub brightness: f32, // step 5.0 — granular (exemplo de uso explícito)
}

#[derive(Debug, Clone)]
pub enum Msg {
    VolumeChanged(f32),
    BalanceChanged(f32),
    BrightnessChanged(f32),
}

pub struct SliderDemo;

impl AppLogic for SliderDemo {
    type State = SliderDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        SliderDemoState {
            volume: 50.0,
            balance: 0.0,
            brightness: 75.0,
        }
    }

    fn view<'a>(s: &'a mut SliderDemoState) -> Widget<'a, Msg> {
        let col = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Center),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(40.0),
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(24.0),
            },
            ..Default::default()
        };
        let slider_s = Style {
            size: Size {
                width: Dimension::length(360.0),
                height: Dimension::length(36.0),
            },
            ..Default::default()
        };

        Widget::Column {
            style: col,
            children: vec![
                // ── Volume: step 1.0 (linear, padrão correto — FIX-6) ──
                Widget::Text {
                    content: format!("Volume: {:.0}%", s.volume),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::Slider {
                    id: 10,
                    value: s.volume,
                    min: 0.0,
                    max: 100.0,
                    step: 1.0, // FIX-6: era 5.0 no demo original
                    on_change: Msg::VolumeChanged,
                    style: slider_s.clone(),
                    label: "",
                },
                // ── Balance: step 0.5 ─────────────────────────────────
                Widget::Text {
                    content: format!("Balance: {:.1}", s.balance),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::Slider {
                    id: 11,
                    value: s.balance,
                    min: -10.0,
                    max: 10.0,
                    step: 0.5,
                    on_change: Msg::BalanceChanged,
                    style: slider_s.clone(),
                    label: "",
                },
                // ── Brightness: step 5.0 (granular — uso explícito) ───
                Widget::Text {
                    content: format!("Brightness: {:.0}%", s.brightness),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::Slider {
                    id: 12,
                    value: s.brightness,
                    min: 0.0,
                    max: 100.0,
                    step: 5.0, // granular intencional
                    on_change: Msg::BrightnessChanged,
                    style: slider_s,
                    label: "",
                },
                Widget::Text {
                    content: "Arraste, clique na track ou use setas".into(),
                    color: None,
                    size: 11.0,
                    style: Style::default(),
                },
            ],
        }
    }

    fn update(s: &mut SliderDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::VolumeChanged(v) => s.volume = v,
            Msg::BalanceChanged(v) => s.balance = v,
            Msg::BrightnessChanged(v) => s.brightness = v,
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

pub fn run() {
    RutterRunner::<SliderDemo>::run();
}
