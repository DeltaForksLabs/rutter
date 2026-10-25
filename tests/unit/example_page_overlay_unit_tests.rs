//! Real demo overlays must paint above, and block pointer access to, the page.
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::{FontSystem, SwashCache};
use rutter::layout::{build_taffy_tree, compute_layout};
use rutter::render::hit_test::{HitResult, hit_test};
use rutter::render::{RichTextRenderer, draw_widgets, text::TextBufferCache};
use rutter::{AppLogic, Theme, Widget};
use skia_safe::{Color, Point, surfaces};
use taffy::prelude::*;
use winit::dpi::PhysicalSize;

#[test]
fn open_modal_paints_above_and_blocks_the_scroll_document() {
    use super::modal_toast_demo::{ModalToastDemo, ModalToastDemoState, Msg};
    let mut state = ModalToastDemoState {
        modal_open: true,
        ..Default::default()
    };
    assert_overlay(&ModalToastDemo::view(&mut state), |message| {
        matches!(message, Msg::CloseModal | Msg::ConfirmModal)
    });
}

#[test]
fn open_dialogs_paint_above_and_block_the_scroll_document() {
    use super::dialog_demo::{DialogDemo, DialogDemoState, Msg};
    for index in 0..3 {
        let mut state = DialogDemoState {
            confirm_open: index == 0,
            delete_open: index == 1,
            info_open: index == 2,
            ..Default::default()
        };
        assert_overlay(&DialogDemo::view(&mut state), |message| {
            matches!(
                message,
                Msg::CloseAll | Msg::ConfirmSave | Msg::ConfirmDelete
            )
        });
    }
}

fn assert_overlay<Message: Clone>(view: &Widget<'_, Message>, allowed: fn(&Message) -> bool) {
    let Widget::Column { children, .. } = view else {
        panic!("expected page column")
    };
    let document_index = children
        .iter()
        .position(|child| matches!(child, Widget::ScrollView { .. }))
        .unwrap();
    let overlay_index = children
        .iter()
        .position(|child| {
            matches!(
                child,
                Widget::Modal { visible: true, .. } | Widget::Dialog { visible: true, .. }
            )
        })
        .unwrap();
    assert!(overlay_index > document_index);
    let overlay = &children[overlay_index];
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    for (width, height) in [(800, 700), (320, 260)] {
        let mut tree = TaffyTree::new();
        let root = build_taffy_tree(&mut tree, view, fonts.clone(), &HashMap::new());
        compute_layout(
            &mut tree,
            root,
            PhysicalSize::new(width, height),
            fonts.clone(),
            &RichTextRenderer::default(),
        );
        let overlay_node = tree.children(root).unwrap()[overlay_index];
        let frame = tree.layout(overlay_node).unwrap();
        assert_eq!(frame.location, taffy::geometry::Point::ZERO);
        // Includes empty card space, controls, backdrop and theme action. None is
        // a legitimate inert hit, but no underlying page callback/scroll may leak.
        let mut overlay_actions = 0;
        for y in (4..height).step_by(8) {
            for x in (4..width).step_by(8) {
                match hit_test(
                    view,
                    &tree,
                    root,
                    Point::new(x as f32, y as f32),
                    Point::default(),
                    &HashMap::new(),
                ) {
                    Some(HitResult::Message { msg, .. }) => {
                        assert!(allowed(&msg), "underlying page callback leaked");
                        overlay_actions += 1;
                    }
                    Some(HitResult::ModalDismiss(_)) | None => {}
                    _ => panic!("underlying page interaction leaked"),
                }
            }
        }
        assert!(overlay_actions > 0, "overlay actions must remain clickable");
        for theme in [Theme::dark(), Theme::light()] {
            let mut full = surfaces::raster_n32_premul((width as i32, height as i32)).unwrap();
            let mut isolated = surfaces::raster_n32_premul((width as i32, height as i32)).unwrap();
            for (surface, widget, node, background) in [
                (&mut full, view, root, theme.surface),
                (&mut isolated, overlay, overlay_node, Color::TRANSPARENT),
            ] {
                surface.canvas().clear(background);
                draw_widgets(
                    surface.canvas(),
                    &tree,
                    node,
                    widget,
                    &mut fonts.borrow_mut(),
                    &mut SwashCache::new(),
                    Point::new(-100.0, -100.0),
                    None,
                    &HashMap::new(),
                    &HashMap::new(),
                    &mut HashMap::new(),
                    &mut TextBufferCache::default(),
                    false,
                    &theme,
                    1.0,
                );
            }
            let full = full.peek_pixels().unwrap();
            let isolated = isolated.peek_pixels().unwrap();
            // Opaque overlay pixels must be independent of the page underneath.
            // Transparent-backed rendering identifies them without duplicating
            // private card geometry; the translucent backdrop is excluded.
            let mut opaque_pixels = 0;
            for y in (0..height as i32).step_by(3) {
                for x in (0..width as i32).step_by(3) {
                    let expected = isolated.get_color((x, y));
                    if expected.a() == 255 {
                        assert_eq!(full.get_color((x, y)), expected);
                        opaque_pixels += 1;
                    }
                }
            }
            assert!(opaque_pixels > 50, "overlay card must actually paint");
        }
    }
}
