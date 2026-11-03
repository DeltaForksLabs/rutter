//! Actual window painting, including the part of a popup outside its item card.
use super::*;
use crate::i18n::LayoutDirection;
use crate::layout::{build_layout_tree, compute_layout_with_renderer};
use crate::render::text::TextBufferCache;
use crate::render::{RichTextRenderer, draw_widgets};
use cosmic_text::SwashCache;
use skia_safe::{Color, Rect, surfaces};
use std::collections::HashMap;
use taffy::prelude::TaffyTree;

struct RasterCase {
    viewport: PhysicalSize<u32>,
    direction: LayoutDirection,
    theme: crate::Theme,
    scale: f32,
}

fn paint(r: &mut RutterRunner<VirtualSearchApp>, case: &RasterCase, open: bool) -> (Vec<u8>, Rect) {
    let id = field_id(r);
    r.engine
        .widget_states
        .get_mut(&id)
        .unwrap()
        .as_search_mut()
        .unwrap()
        .dismissed = !open;
    let view = VirtualSearchApp::view(&mut r.engine.app_state);
    let mut tree = TaffyTree::new();
    let root = build_layout_tree(&mut tree, &view, case.direction);
    compute_layout_with_renderer(&mut tree, root, case.viewport, &RichTextRenderer::new());
    let states = &r.engine.widget_states;
    let viewport = (case.viewport.width as f32, case.viewport.height as f32);
    let overlays = collect_open_search_overlays(
        &view,
        &tree,
        root,
        states,
        &r.engine.input_states,
        Some(id),
        viewport,
    );
    let row = overlays
        .first()
        .map(|overlay| search_option_rect(search_popup_layout(overlay, viewport), 1))
        .unwrap_or_default();
    let mut surface = surfaces::raster_n32_premul((
        (viewport.0 * case.scale) as i32,
        (viewport.1 * case.scale) as i32,
    ))
    .unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    surface.canvas().scale((case.scale, case.scale));
    draw_widgets(
        surface.canvas(),
        &tree,
        root,
        &view,
        &mut FontSystem::new(),
        &mut SwashCache::new(),
        Point::new(-1.0, -1.0),
        Some(id),
        &r.engine.input_states,
        states,
        &mut HashMap::new(),
        &mut TextBufferCache::default(),
        true,
        &case.theme,
        case.scale,
    );
    (
        surface.peek_pixels().unwrap().bytes().unwrap().to_vec(),
        row,
    )
}

fn changed_inside_row(open: &[u8], closed: &[u8], row: Rect, case: &RasterCase) -> bool {
    let width = (case.viewport.width as f32 * case.scale) as usize;
    let start_x = ((row.left + 1.0) * case.scale).ceil() as usize;
    let end_x = ((row.right - 1.0) * case.scale).floor() as usize;
    let start_y = ((row.top + 1.0) * case.scale).ceil() as usize;
    let end_y = ((row.bottom - 1.0) * case.scale).floor() as usize;
    (start_y..end_y).any(|y| {
        (start_x..end_x).any(|x| {
            let offset = (y * width + x) * 4;
            open[offset..offset + 4] != closed[offset..offset + 4]
        })
    })
}

#[test]
fn virtual_search_popup_window_raster_matches_legacy_at_responsive_sizes_and_scales() {
    for family in FAMILIES {
        for combo in [false, true] {
            let mut r = runner(family, combo, false);
            for viewport in [PhysicalSize::new(320, 180), PhysicalSize::new(800, 360)] {
                for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                    for theme in [crate::Theme::light(), crate::Theme::dark()] {
                        for scale in [1.0, 1.5, 2.0] {
                            let case = RasterCase {
                                viewport,
                                direction,
                                theme: theme.clone(),
                                scale,
                            };
                            r.engine.app_state.captured = false;
                            let (legacy, row) = paint(&mut r, &case, true);
                            assert!(row.width() > 0.0 && row.height() > 0.0);
                            let (closed, _) = paint(&mut r, &case, false);
                            assert!(
                                changed_inside_row(&legacy, &closed, row, &case),
                                "popup rows must actually paint: {family:?}, combo={combo}"
                            );
                            if viewport.height == 360 {
                                // The collection ends at the page inset plus its
                                // 140px viewport. The portal must paint below it.
                                let outside =
                                    Rect::new(row.left, row.top.max(164.0), row.right, row.bottom);
                                assert!(outside.height() > 2.0);
                                assert!(
                                    changed_inside_row(&legacy, &closed, outside, &case),
                                    "popup must escape the collection clip: {family:?}, combo={combo}"
                                );
                            }
                            r.engine.app_state.captured = true;
                            let (captured, captured_row) = paint(&mut r, &case, true);
                            assert_eq!(row, captured_row);
                            assert!(
                                legacy == captured,
                                "{family:?}, combo={combo}, {viewport:?}, {direction:?}, scale={scale}"
                            );
                        }
                    }
                }
            }
        }
    }
}
