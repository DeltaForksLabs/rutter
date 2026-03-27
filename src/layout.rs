// ============================================================
// Rutter Framework — layout.rs  (Fase 4)
// ============================================================

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};
use taffy::prelude::*;
use winit::dpi::PhysicalSize;

use crate::engine::widget_state::WidgetState;
use crate::widget::Widget;

pub const OPTION_HEIGHT: f32 = 32.0;
pub const SCROLLBAR_W: f32 = 8.0;

#[derive(Debug, Clone)]
pub struct TextContext {
    pub content: String,
    pub font_size: f32,
}

#[derive(Debug, Clone, Default)]
pub enum RutterContext {
    #[default]
    None,
    Text(TextContext),
}

pub fn build_taffy_tree<'a, Msg>(
    taffy: &mut TaffyTree<RutterContext>,
    widget: &Widget<'a, Msg>,
    fs: Rc<RefCell<FontSystem>>,
    widget_states: &HashMap<u64, WidgetState>,
) -> NodeId {
    match widget {
        Widget::Column { children, style } => {
            let s = Style {
                flex_direction: FlexDirection::Column,
                ..style.clone()
            };
            let ids: Vec<_> = children
                .iter()
                .map(|c| build_taffy_tree(taffy, c, fs.clone(), widget_states))
                .collect();
            taffy.new_with_children(s, &ids).unwrap()
        }
        Widget::Row { children, style } => {
            let s = Style {
                flex_direction: FlexDirection::Row,
                ..style.clone()
            };
            let ids: Vec<_> = children
                .iter()
                .map(|c| build_taffy_tree(taffy, c, fs.clone(), widget_states))
                .collect();
            taffy.new_with_children(s, &ids).unwrap()
        }
        Widget::Container { child, style, .. } => {
            let id = build_taffy_tree(taffy, child, fs.clone(), widget_states);
            taffy.new_with_children(style.clone(), &[id]).unwrap()
        }
        Widget::ScrollView { child, style, .. } => {
            let id = build_taffy_tree(taffy, child, fs.clone(), widget_states);
            taffy.new_with_children(style.clone(), &[id]).unwrap()
        }
        Widget::Tooltip { child, style, .. } => {
            let id = build_taffy_tree(taffy, child, fs.clone(), widget_states);
            taffy.new_with_children(style.clone(), &[id]).unwrap()
        }

        // Modal: filho ocupa todo o espaço (posição via render)
        Widget::Modal {
            child,
            style,
            visible,
            ..
        } => {
            if *visible {
                let id = build_taffy_tree(taffy, child, fs.clone(), widget_states);
                taffy.new_with_children(style.clone(), &[id]).unwrap()
            } else {
                // Quando invisível, folha de tamanho zero
                taffy
                    .new_leaf(Style {
                        size: Size::zero(),
                        ..style.clone()
                    })
                    .unwrap()
            }
        }

        // Select: expande quando aberto
        Widget::Select {
            id, options, style, ..
        } => {
            let is_open = widget_states
                .get(id)
                .and_then(|s| s.as_select())
                .map(|s| s.is_open)
                .unwrap_or(false);
            let s = if is_open {
                let closed_h = extract_height(style);
                Style {
                    size: Size {
                        height: Dimension::length(closed_h + options.len() as f32 * OPTION_HEIGHT),
                        ..style.size
                    },
                    ..style.clone()
                }
            } else {
                style.clone()
            };
            taffy.new_leaf(s).unwrap()
        }

        Widget::Text {
            content,
            style,
            size,
            ..
        } => taffy
            .new_leaf_with_context(
                style.clone(),
                RutterContext::Text(TextContext {
                    content: content.clone(),
                    font_size: *size,
                }),
            )
            .unwrap(),

        // VirtualList: tamanho fixo definido pelo Style
        Widget::Button { style, .. }
        | Widget::TextInput { style, .. }
        | Widget::Checkbox { style, .. }
        | Widget::Switch { style, .. }
        | Widget::Radio { style, .. }
        | Widget::Slider { style, .. }
        | Widget::ProgressBar { style, .. }
        | Widget::Spinner { style, .. }
        | Widget::Image { style, .. }
        | Widget::Divider { style, .. }
        | Widget::Spacer { style, .. }
        | Widget::TabBar { style, .. }
        | Widget::VirtualList { style, .. } => taffy.new_leaf(style.clone()).unwrap(),

        // Toast usa tamanho 0 no layout (desenhado via overlay no render)
        Widget::Toast { .. } => taffy
            .new_leaf(Style {
                size: Size::zero(),
                ..Default::default()
            })
            .unwrap(),
    }
}

pub fn compute_layout(
    taffy: &mut TaffyTree<RutterContext>,
    root: NodeId,
    size: PhysicalSize<u32>,
    fs_rc: Rc<RefCell<FontSystem>>,
) {
    let available = Size {
        width: AvailableSpace::Definite(size.width as f32),
        height: AvailableSpace::Definite(size.height as f32),
    };
    taffy
        .compute_layout_with_measure(root, available, |known, available, _, ctx, _| {
            let Some(RutterContext::Text(t)) = ctx else {
                return Size::ZERO;
            };
            let mut fs = fs_rc.borrow_mut();
            let mut buf = Buffer::new(&mut fs, Metrics::new(t.font_size, t.font_size * 1.2));
            match available.width {
                AvailableSpace::Definite(px) => buf.set_size(&mut fs, Some(px), None),
                AvailableSpace::MaxContent => buf.set_size(&mut fs, None, None),
                AvailableSpace::MinContent => buf.set_size(&mut fs, Some(0.0), None),
            }
            buf.set_text(&mut fs, &t.content, &Attrs::new(), Shaping::Advanced, None);
            buf.shape_until_scroll(&mut fs, true);
            let (w, h) = buf.size();
            Size {
                width: known.width.unwrap_or(w.unwrap_or(0.0)),
                height: known.height.unwrap_or(h.unwrap_or(0.0)),
            }
        })
        .unwrap();
}

fn extract_height(_style: &Style) -> f32 {
    return 40.0;
}

// ── Testes ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::{Orientation, ToastKind};
    use cosmic_text::FontSystem;
    use taffy::prelude::Style;

    fn fs() -> Rc<RefCell<FontSystem>> {
        Rc::new(RefCell::new(FontSystem::new()))
    }
    fn empty() -> HashMap<u64, WidgetState> {
        HashMap::new()
    }

    #[test]
    fn tabbar_is_leaf() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::TabBar {
            id: 1,
            tabs: &["A", "B"],
            active: 0,
            on_change: |_| (),
            style: Style::default(),
        };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty());
        assert_eq!(taffy.child_count(node), 0);
    }

    #[test]
    fn modal_invisible_has_zero_size() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::Modal {
            id: 1,
            visible: false,
            on_dismiss: None,
            style: Style::default(),
            child: Box::new(Widget::Spacer {
                style: Style::default(),
            }),
        };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty());
        let style = taffy.style(node).unwrap();
        assert_eq!(style.size.width, Dimension::length(0.0));
        assert_eq!(style.size.height, Dimension::length(0.0));
    }

    #[test]
    fn modal_visible_has_child() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::Modal {
            id: 1,
            visible: true,
            on_dismiss: None,
            style: Style::default(),
            child: Box::new(Widget::Spacer {
                style: Style::default(),
            }),
        };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty());
        assert_eq!(taffy.child_count(node), 1);
    }

    #[test]
    fn toast_has_zero_layout_size() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::Toast {
            id: 1,
            message: "Hi",
            kind: ToastKind::Info,
            duration_ms: 3000,
            on_dismiss: None,
        };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty());
        let style = taffy.style(node).unwrap();
        assert_eq!(style.size.width, Dimension::length(0.0));
    }

    #[test]
    fn virtual_list_is_leaf() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::VirtualList {
            id: 1,
            item_height: 30.0,
            item_count: 100,
            items: &|_| None,
            on_select: |_| (),
            style: Style {
                size: Size {
                    width: Dimension::length(300.0),
                    height: Dimension::length(200.0),
                },
                ..Default::default()
            },
        };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty());
        assert_eq!(taffy.child_count(node), 0);
    }

    #[test]
    fn select_open_expands() {
        use crate::engine::widget_state::{SelectState, WidgetState};
        let mut states: HashMap<u64, WidgetState> = HashMap::new();
        states.insert(
            99,
            WidgetState::Select(SelectState {
                is_open: true,
                hovered_option: None,
            }),
        );
        let mut taffy = TaffyTree::new();
        let base = Style {
            size: Size {
                width: Dimension::length(200.0),
                height: Dimension::length(40.0),
            },
            ..Default::default()
        };
        let w: Widget<()> = Widget::Select {
            id: 99,
            options: &["A", "B", "C"],
            selected_index: 0,
            on_change: |_| (),
            style: base,
            label: "",
            placeholder: "",
        };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &states);
        let style = taffy.style(node).unwrap();
        let expected = 40.0 + 3.0 * OPTION_HEIGHT;
        assert_eq!(style.size.height, Dimension::length(expected));
    }

    #[test]
    fn column_with_all_phase4_widgets() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::Column {
            style: Style::default(),
            children: vec![
                Widget::TabBar {
                    id: 1,
                    tabs: &["A", "B"],
                    active: 0,
                    on_change: |_| (),
                    style: Style::default(),
                },
                Widget::Divider {
                    style: Style::default(),
                    orientation: Orientation::Horizontal,
                },
                Widget::Spacer {
                    style: Style::default(),
                },
                Widget::VirtualList {
                    id: 2,
                    item_height: 30.0,
                    item_count: 10,
                    items: &|_| None,
                    on_select: |_| (),
                    style: Style::default(),
                },
            ],
        };
        let root = build_taffy_tree(&mut taffy, &w, fs(), &empty());
        assert_eq!(taffy.child_count(root), 4);
    }
}
