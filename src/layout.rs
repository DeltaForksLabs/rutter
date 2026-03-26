// ============================================================
// Rutter Framework — layout.rs  (Fase 3)
//
// Novidades:
//   • build_taffy_tree recebe widget_states para que Select
//     possa expandir seu nó quando aberto.
//   • Novos widgets mapeados para folhas ou com filhos.
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
pub const SCROLLBAR_W:   f32 = 8.0;

// ── Contexto Taffy ───────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TextContext { pub content: String, pub font_size: f32 }

#[derive(Debug, Clone, Default)]
pub enum RutterContext {
    #[default] None,
    Text(TextContext),
}

// ── Construção da árvore ─────────────────────────────────────

pub fn build_taffy_tree<'a, Msg>(
    taffy:         &mut TaffyTree<RutterContext>,
    widget:        &Widget<'a, Msg>,
    fs:            Rc<RefCell<FontSystem>>,
    widget_states: &HashMap<u64, WidgetState>,
) -> NodeId {
    match widget {
        Widget::Column { children, style } => {
            let s = Style { flex_direction: FlexDirection::Column, ..style.clone() };
            let ids: Vec<_> = children.iter()
                .map(|c| build_taffy_tree(taffy, c, fs.clone(), widget_states))
                .collect();
            taffy.new_with_children(s, &ids).unwrap()
        }
        Widget::Row { children, style } => {
            let s = Style { flex_direction: FlexDirection::Row, ..style.clone() };
            let ids: Vec<_> = children.iter()
                .map(|c| build_taffy_tree(taffy, c, fs.clone(), widget_states))
                .collect();
            taffy.new_with_children(s, &ids).unwrap()
        }
        Widget::Container { child, style, .. } => {
            let id = build_taffy_tree(taffy, child, fs.clone(), widget_states);
            taffy.new_with_children(style.clone(), &[id]).unwrap()
        }

        // ScrollView: único filho (o conteúdo)
        Widget::ScrollView { child, style, .. } => {
            let child_id = build_taffy_tree(taffy, child, fs.clone(), widget_states);
            taffy.new_with_children(style.clone(), &[child_id]).unwrap()
        }

        // Tooltip: único filho (o widget decorado)
        Widget::Tooltip { child, style, .. } => {
            let child_id = build_taffy_tree(taffy, child, fs.clone(), widget_states);
            taffy.new_with_children(style.clone(), &[child_id]).unwrap()
        }

        // Select: altera height quando aberto para acomodar as opções inline
        Widget::Select { id, options, style, .. } => {
            let is_open = widget_states.get(id)
                .and_then(|s| s.as_select())
                .map(|s| s.is_open)
                .unwrap_or(false);

            let s = if is_open {
                let closed_h = extract_height(style);
                let total_h  = closed_h + options.len() as f32 * OPTION_HEIGHT;
                Style {
                    size: Size {
                        height: Dimension::length(total_h),
                        ..style.size
                    },
                    ..style.clone()
                }
            } else {
                style.clone()
            };
            taffy.new_leaf(s).unwrap()
        }

        // Texto: contexto de medição intrinsic
        Widget::Text { content, style, size, .. } => {
            taffy.new_leaf_with_context(style.clone(), RutterContext::Text(TextContext {
                content: content.clone(), font_size: *size,
            })).unwrap()
        }

        // Todos os outros widgets são folhas simples
        Widget::Button       { style, .. }
        | Widget::TextInput  { style, .. }
        | Widget::Checkbox   { style, .. }
        | Widget::Switch     { style, .. }
        | Widget::Radio      { style, .. }
        | Widget::Slider     { style, .. }
        | Widget::ProgressBar{ style, .. }
        | Widget::Spinner    { style, .. }
        | Widget::Image      { style, .. }
        | Widget::Divider    { style, .. }
        | Widget::Spacer     { style, .. } => {
            taffy.new_leaf(style.clone()).unwrap()
        }
    }
}

// ── Cálculo de layout ────────────────────────────────────────

pub fn compute_layout(
    taffy:  &mut TaffyTree<RutterContext>,
    root:   NodeId,
    size:   PhysicalSize<u32>,
    fs_rc:  Rc<RefCell<FontSystem>>,
) {
    let available = Size {
        width:  AvailableSpace::Definite(size.width  as f32),
        height: AvailableSpace::Definite(size.height as f32),
    };
    taffy.compute_layout_with_measure(root, available, |known, available, _, ctx, _| {
        let Some(RutterContext::Text(t)) = ctx else { return Size::ZERO; };
        let mut fs  = fs_rc.borrow_mut();
        let mut buf = Buffer::new(&mut fs, Metrics::new(t.font_size, t.font_size * 1.2));
        match available.width {
            AvailableSpace::Definite(px) => buf.set_size(&mut fs, Some(px), None),
            AvailableSpace::MaxContent   => buf.set_size(&mut fs, None,     None),
            AvailableSpace::MinContent   => buf.set_size(&mut fs, Some(0.0),None),
        }
        buf.set_text(&mut fs, &t.content, &Attrs::new(), Shaping::Advanced, None);
        buf.shape_until_scroll(&mut fs, true);
        let (w, h) = buf.size();
        Size {
            width:  known.width.unwrap_or(w.unwrap_or(0.0)),
            height: known.height.unwrap_or(h.unwrap_or(0.0)),
        }
    }).unwrap();
}

// ── Helper interno ───────────────────────────────────────────

fn extract_height(_style: &Style) -> f32 {
    return 40.0 // Fallback seguro para a altura base do Select

}

// ── Testes ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::FontSystem;
    use taffy::prelude::Style;
    use crate::widget::{Orientation};

    fn fs() -> Rc<RefCell<FontSystem>> {
        Rc::new(RefCell::new(FontSystem::new()))
    }
    fn empty_states() -> HashMap<u64, WidgetState> { HashMap::new() }

    #[test]
    fn spacer_is_leaf() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::Spacer { style: Style::default() };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
        assert_eq!(taffy.child_count(node), 0);
    }

    #[test]
    fn divider_is_leaf() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::Divider { style: Style::default(), orientation: Orientation::Horizontal };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
        assert_eq!(taffy.child_count(node), 0);
    }

    #[test]
    fn checkbox_is_leaf() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::Checkbox { checked: false, on_change: |_| (), label: "x", style: Style::default() };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
        assert_eq!(taffy.child_count(node), 0);
    }

    #[test]
    fn switch_is_leaf() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::Switch { checked: true, on_change: |_| (), style: Style::default() };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
        assert_eq!(taffy.child_count(node), 0);
    }

    #[test]
    fn slider_is_leaf() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::Slider { id: 1, value: 0.5, min: 0.0, max: 1.0, step: 0.1, on_change: |_| (), style: Style::default(), label: "" };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
        assert_eq!(taffy.child_count(node), 0);
    }

    #[test]
    fn progress_bar_is_leaf() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::ProgressBar { value: 0.6, indeterminate: false, style: Style::default() };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
        assert_eq!(taffy.child_count(node), 0);
    }

    #[test]
    fn spinner_is_leaf() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::Spinner { id: 2, style: Style::default() };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
        assert_eq!(taffy.child_count(node), 0);
    }

    #[test]
    fn scroll_view_has_one_child() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::ScrollView {
            id: 1,
            style: Style::default(),
            child: Box::new(Widget::Spacer { style: Style::default() }),
        };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
        assert_eq!(taffy.child_count(node), 1);
    }

    #[test]
    fn tooltip_has_one_child() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::Tooltip {
            text: "hint",
            style: Style::default(),
            child: Box::new(Widget::Spacer { style: Style::default() }),
        };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
        assert_eq!(taffy.child_count(node), 1);
    }

    #[test]
    fn select_closed_uses_original_height() {
        let mut taffy = TaffyTree::new();
        let base_style = Style {
            size: Size { width: Dimension::length(200.0), height: Dimension::length(40.0) },
            ..Default::default()
        };
        let w: Widget<()> = Widget::Select {
            id: 10, options: &["A","B","C"], selected_index: 0,
            on_change: |_| (), style: base_style, label: "", placeholder: "",
        };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
        let style = taffy.style(node).unwrap();
        // Altura deve ser 40 (fechado)
        assert_eq!(style.size.height, Dimension::length(40.0));
    }

    #[test]
    fn select_open_expands_height() {
        use crate::engine::widget_state::{SelectState, WidgetState};
        let mut states: HashMap<u64, WidgetState> = HashMap::new();
        states.insert(10, WidgetState::Select(SelectState { is_open: true, hovered_option: None }));

        let mut taffy = TaffyTree::new();
        let base_style = Style {
            size: Size { width: Dimension::length(200.0), height: Dimension::length(40.0) },
            ..Default::default()
        };
        let w: Widget<()> = Widget::Select {
            id: 10, options: &["A","B","C"], selected_index: 0,
            on_change: |_| (), style: base_style, label: "", placeholder: "",
        };
        let node = build_taffy_tree(&mut taffy, &w, fs(), &states);
        let style = taffy.style(node).unwrap();
        // Altura deve ser 40 + 3 * 32 = 136
        let expected = 40.0 + 3.0 * OPTION_HEIGHT;
        assert_eq!(style.size.height, Dimension::length(expected));
    }

    #[test]
    fn column_with_new_widgets() {
        let mut taffy = TaffyTree::new();
        let w: Widget<()> = Widget::Column {
            style: Style::default(),
            children: vec![
                Widget::Checkbox { checked: false, on_change: |_| (), label: "a", style: Style::default() },
                Widget::Switch   { checked: true,  on_change: |_| (), style: Style::default() },
                Widget::ProgressBar { value: 0.5, indeterminate: false, style: Style::default() },
                Widget::Spacer   { style: Style::default() },
            ],
        };
        let root = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
        assert_eq!(taffy.child_count(root), 4);
    }
}
