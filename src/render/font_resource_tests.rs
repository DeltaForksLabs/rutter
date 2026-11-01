//! Font-free virtual layout preserves public layout and renderer behavior.
use std::{cell::RefCell, rc::Rc};

use skia_safe::{Color, surfaces};
use taffy::prelude::{Dimension, Size, Style};
use winit::dpi::PhysicalSize;

use super::*;
use crate::{RichText, RichTextSpan, layout, spacing};

fn fixed_style(width: f32, height: f32) -> Style {
    Style {
        size: Size {
            width: Dimension::length(width),
            height: Dimension::length(height),
        },
        flex_shrink: 0.0,
        ..Style::default()
    }
}

fn virtual_item(_: usize) -> Option<Widget<'static, ()>> {
    Some(Widget::Column {
        children: vec![
            Widget::Text {
                content: "東京 القاهرة fallback".into(),
                size: 14.0,
                color: None,
                style: Style::default(),
            },
            Widget::rich_text(
                RichText::from_spans([
                    RichTextSpan::new("Wrapped ").bold(),
                    RichTextSpan::new("virtual item text"),
                ]),
                Style::default(),
            ),
        ],
        style: Style::default(),
    })
}

fn specimen(width: f32, grid: bool) -> Widget<'static, ()> {
    let collection = if grid {
        Widget::VirtualGridContent {
            id: 100,
            columns: 2,
            item_height: 64.0,
            item_count: 4,
            items: &virtual_item,
            on_select: |_| (),
            style: fixed_style(width, 120.0),
        }
    } else {
        Widget::VirtualListContent {
            id: 100,
            item_height: 64.0,
            item_count: 4,
            items: &virtual_item,
            on_select: |_| (),
            style: fixed_style(width, 120.0),
        }
    };
    Widget::Column {
        children: vec![
            virtual_item(0).unwrap(),
            Widget::TextArea {
                id: 42,
                on_change: |_| (),
                on_submit: None,
                style: fixed_style(width, 48.0),
                label: "",
                state: InputState::Idle,
                placeholder: "",
                error_msg: None,
            },
            collection,
        ],
        style: Style {
            size: Size {
                width: Dimension::length(width),
                height: Dimension::auto(),
            },
            gap: Size::length(spacing::SM),
            ..Style::default()
        },
    }
}

fn assert_layout_matches(
    first: (&TaffyTree<RutterContext>, NodeId),
    second: (&TaffyTree<RutterContext>, NodeId),
) {
    assert_eq!(first.0.layout(first.1), second.0.layout(second.1));
    assert_eq!(
        first.0.style(first.1).unwrap().direction,
        second.0.style(second.1).unwrap().direction
    );
    let first_children = first.0.children(first.1).unwrap();
    let second_children = second.0.children(second.1).unwrap();
    assert_eq!(first_children.len(), second_children.len());
    for (&left, &right) in first_children.iter().zip(&second_children) {
        assert_layout_matches((first.0, left), (second.0, right));
    }
}

#[test]
fn public_layout_wrappers_match_font_free_layout_with_borrowed_fonts() {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let _borrowed = fonts.borrow_mut();
    let renderer = RichTextRenderer::new();
    let states = HashMap::new();
    for width in [320, 800] {
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            let widget = specimen(width as f32, true);
            let mut public_tree = TaffyTree::new();
            let public_root = layout::build_taffy_tree_with_direction(
                &mut public_tree,
                &widget,
                fonts.clone(),
                &states,
                direction,
            );
            layout::compute_layout(
                &mut public_tree,
                public_root,
                PhysicalSize::new(width, 300),
                fonts.clone(),
                &renderer,
            );
            let mut render_tree = TaffyTree::new();
            let render_root = build_layout_tree(&mut render_tree, &widget, direction);
            compute_layout_with_renderer(
                &mut render_tree,
                render_root,
                PhysicalSize::new(width, 300),
                &renderer,
            );
            assert_layout_matches((&public_tree, public_root), (&render_tree, render_root));
        }
    }
}

struct RenderFixture {
    fonts: Rc<RefCell<FontSystem>>,
    swash: SwashCache,
    font_cache: HashMap<(String, u32), Font>,
    text_cache: TextBufferCache,
    inputs: HashMap<u64, InputWidgetState>,
    measurement: RichTextRenderer,
}

struct RasterCase {
    size: (u32, u32),
    direction: LayoutDirection,
    theme: Theme,
    scale: f32,
}

impl RenderFixture {
    fn new() -> Self {
        let mut fonts = FontSystem::new();
        let mut input = InputWidgetState::new(&mut fonts);
        input.set_text(&mut fonts, "visible input 東京 القاهرة");
        Self {
            fonts: Rc::new(RefCell::new(fonts)),
            swash: SwashCache::new(),
            font_cache: HashMap::new(),
            text_cache: TextBufferCache::default(),
            inputs: HashMap::from([(42, input)]),
            measurement: RichTextRenderer::new(),
        }
    }

    fn pixels(
        &mut self,
        widget: &Widget<'_, ()>,
        case: &RasterCase,
        cache: Option<&mut ImageRenderCache>,
    ) -> Vec<u8> {
        let RasterCase {
            size,
            direction,
            theme,
            scale,
        } = case;
        let mut taffy = TaffyTree::new();
        let root = build_layout_tree(&mut taffy, widget, *direction);
        compute_layout_with_renderer(
            &mut taffy,
            root,
            PhysicalSize::new(size.0, size.1),
            &self.measurement,
        );
        let mut surface = surfaces::raster_n32_premul((
            (size.0 as f32 * scale) as i32,
            (size.1 as f32 * scale) as i32,
        ))
        .unwrap();
        surface.canvas().clear(Color::TRANSPARENT);
        surface.canvas().scale((*scale, *scale));
        // Match the engine: retain the RefMut across the entire draw, including
        // nested virtual-item measurement. A second borrow must not occur.
        let mut fonts = self.fonts.borrow_mut();
        let states = HashMap::new();
        if let Some(cache) = cache {
            draw_widgets_with_cache(
                surface.canvas(),
                &taffy,
                root,
                widget,
                &mut fonts,
                &mut self.swash,
                Point::new(-1.0, -1.0),
                None,
                &self.inputs,
                &states,
                &mut self.font_cache,
                &mut self.text_cache,
                cache,
                false,
                theme,
                *scale,
            );
        } else {
            draw_widgets(
                surface.canvas(),
                &taffy,
                root,
                widget,
                &mut fonts,
                &mut self.swash,
                Point::new(-1.0, -1.0),
                None,
                &self.inputs,
                &states,
                &mut self.font_cache,
                &mut self.text_cache,
                false,
                theme,
                *scale,
            );
        }
        surface.peek_pixels().unwrap().bytes().unwrap().to_vec()
    }
}

#[test]
fn cached_and_uncached_virtual_text_keep_pixels_across_frames_and_surface_teardown() {
    let mut fixture = RenderFixture::new();
    let mut first_cache = ImageRenderCache::default();
    // Representative combinations rather than a costly Cartesian product:
    // widths, short/full viewport, both directions/themes and each scale.
    for (size, direction, theme) in [
        ((320, 120), LayoutDirection::Rtl, Theme::dark()),
        ((320, 300), LayoutDirection::Ltr, Theme::light()),
        ((800, 120), LayoutDirection::Ltr, Theme::dark()),
        ((800, 300), LayoutDirection::Rtl, Theme::light()),
    ] {
        for scale in [1.0, 1.5, 2.0] {
            let case = RasterCase {
                size,
                direction,
                theme: theme.clone(),
                scale,
            };
            for grid in [false, true] {
                let widget = specimen(size.0 as f32, grid);
                assert_cache_parity(&mut fixture, &mut first_cache, &widget, &case);
            }
        }
    }
}

fn assert_cache_parity(
    fixture: &mut RenderFixture,
    first_cache: &mut ImageRenderCache,
    widget: &Widget<'_, ()>,
    case: &RasterCase,
) {
    let expected = fixture.pixels(widget, case, None);
    assert!(expected.iter().any(|&byte| byte != 0));
    let mut second_cache = ImageRenderCache::default();
    for _ in 0..2 {
        assert_eq!(fixture.pixels(widget, case, Some(first_cache)), expected);
    }
    first_cache.clear();
    second_cache.exposed_combo_box = Some(999);
    assert_eq!(
        fixture.pixels(widget, case, Some(&mut second_cache)),
        expected
    );
    assert_eq!(second_cache.exposed_combo_box, None);
    drop(second_cache);
    assert_eq!(fixture.pixels(widget, case, Some(first_cache)), expected);
}
