//! Callback ownership must not change native allocations, identity or pixels.
use super::*;
use crate::layout::{build_layout_tree, compute_layout_with_renderer};
use crate::render::hit_test::{
    find_input_callbacks, find_input_props_with, find_slider_callback, find_slider_callback_with,
};
use crate::widget::id::validate_widget_id_snapshot;
use skia_safe::{Color, surfaces};
use taffy::prelude::{Size, Style};
use winit::dpi::PhysicalSize;

fn field(captured: bool, kind: usize) -> Widget<'static, ()> {
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
        (false, _) => Widget::slider(42.0, 0.0, 100.0, 1.0, |_| (), style, "Amount"),
        (true, _) => Widget::slider_with(42.0, 0.0, 100.0, 1.0, |_| (), style, "Amount"),
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
    for kind in 0..4 {
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
                            Some(42),
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
