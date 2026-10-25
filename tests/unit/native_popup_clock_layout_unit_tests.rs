use std::{cell::RefCell, collections::HashMap, rc::Rc};

use super::*;
use rutter::engine::widget_state::{ScrollState, WidgetState};
use rutter::layout::{build_taffy_tree, compute_layout};
use rutter::render::{
    RichTextRenderer,
    hit_test::{HitResult, hit_test},
};
use skia_safe::Point;
use winit::dpi::PhysicalSize;

#[test]
fn native_clock_surfaces_keep_real_controls_and_scroll_to_close_on_short_popups() {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut state = ClockCalendar::new(&mut fonts.borrow_mut());
    let panel = ClockCalendar::view(&mut state, SurfaceId::PRIMARY);
    assert!(matches!(
        panel,
        Widget::ButtonContent {
            on_press: Message::Toggle,
            ..
        }
    ));
    let mut tree = TaffyTree::new();
    let root = build_taffy_tree(&mut tree, &panel, fonts.clone(), &HashMap::new());
    // The 32px panel anchor is a platform surface constraint, not page spacing.
    compute_layout(
        &mut tree,
        root,
        PhysicalSize::new(32, 32),
        fonts.clone(),
        &RichTextRenderer::default(),
    );
    assert_eq!(
        tree.layout(root).unwrap().size,
        Size {
            width: 32.0,
            height: 32.0,
        }
    );
    assert!(matches!(
        hit_test(
            &panel,
            &tree,
            root,
            Point::new(16.0, 16.0),
            Point::default(),
            &HashMap::new()
        ),
        Some(HitResult::Message {
            msg: Message::Toggle,
            ..
        })
    ));
    drop(panel);

    for (width, height) in [(384, 448), (800, 700), (320, 448), (320, 220), (800, 220)] {
        let popup = ClockCalendar::view(&mut state, CLOCK_POPUP);
        let Widget::ScrollView { id, .. } = &popup else {
            panic!("popup document must scroll")
        };
        let mut tree = TaffyTree::new();
        let root = build_taffy_tree(&mut tree, &popup, fonts.clone(), &HashMap::new());
        compute_layout(
            &mut tree,
            root,
            PhysicalSize::new(width, height),
            fonts.clone(),
            &RichTextRenderer::default(),
        );
        let viewport = tree.layout(root).unwrap();
        let document_node = tree.children(root).unwrap()[0];
        let document = tree.layout(document_node).unwrap();
        let children = tree.children(document_node).unwrap();
        let clock = tree.layout(children[0]).unwrap();
        let calendar = tree.layout(children[1]).unwrap();
        let close = tree.layout(children[2]).unwrap();
        assert_eq!(clock.size.height, 38.0);
        assert_eq!(calendar.size.height, 330.0);
        assert_eq!(close.size.height, 36.0);
        assert!(calendar.location.y - clock.location.y - clock.size.height >= spacing::SM - 1.0);
        assert!(close.location.y - calendar.location.y - calendar.size.height >= spacing::SM - 1.0);
        assert!(document.size.width <= viewport.size.width + 1.0);
        let mut scroll = ScrollState {
            offset_y: 0.0,
            content_height: document.size.height,
            viewport_h: viewport.size.height,
        };
        scroll.scroll_by(f32::MAX);
        if height < 448 {
            assert!(scroll.offset_y > 0.0);
        }
        let point = Point::new(
            document.location.x + close.location.x + close.size.width * 0.5,
            document.location.y + close.location.y + close.size.height * 0.5 - scroll.offset_y,
        );
        assert!(point.y > 0.0 && point.y < height as f32);
        let states = HashMap::from([(*id, WidgetState::Scroll(scroll))]);
        assert!(matches!(
            hit_test(&popup, &tree, root, point, Point::default(), &states),
            Some(HitResult::Message {
                msg: Message::Close,
                ..
            })
        ));
    }
}
