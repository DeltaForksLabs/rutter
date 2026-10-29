// ============================================================
// Rutter Framework — demos/scroll_demo.rs
// Demo isolada de Widget::ScrollView.
// Scroll por roda, navegação no ancestral do controle focado e trilha/polegar.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use taffy::prelude::*;

use rutter::{AppLogic, RutterRunner, Theme, Widget};

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

const ITEM_COUNT: usize = 60;

#[derive(Default)]
pub struct ScrollDemoState {
    pub theme: ExampleTheme,
    pub selected: Vec<usize>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    ItemToggled(usize),
}

pub struct ScrollDemo;

impl AppLogic for ScrollDemo {
    type State = ScrollDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        ScrollDemoState::default()
    }

    fn view<'a>(s: &'a mut ScrollDemoState) -> Widget<'a, Msg> {
        let root = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(spacing::XL),
            gap: Size::length(spacing::LG),
            ..Default::default()
        };
        // ScrollView de altura fixa com conteúdo longo
        let scrollbar_gutter = rutter::layout::SCROLLBAR_W + 2.0 + spacing::SM;
        let scroll_s = Style {
            // The outer page also scrolls on short windows. Keep the inner
            // track clear of its physical-right track, including narrow widths.
            size: Size {
                width: Dimension::auto(),
                height: Dimension::length(300.0),
            },
            margin: Rect {
                right: LengthPercentageAuto::length(scrollbar_gutter),
                ..Rect::zero()
            },
            ..responsive_width(400.0, Dimension::length(300.0))
        };
        // Intrinsic document height includes the real item gaps, not a guessed extent.
        let inner_col = Style {
            flex_direction: FlexDirection::Column,
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::auto(),
            },
            flex_shrink: 0.0,
            gap: Size::length(spacing::XS),
            // The track occupies 8px, inset 2px from the physical right edge
            // even in RTL. Reserve its lane plus an SM gap in the document.
            padding: Rect {
                right: LengthPercentage::length(scrollbar_gutter),
                ..Rect::zero()
            },
            ..Default::default()
        };

        // Generate independently toggleable items inside the scroll view.
        let items: Vec<Widget<Msg>> = (0..ITEM_COUNT)
            .map(|index| {
                let selected = s.selected.binary_search(&index).is_ok();
                Widget::Button {
                    text: if selected {
                        "✓ Item selecionado"
                    } else {
                        "Item da lista"
                    },
                    on_press: Msg::ItemToggled(index),
                    style: Style {
                        flex_shrink: 0.0,
                        size: Size {
                            width: Dimension::percent(1.0),
                            height: Dimension::length(40.0),
                        },
                        ..Default::default()
                    },
                    color: None,
                    variant: if selected {
                        rutter::ButtonVariant::Primary
                    } else {
                        rutter::ButtonVariant::Ghost
                    },
                }
            })
            .collect();

        super::layout::page("ScrollView", Widget::Column {
            style: root,
            children: vec![
                example_theme_selector(s.theme, Msg::ThemeChanged),
                Widget::Text {
                    content: "ScrollView — roda do mouse, foco pelo teclado e seleção múltipla"
                        .into(),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::Text {
                    content: scroll_selection_caption(&s.selected),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::ScrollView {
                    id: 70,
                    style: scroll_s,
                    child: Box::new(Widget::Column {
                        style: inner_col,
                        children: items,
                    }),
                },
                Widget::Text {
                    content: "Clique em itens para alternar a seleção; use ↑↓, PageUp/PageDown e Home/End para navegar."
                        .into(),
                    color: None,
                    size: 11.0,
                    style: Style::default(),
                },
            ],
        })
    }

    fn update(s: &mut ScrollDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::ThemeChanged(theme) => s.theme = theme,
            Msg::ItemToggled(index) => toggle_scroll_selection(&mut s.selected, index),
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn toggle_scroll_selection(selected: &mut Vec<usize>, index: usize) {
    match selected.binary_search(&index) {
        Ok(position) => {
            selected.remove(position);
        }
        Err(position) => selected.insert(position, index),
    }
}

fn scroll_selection_caption(selected: &[usize]) -> String {
    match selected {
        [] => "Nenhum item selecionado".into(),
        [index] => format!("Selecionado: item #{}", index + 1),
        indices => format!("{} itens selecionados", indices.len()),
    }
}

pub fn run() {
    RutterRunner::<ScrollDemo>::run();
}

#[cfg(test)]
mod tests {
    use super::{scroll_selection_caption, toggle_scroll_selection};

    #[test]
    fn scroll_selection_toggles_and_stays_sorted() {
        let mut selected = vec![1, 4];

        toggle_scroll_selection(&mut selected, 3);
        toggle_scroll_selection(&mut selected, 4);

        assert_eq!(selected, vec![1, 3]);
        assert_eq!(scroll_selection_caption(&selected), "2 itens selecionados");
    }
}

#[cfg(test)]
mod geometry_tests {
    //! Real demo geometry and software-raster clipping, including the outer page.
    use super::*;
    use cosmic_text::SwashCache;
    use rutter::engine::widget_state::{ScrollState, WidgetState};
    use rutter::layout::{SCROLLBAR_W, build_taffy_tree, compute_layout};
    use rutter::render::hit_test::{HitResult, find_scrollbar_drag_hit, hit_test};
    use rutter::render::{RichTextRenderer, draw_widgets, text::TextBufferCache};
    use skia_safe::{Contains, Point, Rect as Bounds, surfaces};
    use std::{cell::RefCell, collections::HashMap, rc::Rc};
    use winit::dpi::PhysicalSize;

    #[test]
    fn task63_real_scroll_demo_extent_and_raster_clip_matrix() {
        let fonts = Rc::new(RefCell::new(FontSystem::new()));
        for (width, height) in [(320, 260), (800, 260), (320, 700), (800, 700)] {
            for theme in [ExampleTheme::Light, ExampleTheme::Dark] {
                let mut state = ScrollDemoState {
                    theme,
                    selected: vec![1, 59],
                };
                let view = ScrollDemo::view(&mut state);
                let mut unselected = ScrollDemoState {
                    theme,
                    selected: vec![],
                };
                let unselected_view = ScrollDemo::view(&mut unselected);
                let mut tree = TaffyTree::new();
                let root = build_taffy_tree(&mut tree, &view, fonts.clone(), &HashMap::new());
                compute_layout(
                    &mut tree,
                    root,
                    PhysicalSize::new(width, height),
                    fonts.clone(),
                    &RichTextRenderer::default(),
                );
                let page_node = tree.children(root).unwrap()[1];
                let document_node = tree.children(page_node).unwrap()[0];
                let scroll_node = tree.children(document_node).unwrap()[2];
                let content_node = tree.children(scroll_node).unwrap()[0];
                let page = tree.layout(page_node).unwrap();
                let document = tree.layout(document_node).unwrap();
                let scroll = tree.layout(scroll_node).unwrap();
                let content = tree.layout(content_node).unwrap();
                assert_eq!(scroll.size.height, 300.0);
                assert_eq!(content.size.height, 2636.0);
                let outer_track_left = page.size.width - SCROLLBAR_W - 2.0;
                assert!(scroll.location.x + scroll.size.width + spacing::SM <= outer_track_left);
                assert_eq!(tree.children(content_node).unwrap().len(), 60);
                for (index, node) in tree.children(content_node).unwrap().iter().enumerate() {
                    let item = tree.layout(*node).unwrap();
                    assert_eq!(item.location.y, index as f32 * 44.0);
                    assert_eq!(item.size.height, 40.0);
                    assert_eq!(item.location.x, 0.0);
                    assert!(item.size.width > 0.0);
                    let track_left = scroll.size.width - SCROLLBAR_W - 2.0;
                    assert_eq!(
                        track_left - (item.location.x + item.size.width),
                        spacing::SM
                    );
                }
                // Reveal the top of the inner viewport in the short outer document.
                let page_max_offset = (document.size.height - page.size.height).max(0.0);
                let page_offset = scroll.location.y.min(page_max_offset);
                let inner = Bounds::from_xywh(
                    page.location.x + document.location.x + scroll.location.x,
                    page.location.y + document.location.y + scroll.location.y - page_offset,
                    scroll.size.width,
                    scroll.size.height,
                );
                let states_at = |offset, outer_offset| {
                    HashMap::from([
                        (
                            super::super::layout::PAGE_SCROLL_ID,
                            WidgetState::Scroll(ScrollState {
                                offset_y: outer_offset,
                                content_height: document.size.height,
                                viewport_h: page.size.height,
                            }),
                        ),
                        (
                            70,
                            WidgetState::Scroll(ScrollState {
                                offset_y: offset,
                                content_height: 2636.0,
                                viewport_h: 300.0,
                            }),
                        ),
                    ])
                };
                // The card edge, empty gutter, track jump and thumb use the same
                // live coordinates, including when both ancestors have scrolled.
                for outer_offset in [page_offset, (page_offset + 44.0).min(page_max_offset)] {
                    let top = inner.top - (outer_offset - page_offset);
                    let track_x = inner.right - SCROLLBAR_W / 2.0 - 2.0;
                    let card_right = inner.right - SCROLLBAR_W - 2.0 - spacing::SM;
                    for offset in [0.0, 44.0, 1000.0, 2336.0] {
                        let states = states_at(offset, outer_offset);
                        let y = top + 60.0;
                        let hit = |x| {
                            hit_test(
                                &view,
                                &tree,
                                root,
                                Point::new(x, y),
                                Point::default(),
                                &states,
                            )
                        };
                        let expected = ((offset + 60.0) / 44.0) as usize;
                        assert!(matches!(hit(card_right - 1.0), Some(HitResult::Message {
                            msg: Msg::ItemToggled(index), ..
                        }) if index == expected));
                        for x in [card_right + spacing::SM / 2.0, track_x] {
                            assert!(matches!(hit(x), Some(HitResult::ScrollFocus(70))));
                        }
                        assert!(
                            find_scrollbar_drag_hit(
                                &view,
                                &tree,
                                root,
                                Point::new(card_right + spacing::SM / 2.0, y),
                                Point::default(),
                                &states
                            )
                            .is_none()
                        );
                        let jump = find_scrollbar_drag_hit(
                            &view,
                            &tree,
                            root,
                            Point::new(track_x, y),
                            Point::default(),
                            &states,
                        )
                        .unwrap();
                        assert_eq!(jump.id, 70);
                        assert_eq!(jump.content_extent - jump.viewport_extent, 2336.0);
                        let thumb_h = (300.0_f32 * 300.0 / 2636.0).max(20.0);
                        let expected = (60.0 - thumb_h / 2.0) / (300.0 - thumb_h) * 2336.0;
                        assert!((jump.start_offset - expected).abs() < 0.001);
                        let thumb_y = offset / 2336.0 * (300.0 - thumb_h) + thumb_h / 2.0;
                        if top + thumb_y >= page.location.y
                            && top + thumb_y < page.location.y + page.size.height
                        {
                            let thumb = find_scrollbar_drag_hit(
                                &view,
                                &tree,
                                root,
                                Point::new(track_x, top + thumb_y),
                                Point::default(),
                                &states,
                            )
                            .unwrap();
                            assert_eq!(thumb.id, 70);
                            assert_eq!(thumb.start_offset, offset);
                        }
                    }
                }
                for scale in [1.0, 1.5, 2.0] {
                    // In a short page, reveal the bottom of the inner viewport so
                    // the selected final item is actually visible in the raster.
                    let bottom_reveal = |offset| {
                        if offset == 2336.0 {
                            (300.0 - page.size.height)
                                .max(0.0)
                                .min(page_max_offset - page_offset)
                        } else {
                            0.0
                        }
                    };
                    let render = |painted_view: &Widget<'_, Msg>, offset| {
                        let states = states_at(offset, page_offset + bottom_reveal(offset));
                        let mut surface = surfaces::raster_n32_premul((
                            (width as f32 * scale) as i32,
                            (height as f32 * scale) as i32,
                        ))
                        .unwrap();
                        surface.canvas().clear(skia_safe::Color::MAGENTA);
                        surface.canvas().scale((scale, scale));
                        draw_widgets(
                            surface.canvas(),
                            &tree,
                            root,
                            painted_view,
                            &mut fonts.borrow_mut(),
                            &mut SwashCache::new(),
                            Point::new(-1.0, -1.0),
                            None,
                            &HashMap::new(),
                            &states,
                            &mut HashMap::new(),
                            &mut TextBufferCache::new(128),
                            false,
                            &theme.resolve(),
                            scale,
                        );
                        surface
                    };
                    // Selection must not paint in the scrollbar lane, even at the
                    // end. Allow 1 logical px at the card edge for the Ghost variant's
                    // centered 1px border and antialiasing; the track is still 8px away.
                    for offset in [44.0, 2336.0] {
                        let mut selected = render(&view, offset);
                        let mut reference = render(&unselected_view, offset);
                        let selected = selected.peek_pixels().unwrap();
                        let reference = reference.peek_pixels().unwrap();
                        let gutter_left = inner.right - SCROLLBAR_W - 2.0 - spacing::SM;
                        let mut changed_card_pixels = 0;
                        let top = (inner.top - bottom_reveal(offset)).max(page.location.y);
                        let bottom = (inner.bottom - bottom_reveal(offset))
                            .min(page.location.y + page.size.height);
                        for y in (top * scale).ceil() as i32..(bottom * scale).floor() as i32 {
                            for x in (inner.left * scale).ceil() as i32
                                ..(inner.right * scale).floor() as i32
                            {
                                if selected.get_color((x, y)) != reference.get_color((x, y)) {
                                    assert!(
                                        (x as f32 + 0.5) / scale < gutter_left + 1.0,
                                        "card paint escaped its edge tolerance: {x},{y}, scale={scale}, edge={gutter_left}"
                                    );
                                    changed_card_pixels += 1;
                                }
                            }
                        }
                        assert!(
                            changed_card_pixels > 0,
                            "reference must expose a selected card"
                        );
                    }
                    let mut before = render(&view, 44.0);
                    let mut after = render(&view, 88.0);
                    let before = before.peek_pixels().unwrap();
                    let after = after.peek_pixels().unwrap();
                    let mut changed_inside = 0;
                    for y in 0..(height as f32 * scale) as i32 {
                        for x in 0..(width as f32 * scale) as i32 {
                            if before.get_color((x, y)) != after.get_color((x, y)) {
                                let logical =
                                    Point::new((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
                                assert!(
                                    inner.with_outset((1.0, 1.0)).contains(logical),
                                    "scroll paint escaped its viewport"
                                );
                                changed_inside += 1;
                            }
                        }
                    }
                    assert!(changed_inside > 0, "selected item must visibly scroll away");
                }
            }
        }
    }
}

#[cfg(test)]
mod page_tests {
    use super::ScrollDemo;

    #[test]
    fn page_reflows_and_retains_a_reachable_document() {
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new()));
        super::super::layout::test_support::assert_application::<ScrollDemo>(&fonts);
    }

    #[test]
    fn starts_dark_with_a_theme_selector() {
        super::super::theme_selector::test_support::assert_single_window_theme::<ScrollDemo>();
    }
}
