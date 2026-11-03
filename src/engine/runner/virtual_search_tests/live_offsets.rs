//! Live collection offsets must move portals without rebuilding the page layout.
use super::*;

#[test]
fn virtual_search_live_fractional_offsets_preserve_bounds_hits_and_current_selection() {
    for family in FAMILIES {
        for combo in [false, true] {
            for captured in [false, true] {
                for input in ["pointer", "keyboard", "accessibility"] {
                    let mut r = runner(family, combo, captured);
                    let id = field_id(&r);
                    let before = popup_row(&mut r);
                    let revision = r.engine.layout_revision;
                    match r.engine.widget_states.get_mut(&COLLECTION).unwrap() {
                        WidgetState::VList(state) => state.scroll_y = 12.5,
                        WidgetState::VGrid(state) => state.scroll_y = 12.5,
                        WidgetState::Carousel(state) => state.position.progress = 12.5 / 240.0,
                        _ => panic!("expected an interactive collection"),
                    }
                    let moved = popup_row(&mut r); // Uses the existing page Taffy tree.
                    let (dx, dy) = match family {
                        Family::Carousel => (-12.5, 0.0),
                        _ => (0.0, -12.5),
                    };
                    for (actual, expected) in [
                        (moved.left, before.left + dx),
                        (moved.right, before.right + dx),
                        (moved.top, before.top + dy),
                        (moved.bottom, before.bottom + dy),
                    ] {
                        assert!((actual - expected).abs() <= 1.0, "{family:?}");
                    }
                    let options = published_options(&mut r);
                    assert_eq!(options.len(), 2);
                    let bounds = options[0].1.bounds().unwrap();
                    for (actual, expected) in [
                        (bounds.x0, moved.left),
                        (bounds.x1, moved.right),
                        (bounds.y0, moved.top),
                        (bounds.y1, moved.bottom),
                    ] {
                        assert!((actual - expected as f64).abs() <= 1.0);
                    }
                    assert_eq!(r.engine.layout_revision, revision);
                    assert!(!r.engine.layout_dirty);

                    // Commit the second result, not its filtered row number (1).
                    assert_eq!(options[1].0, resolve_search_suggestion_id(id, 2));
                    let second = options[1].1.bounds().unwrap();
                    r.engine.app_state.fresh = true;
                    r.engine.layout_dirty = true;
                    match input {
                        "pointer" => pointer(
                            &mut r,
                            Point::new(
                                ((second.x0 + second.x1) * 0.5) as f32,
                                ((second.y0 + second.y1) * 0.5) as f32,
                            ),
                        ),
                        "keyboard" => {
                            key(&mut r, NamedKey::ArrowDown);
                            key(&mut r, NamedKey::ArrowDown);
                            key(&mut r, NamedKey::Enter);
                        }
                        _ => action(&mut r, options[1].0, Action::Click),
                    }
                    assert_eq!(
                        r.engine.app_state.messages,
                        vec![Message::Pick(if captured { 7 } else { 0 }, 2)],
                        "{family:?}, combo={combo}, captured={captured}, {input}"
                    );
                }
            }
        }
    }
}
