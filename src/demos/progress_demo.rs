// ============================================================
// Rutter Framework — demos/progress_demo.rs
// Demo isolada de ProgressBar (determinado + indeterminado) e Spinner.
// FIX-5: a animação da barra indeterminada deve crescer/encolher
//        suavemente (corrigida em render/mod.rs).
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, RutterRunner, Theme, Widget};

#[derive(Default)]
pub struct ProgressDemoState {
    pub upload: f32, // 0.0 → 1.0
    pub download: f32,
    pub is_loading: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    UploadStep,
    DownloadStep,
    ToggleLoading,
    Reset,
}

pub struct ProgressDemo;

impl AppLogic for ProgressDemo {
    type State = ProgressDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        ProgressDemoState {
            upload: 0.2,
            download: 0.65,
            is_loading: true,
        }
    }

    fn view<'a>(s: &'a mut ProgressDemoState) -> Widget<'a, Msg> {
        let col = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::FlexStart),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(40.0),
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(20.0),
            },
            ..Default::default()
        };
        let bar_s = Style {
            size: Size {
                width: Dimension::length(360.0),
                height: Dimension::length(20.0),
            },
            ..Default::default()
        };
        let btn_s = Style {
            size: Size {
                width: Dimension::length(160.0),
                height: Dimension::length(36.0),
            },
            ..Default::default()
        };
        let row_s = Style {
            flex_direction: FlexDirection::Row,
            gap: Size {
                width: LengthPercentage::length(12.0),
                height: LengthPercentage::length(0.0),
            },
            ..Default::default()
        };
        let spin_s = Style {
            size: Size {
                width: Dimension::length(32.0),
                height: Dimension::length(32.0),
            },
            ..Default::default()
        };

        Widget::Column {
            style: col,
            children: vec![
                // ── Upload — determinado ─────────────────────────────
                Widget::Text {
                    content: format!("Upload: {:.0}%", s.upload * 100.0),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::ProgressBar {
                    id: 30,
                    value: s.upload,
                    indeterminate: false,
                    style: bar_s.clone(),
                },
                // ── Download — determinado ───────────────────────────
                Widget::Text {
                    content: format!("Download: {:.0}%", s.download * 100.0),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::ProgressBar {
                    id: 31,
                    value: s.download,
                    indeterminate: false,
                    style: bar_s.clone(),
                },
                // ── Indeterminado (FIX-5) ────────────────────────────
                Widget::Text {
                    content: if s.is_loading {
                        "Carregando... (indeterminado)".into()
                    } else {
                        "Pausado".into()
                    },
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::ProgressBar {
                    id: 32,
                    value: 0.0,
                    indeterminate: s.is_loading,
                    style: bar_s,
                },
                // ── Spinner ──────────────────────────────────────────
                Widget::Spinner {
                    id: 40,
                    style: spin_s,
                },
                // ── Controles ────────────────────────────────────────
                Widget::Row {
                    style: row_s,
                    children: vec![
                        Widget::Button {
                            text: "+10% Upload",
                            on_press: Msg::UploadStep,
                            style: btn_s.clone(),
                            color: None,
                            variant: rutter::ButtonVariant::Ghost,
                        },
                        Widget::Button {
                            text: "+10% Download",
                            on_press: Msg::DownloadStep,
                            style: btn_s.clone(),
                            color: None,
                            variant: rutter::ButtonVariant::Ghost,
                        },
                        Widget::Button {
                            text: if s.is_loading { "Pausar" } else { "Retomar" },
                            on_press: Msg::ToggleLoading,
                            style: btn_s.clone(),
                            color: None,
                            variant: rutter::ButtonVariant::Primary,
                        },
                        Widget::Button {
                            text: "Reset",
                            on_press: Msg::Reset,
                            style: btn_s,
                            color: None,
                            variant: rutter::ButtonVariant::Text,
                        },
                    ],
                },
            ],
        }
    }

    fn update(s: &mut ProgressDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::UploadStep => s.upload = (s.upload + 0.1).min(1.0),
            Msg::DownloadStep => s.download = (s.download + 0.1).min(1.0),
            Msg::ToggleLoading => s.is_loading = !s.is_loading,
            Msg::Reset => {
                s.upload = 0.0;
                s.download = 0.0;
                s.is_loading = false;
            }
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

pub fn run() {
    RutterRunner::<ProgressDemo>::run();
}
