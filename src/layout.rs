// ============================================================
// Rutter Framework — layout.rs
// ============================================================

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};
use taffy::prelude::*;
use winit::dpi::PhysicalSize;

use crate::engine::widget_state::WidgetState;
use crate::widget::Widget;

const ACCORDION_HEADER_H: f32 = 44.0;

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
        Widget::Container { child, style, .. }
        | Widget::ScrollView { child, style, .. }
        | Widget::Tooltip { child, style, .. } => {
            let id = build_taffy_tree(taffy, child, fs.clone(), widget_states);
            taffy.new_with_children(style.clone(), &[id]).unwrap()
        }
        Widget::Accordion {
            child,
            style,
            expanded,
            ..
        } => {
            let mut s = style.clone();
            s.padding.top = LengthPercentage::length(ACCORDION_HEADER_H);
            if *expanded {
                let id = build_taffy_tree(taffy, child, fs.clone(), widget_states);
                taffy.new_with_children(s, &[id]).unwrap()
            } else {
                s.size.height = Dimension::length(ACCORDION_HEADER_H);
                taffy.new_leaf(s).unwrap()
            }
        }
        Widget::Modal {
            child,
            style,
            visible,
            ..
        }
        | Widget::Dialog {
            child,
            style,
            visible,
            ..
        } => {
            if *visible {
                let id = build_taffy_tree(taffy, child, fs.clone(), widget_states);
                taffy.new_with_children(style.clone(), &[id]).unwrap()
            } else {
                taffy
                    .new_leaf(Style {
                        size: Size::zero(),
                        ..style.clone()
                    })
                    .unwrap()
            }
        }
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
        Widget::Button { style, .. }
        | Widget::TextInput { style, .. }
        | Widget::TextArea { style, .. }
        | Widget::SearchBar { style, .. }
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
