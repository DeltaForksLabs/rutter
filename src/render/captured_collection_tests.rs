//! Captured selection must preserve native collection geometry and painting.
use super::*;
use crate::accessibility::{AccessibilityInputs, build_accessibility_update};
use crate::{
    CarouselConfig, KeyedVirtualItems, VirtualItemKey, VirtualSelection, VirtualSelectionWith,
};

const SELECTED: &[usize] = &[1, 3, 3, 99];

fn row(index: usize) -> Option<String> {
    Some(format!("Long localized row {index} 東京 القاهرة"))
}
fn content<'a>(index: usize) -> Option<Widget<'a, ()>> {
    Some(Widget::Text {
        content: row(index).unwrap(),
        style: Style::default(),
        color: None,
        size: 14.0,
    })
}
fn interactive<'a>(_: usize) -> Option<Widget<'a, ()>> {
    Some(Widget::checkbox_with(
        true,
        |_| (),
        "Child 東京",
        Style::default(),
    ))
}
fn collection<'a>(
    captured: bool,
    family: usize,
    multiple: bool,
    keys: &'a [VirtualItemKey],
) -> Widget<'a, ()> {
    let style = Style {
        size: Size::from_lengths(280.0, 120.0),
        flex_shrink: 0.0,
        ..Style::default()
    };
    let legacy = if multiple {
        VirtualSelection::multiple(SELECTED, |_| ())
    } else {
        VirtualSelection::single(|_| ())
    };
    let policy = VirtualSelectionWith::from(legacy);
    let keyed = || KeyedVirtualItems::try_new(keys, &interactive).unwrap();
    let config = || CarouselConfig::uncontained(120.0).unwrap();
    match (captured, family, multiple) {
        (false, 0, false) => Widget::virtual_list(40.0, 4, &row, |_| (), style),
        (false, 1, false) => Widget::virtual_list_content(40.0, 4, &content, |_| (), style),
        (false, 2, false) => Widget::virtual_grid(2, 40.0, 4, &row, |_| (), style),
        (false, 3, false) => Widget::virtual_grid_content(2, 40.0, 4, &content, |_| (), style),
        (false, 0, true) => Widget::virtual_list_with_selection(40.0, 4, &row, legacy, style),
        (false, 1, true) => {
            Widget::virtual_list_content_with_selection(40.0, 4, &content, legacy, style)
        }
        (false, 2, true) => Widget::virtual_grid_with_selection(2, 40.0, 4, &row, legacy, style),
        (false, 3, true) => {
            Widget::virtual_grid_content_with_selection(2, 40.0, 4, &content, legacy, style)
        }
        (true, 0, false) => Widget::virtual_list_with(40.0, 4, &row, |_| (), style),
        (true, 1, false) => Widget::virtual_list_content_with(40.0, 4, &content, |_| (), style),
        (true, 2, false) => Widget::virtual_grid_with(2, 40.0, 4, &row, |_| (), style),
        (true, 3, false) => Widget::virtual_grid_content_with(2, 40.0, 4, &content, |_| (), style),
        (true, 0, true) => Widget::virtual_list_with_selection_with(40.0, 4, &row, policy, style),
        (true, 1, true) => {
            Widget::virtual_list_content_with_selection_with(40.0, 4, &content, policy, style)
        }
        (true, 2, true) => {
            Widget::virtual_grid_with_selection_with(2, 40.0, 4, &row, policy, style)
        }
        (true, 3, true) => {
            Widget::virtual_grid_content_with_selection_with(2, 40.0, 4, &content, policy, style)
        }
        (false, 4, _) => Widget::interactive_virtual_list_content(40.0, keyed(), |_| (), style),
        (true, 4, _) => Widget::interactive_virtual_list_content_with(40.0, keyed(), |_| (), style),
        (false, 5, _) => Widget::interactive_virtual_grid_content(2, 40.0, keyed(), |_| (), style),
        (true, 5, _) => {
            Widget::interactive_virtual_grid_content_with(2, 40.0, keyed(), |_| (), style)
        }
        (false, 6, _) => Widget::carousel_view(4, content, |_| (), config(), style),
        (true, 6, _) => Widget::carousel_view_with(4, content, |_| (), config(), style),
        (false, _, _) => Widget::interactive_carousel_view(keyed(), |_| (), config(), style),
        (true, _, _) => Widget::interactive_carousel_view_with(keyed(), |_| (), config(), style),
    }
}

#[test]
fn captured_collections_identity_accessibility_geometry_raster_parity() {
    let mut fonts = FontSystem::new();
    let mut swash = SwashCache::new();
    let mut font_cache = HashMap::new();
    let mut text_cache = TextBufferCache::default();
    let renderer = RichTextRenderer::new();
    let keys: Vec<_> = (1..=4).map(|v| VirtualItemKey::new(v).unwrap()).collect();
    for family in 0..8 {
        for multiple in [false, true] {
            if multiple && family >= 4 {
                continue;
            }
            let widgets = [
                collection(false, family, multiple, &keys),
                collection(true, family, multiple, &keys),
            ];
            assert_eq!(
                widgets[0].resolved_id(&[2, 1]),
                widgets[1].resolved_id(&[2, 1])
            );
            assert_eq!(
                widgets[0].keyboard_focus_id(&[2, 1]),
                widgets[1].keyboard_focus_id(&[2, 1])
            );
            assert_eq!(
                validate_widget_id_snapshot(&widgets[0]).unwrap(),
                validate_widget_id_snapshot(&widgets[1]).unwrap()
            );
            for disabled in [false, true] {
                let widgets = [
                    collection(false, family, multiple, &keys)
                        .with_id(42)
                        .enabled(!disabled),
                    collection(true, family, multiple, &keys)
                        .with_id(42)
                        .enabled(!disabled),
                ];
                for (width, direction, theme) in [
                    (320, LayoutDirection::Ltr, Theme::light()),
                    (320, LayoutDirection::Rtl, Theme::dark()),
                    (800, LayoutDirection::Rtl, Theme::light()),
                    (800, LayoutDirection::Ltr, Theme::dark()),
                ] {
                    let mut expected_geometry = None;
                    let mut expected_accessibility = None;
                    for scale in [1.0_f32, 1.5, 2.0] {
                        let mut pixels = Vec::new();
                        for widget in &widgets {
                            let mut tree = TaffyTree::new();
                            let root = build_layout_tree(&mut tree, widget, direction);
                            compute_layout_with_renderer(
                                &mut tree,
                                root,
                                PhysicalSize::new(width, 96),
                                &renderer,
                            );
                            let actual = geometry(&tree, root);
                            assert!(actual.iter().all(|(_, _, w, h)| *w > 0.0 && *h > 0.0));
                            assert_eq!(actual[0].2, 280.0);
                            assert_eq!(actual[0].3, 120.0);
                            if let Some(expected) = &expected_geometry {
                                assert_eq!(&actual, expected);
                            } else {
                                expected_geometry = Some(actual);
                            }
                            let states = HashMap::new();
                            let inputs = HashMap::new();
                            let update = build_accessibility_update(
                                &tree,
                                widget,
                                root,
                                AccessibilityInputs {
                                    input_states: &inputs,
                                    widget_states: &states,
                                    focused_widget_id: Some(42),
                                    viewport: (width as f32, 96.0),
                                    direction,
                                },
                            );
                            let props: Vec<_> = update
                                .nodes
                                .iter()
                                .map(|(id, node)| {
                                    (
                                        *id,
                                        node.role(),
                                        node.label().map(str::to_owned),
                                        node.size_of_set(),
                                        node.row_count(),
                                        node.column_count(),
                                        node.is_multiselectable(),
                                        node.is_disabled(),
                                    )
                                })
                                .collect();
                            if let Some(expected) = &expected_accessibility {
                                assert_eq!(&props, expected);
                            } else {
                                expected_accessibility = Some(props);
                            }
                            if multiple {
                                assert!(
                                    update
                                        .nodes
                                        .iter()
                                        .any(|(_, node)| node.is_multiselectable())
                                );
                            }
                            let mut surface = surfaces::raster_n32_premul((
                                (width as f32 * scale) as i32,
                                (96.0 * scale) as i32,
                            ))
                            .unwrap();
                            surface.canvas().clear(Color::TRANSPARENT);
                            surface.canvas().scale((scale, scale));
                            draw_widgets(
                                surface.canvas(),
                                &tree,
                                root,
                                widget,
                                &mut fonts,
                                &mut swash,
                                Point::new(-1.0, -1.0),
                                Some(42),
                                &inputs,
                                &states,
                                &mut font_cache,
                                &mut text_cache,
                                true,
                                &theme,
                                scale,
                            );
                            pixels.push(surface.peek_pixels().unwrap().bytes().unwrap().to_vec());
                        }
                        assert!(pixels[0].iter().any(|b| *b != 0));
                        assert!(
                            pixels[0] == pixels[1],
                            "family={family} multiple={multiple} width={width} scale={scale} disabled={disabled}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn captured_collections_legacy_policy_stays_copy_const_and_captured_clone_has_no_message_bounds() {
    const SINGLE: VirtualSelection<'static, ()> = VirtualSelection::single(|_| ());
    const MULTIPLE: VirtualSelection<'static, ()> = VirtualSelection::multiple(&[1, 2], |_| ());
    fn copy<T: Copy>(value: T) -> (T, T) {
        (value, value)
    }
    let _ = copy(SINGLE);
    let _ = copy(MULTIPLE);
    struct NonCloneMessage(usize);
    let owned = Box::new(7usize);
    let selection = VirtualSelectionWith::single(move |v| NonCloneMessage(*owned + v));
    let clone = selection.clone();
    let VirtualSelectionWith::Single(callback) = clone else {
        unreachable!()
    };
    assert_eq!(callback.call(2).0, 9);
}
