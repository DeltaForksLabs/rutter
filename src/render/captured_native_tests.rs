//! Callback ownership must not change native allocations, identity or pixels.
use super::*;
use crate::layout::{build_layout_tree, compute_layout_with_renderer};
use crate::render::hit_test::{
    find_input_callbacks, find_input_props_with, find_slider_callback, find_slider_callback_with,
};
use crate::widget::id::validate_widget_id_snapshot;
use skia_safe::{Color, surfaces};
use taffy::prelude::{LengthPercentage, Size, Style};
use winit::dpi::PhysicalSize;

#[path = "captured_collection_tests.rs"]
mod collections;

#[path = "captured_table_tests.rs"]
mod tables;

fn field<'a>(captured: bool, kind: usize) -> Widget<'a, ()> {
    let style = Style {
        size: Size::from_lengths(260.0, 44.0),
        flex_shrink: 0.0,
        ..Style::default()
    };
    let label = "Long localized label 東京 القاهرة";
    let error = Some("Validation message must retain its annotation row".into());
    match (captured, kind) {
        (false, 0 | 1) => Widget::text_input(
            |_| (),
            None,
            style,
            label,
            "Placeholder",
            InputState::Error,
            error,
            kind == 1,
        ),
        (true, 0 | 1) => Widget::text_input_with(
            |_| (),
            None,
            style,
            label,
            "Placeholder",
            InputState::Error,
            error,
            kind == 1,
        ),
        (false, 2) => Widget::text_area(
            |_| (),
            None,
            Style {
                size: Size::from_lengths(260.0, 120.0),
                ..style
            },
            label,
            InputState::Error,
            "Multiline",
            error,
        ),
        (true, 2) => Widget::text_area_with(
            |_| (),
            None,
            Style {
                size: Size::from_lengths(260.0, 120.0),
                ..style
            },
            label,
            InputState::Error,
            "Multiline",
            error,
        ),
        (false, 3) => Widget::slider(42.0, 0.0, 100.0, 1.0, |_| (), style, "Amount"),
        (true, 3) => Widget::slider_with(42.0, 0.0, 100.0, 1.0, |_| (), style, "Amount"),
        (false, 4) => Widget::Checkbox {
            checked: true,
            on_change: |_| (),
            label,
            style,
        },
        (true, 4) => Widget::checkbox_with(true, |_| (), label, style),
        (false, 5) => Widget::Switch {
            checked: true,
            on_change: |_| (),
            style,
        },
        (true, 5) => Widget::switch_with(true, |_| (), style),
        (false, 6) => Widget::Radio {
            selected: true,
            on_select: || (),
            label,
            style,
        },
        (true, 6) => Widget::radio_with(true, || (), label, style),
        (false, _) => Widget::tab_bar(&["Long localized tab 東京", "القاهرة"], 1, |_| (), style),
        (true, _) => {
            Widget::tab_bar_with(&["Long localized tab 東京", "القاهرة"], 1, |_| (), style)
        }
    }
}

fn geometry(tree: &TaffyTree<RutterContext>, root: NodeId) -> Vec<(f32, f32, f32, f32)> {
    let layout = tree.layout(root).unwrap();
    let mut result = vec![(
        layout.location.x,
        layout.location.y,
        layout.size.width,
        layout.size.height,
    )];
    for child in tree.children(root).unwrap() {
        result.extend(geometry(tree, child));
    }
    result
}

#[test]
fn captured_native_fields_share_native_identity_layout_and_raster_across_viewports() {
    let mut fonts = FontSystem::new();
    let mut swash = SwashCache::new();
    let mut font_cache = HashMap::new();
    let mut text_cache = TextBufferCache::default();
    let measurement = RichTextRenderer::new();
    for kind in 0..8 {
        let legacy = field(false, kind);
        let captured = field(true, kind);
        assert_eq!(legacy.resolved_id(&[2, 1]), captured.resolved_id(&[2, 1]));
        assert_eq!(
            legacy.keyboard_focus_id(&[2, 1]),
            captured.keyboard_focus_id(&[2, 1])
        );
        assert_eq!(
            validate_widget_id_snapshot(&legacy).unwrap(),
            validate_widget_id_snapshot(&captured).unwrap()
        );
        if kind == 7 {
            for index in 0..2 {
                assert_eq!(
                    legacy.tab_focus_id(&[2, 1], index),
                    captured.tab_focus_id(&[2, 1], index)
                );
            }
            assert_eq!(
                legacy.with_id(42).tab_focus_id(&[], 1),
                captured.with_id(42).tab_focus_id(&[], 1)
            );
        }
        let focused_id = if kind == 7 {
            field(true, kind).with_id(42).tab_focus_id(&[], 1)
        } else {
            field(true, kind).with_id(42).keyboard_focus_id(&[])
        };
        for disabled in [false, true] {
            let widgets = [
                field(false, kind).with_id(42).enabled(!disabled),
                field(true, kind).with_id(42).enabled(!disabled),
            ];
            for (width, height, direction, theme) in [
                (320, 96, LayoutDirection::Ltr, Theme::light()),
                (320, 240, LayoutDirection::Rtl, Theme::dark()),
                (800, 96, LayoutDirection::Rtl, Theme::light()),
                (800, 240, LayoutDirection::Ltr, Theme::dark()),
            ] {
                let mut expected_geometry = None;
                let mut inputs = HashMap::new();
                let mut input = InputWidgetState::new(&mut fonts);
                input.set_text(&mut fonts, "Long text 東京 القاهرة\nsecond line");
                input.set_sensitive(kind == 1);
                inputs.insert(42, input);
                for scale in [1.0_f32, 1.5, 2.0] {
                    let mut pixels = Vec::new();
                    for widget in &widgets {
                        let mut tree = TaffyTree::new();
                        let root = build_layout_tree(&mut tree, widget, direction);
                        compute_layout_with_renderer(
                            &mut tree,
                            root,
                            PhysicalSize::new(width, height),
                            &measurement,
                        );
                        let actual = geometry(&tree, root);
                        assert!(
                            actual
                                .iter()
                                .all(|(_, _, width, height)| *width > 0.0 && *height > 0.0)
                        );
                        if let Some(expected) = &expected_geometry {
                            assert_eq!(&actual, expected);
                        } else {
                            expected_geometry = Some(actual);
                        }
                        if !disabled && kind < 3 {
                            let layout = tree.layout(root).unwrap();
                            let body = crate::layout::field::FieldInsets::for_widget(widget)
                                .body((layout.size.width, layout.size.height));
                            assert_eq!(body.height(), if kind == 2 { 120.0 } else { 44.0 });
                            assert!(body.width() > 0.0);
                        }
                        let mut surface = surfaces::raster_n32_premul((
                            (width as f32 * scale) as i32,
                            (height as f32 * scale) as i32,
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
                            focused_id,
                            &inputs,
                            &HashMap::new(),
                            &mut font_cache,
                            &mut text_cache,
                            true,
                            &theme,
                            scale,
                        );
                        pixels.push(surface.peek_pixels().unwrap().bytes().unwrap().to_vec());
                    }
                    assert!(pixels[0].iter().any(|byte| *byte != 0));
                    assert!(
                        pixels[0] == pixels[1],
                        "kind={kind}, width={width}, height={height}, scale={scale}, disabled={disabled}"
                    );
                }
            }
        }
    }
}

#[test]
fn captured_native_textarea_auto_height_keeps_the_native_120px_body() {
    for captured in [false, true] {
        let widget = if captured {
            Widget::text_area_with(
                |_| (),
                None,
                Style::default(),
                "Label",
                InputState::Error,
                "",
                Some("Error".into()),
            )
        } else {
            Widget::text_area(
                |_| (),
                None,
                Style::default(),
                "Label",
                InputState::Error,
                "",
                Some("Error".into()),
            )
        };
        let mut tree = TaffyTree::new();
        let root = build_layout_tree(&mut tree, &widget, LayoutDirection::Ltr);
        compute_layout_with_renderer(
            &mut tree,
            root,
            PhysicalSize::new(320, 240),
            &RichTextRenderer::new(),
        );
        let layout = tree.layout(root).unwrap();
        let body = crate::layout::field::FieldInsets::for_widget(&widget)
            .body((layout.size.width, layout.size.height));
        assert_eq!(body.height(), 120.0);
    }
}

#[test]
fn captured_native_slider_validation_matches_legacy_for_finite_ranges_and_invalid_values() {
    for (value, min, max, step) in [
        (0.5, 0.0, 1.0, 0.1),
        (2.0, 0.0, 1.0, 0.1),
        (0.0, 0.0, 0.0, 1.0),
        (f32::NAN, 0.0, 1.0, 1.0),
        (0.0, 1.0, 0.0, 1.0),
        (0.0, 0.0, f32::INFINITY, 1.0),
        (0.0, 0.0, 1.0, 0.0),
    ] {
        let legacy: Result<Widget<'_, ()>, _> =
            Widget::try_slider(value, min, max, step, |_| (), Style::default(), "");
        let captured: Result<Widget<'_, ()>, _> =
            Widget::try_slider_with(value, min, max, step, |_| (), Style::default(), "");
        assert_eq!(legacy.err(), captured.err());
    }
}

#[test]
fn captured_native_lookups_follow_auto_paths_wrappers_disabled_and_open_popovers() {
    for open in [false, true] {
        let anchor = field(true, 1);
        let input_id = anchor.resolved_id(&[0, 0, 0]).unwrap();
        let area = field(true, 2);
        let area_id = area.resolved_id(&[0, 0, 1, 0]).unwrap();
        let slider = field(true, 3);
        let slider_id = slider.resolved_id(&[0, 0, 1, 1]).unwrap();
        let widget = Widget::button_content(
            "Wrapper",
            Widget::pointer_region(
                crate::WidgetId::manual(99).unwrap(),
                Widget::popover(
                    open,
                    anchor,
                    Widget::Column {
                        children: vec![area, slider],
                        style: Style::default(),
                    },
                    None,
                    Style::default(),
                    Style::default(),
                ),
                crate::PointerRegionConfig::new(|_| ()),
                Style::default(),
            ),
            (),
            Style::default(),
            None,
            ButtonVariant::Primary,
        );
        let (callback, _, password, multiline) = find_input_props_with(&widget, input_id).unwrap();
        callback.call("owned".into());
        assert!(password && !multiline);
        assert!(find_input_callbacks(&widget, input_id).is_none());
        assert_eq!(find_input_props_with(&widget, area_id).is_some(), open);
        assert_eq!(
            find_slider_callback_with(&widget, slider_id).is_some(),
            open
        );
        assert!(find_slider_callback(&widget, slider_id).is_none());
        let disabled = widget.enabled(false);
        assert!(find_input_props_with(&disabled, input_id).is_none());
        assert!(find_slider_callback_with(&disabled, slider_id).is_none());
    }
}

#[test]
fn captured_choices_preserve_accessibility_roles_state_labels_bounds_and_disabled_actions() {
    use crate::accessibility::{AccessibilityInputs, build_accessibility_update};
    use accesskit::{Action, Role, Toggled};

    for kind in 4..8 {
        for disabled in [false, true] {
            let mut expected_summary = None;
            for captured in [false, true] {
                let widget = field(captured, kind).enabled(!disabled);
                let mut tree = TaffyTree::new();
                let root = build_layout_tree(&mut tree, &widget, LayoutDirection::Rtl);
                compute_layout_with_renderer(
                    &mut tree,
                    root,
                    PhysicalSize::new(320, 96),
                    &RichTextRenderer::new(),
                );
                let update = build_accessibility_update(
                    &tree,
                    &widget,
                    root,
                    AccessibilityInputs {
                        input_states: &HashMap::new(),
                        widget_states: &HashMap::new(),
                        focused_widget_id: None,
                        viewport: (320.0, 96.0),
                        direction: LayoutDirection::Rtl,
                    },
                );
                let role = match kind {
                    4 => Role::CheckBox,
                    5 => Role::Switch,
                    6 => Role::RadioButton,
                    _ => Role::TabList,
                };
                let (id, node) = update
                    .nodes
                    .iter()
                    .find(|(_, node)| node.role() == role)
                    .unwrap();
                assert_eq!(node.is_disabled(), disabled);
                if kind < 7 {
                    assert_eq!(node.supports_action(Action::Click), !disabled);
                }
                assert_eq!(node.supports_action(Action::Focus), kind < 7 && !disabled);
                if kind == 7 {
                    assert_eq!(node.children().len(), 2);
                    let bounds = node.bounds().unwrap();
                    let tab_width = (bounds.x1 - bounds.x0) / 2.0;
                    for (index, &tab_id) in node.children().iter().enumerate() {
                        assert_eq!(
                            Some(tab_id.0),
                            field(captured, kind).tab_focus_id(&[], index)
                        );
                        let (_, tab) = update.nodes.iter().find(|(id, _)| *id == tab_id).unwrap();
                        assert_eq!(tab.role(), Role::Tab);
                        assert_eq!(tab.is_selected(), Some(index == 1));
                        assert_eq!(tab.is_disabled(), disabled);
                        assert_eq!(tab.supports_action(Action::Focus), !disabled);
                        assert_eq!(tab.supports_action(Action::Click), !disabled);
                        let tab_bounds = tab.bounds().unwrap();
                        assert!(
                            (tab_bounds.x0 - bounds.x0 - index as f64 * tab_width).abs() <= 1.0
                        );
                        assert!((tab_bounds.x1 - tab_bounds.x0 - tab_width).abs() <= 1.0);
                        assert_eq!((tab_bounds.y0, tab_bounds.y1), (bounds.y0, bounds.y1));
                    }
                }
                if kind < 6 {
                    assert_eq!(node.toggled(), Some(Toggled::True));
                }
                if kind == 6 {
                    assert_eq!(node.is_selected(), Some(true));
                }
                let owned = (
                    *id,
                    node.role(),
                    node.label().map(str::to_owned),
                    node.bounds(),
                    node.toggled(),
                    node.is_selected(),
                    node.is_disabled(),
                );
                if captured {
                    assert_eq!(owned, expected_summary.take().unwrap());
                } else {
                    expected_summary = Some(owned);
                }
            }
        }
    }
}

#[test]
fn captured_choices_group_geometry_preserves_peer_insets_gaps_and_scroll_document_height() {
    for width in [320, 800] {
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            let mut expected = None;
            for captured in [false, true] {
                let widget = Widget::scroll_view(
                    Widget::Column {
                        children: (4..8).map(|kind| field(captured, kind)).collect(),
                        style: Style {
                            gap: Size {
                                width: LengthPercentage::length(0.0),
                                height: LengthPercentage::length(crate::spacing::MD),
                            },
                            flex_shrink: 0.0,
                            ..Style::default()
                        },
                    },
                    Style {
                        size: Size::from_lengths(width as f32, 96.0),
                        ..Style::default()
                    },
                );
                let mut tree = TaffyTree::new();
                let root = build_layout_tree(&mut tree, &widget, direction);
                compute_layout_with_renderer(
                    &mut tree,
                    root,
                    PhysicalSize::new(width, 96),
                    &RichTextRenderer::new(),
                );
                let actual = geometry(&tree, root);
                if let Some(expected) = &expected {
                    assert_eq!(&actual, expected);
                } else {
                    expected = Some(actual);
                }
                let document = tree.children(root).unwrap()[0];
                let document_layout = tree.layout(document).unwrap();
                assert!(document_layout.size.height > 96.0);
                let mut previous_bottom: Option<f32> = None;
                let mut start_edge: Option<f32> = None;
                for node in tree.children(document).unwrap() {
                    let layout = tree.layout(node).unwrap();
                    assert!(layout.size.width > 0.0 && layout.size.height > 0.0);
                    if let Some(bottom) = previous_bottom {
                        assert!((layout.location.y - bottom - crate::spacing::MD).abs() <= 1.0);
                    }
                    if let Some(edge) = start_edge {
                        assert!((layout.location.x - edge).abs() <= 1.0);
                    }
                    let bottom = layout.location.y + layout.size.height;
                    assert!(bottom <= document_layout.size.height + 1.0);
                    assert!(layout.location.x >= -1.0);
                    assert!(
                        layout.location.x + layout.size.width <= document_layout.size.width + 1.0
                    );
                    previous_bottom = Some(bottom);
                    start_edge = Some(layout.location.x);
                }
            }
        }
    }
}
