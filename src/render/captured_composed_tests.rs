//! Shared composition must retain legacy geometry, IDs, accessibility and pixels.
use super::*;
use crate::accessibility::{AccessibilityInputs, build_accessibility_update};
use crate::widget::id::validate_widget_id_snapshot;
use crate::*;
use skia_safe::{Color, surfaces};
use taffy::prelude::*;

fn composed<'a>(
    captured: bool,
    family: usize,
    state: &'a ColorPickerState,
    activity: &activity::Activity,
) -> Widget<'a, ()> {
    let style = Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::auto(),
        },
        flex_shrink: 0.0,
        ..Style::default()
    };
    let anchor = Style {
        size: Size::from_lengths(280.0, 44.0),
        ..Style::default()
    };
    let month = CalendarMonth::new(2026, 7).unwrap();
    let config = CalendarConfig::new(CalendarLabels::PORTUGUESE, WeekStart::Monday);
    let popup = Style {
        size: Size {
            width: Dimension::length(300.0),
            height: Dimension::auto(),
        },
        ..Style::default()
    };
    match (captured, family) {
        (false, 0) => Widget::calendar_with_config(month, None, |_| (), |_| (), config, style),
        (true, 0) => Widget::calendar_with_config_with(month, None, |_| (), |_| (), config, style),
        (false, 1) => Widget::date_picker_with_config(
            true,
            month,
            None,
            (),
            (),
            |_| (),
            |_| (),
            "Date 東京",
            "YYYY-MM-DD long localized placeholder",
            config,
            anchor,
            popup,
        ),
        (true, 1) => Widget::date_picker_with_config_with(
            true,
            month,
            None,
            (),
            (),
            |_| (),
            |_| (),
            "Date 東京",
            "YYYY-MM-DD long localized placeholder",
            config,
            anchor,
            popup,
        ),
        (false, 2) => Widget::time_picker(
            true,
            TimeOfDay::new(9, 30, 0).unwrap(),
            TimePickerConfig::new(
                &["UTC", "America/Sao_Paulo"],
                1,
                (),
                (),
                |_| (),
                |_| (),
                |_| (),
                |_| (),
                "Time",
            )
            .unwrap()
            .with_labels(TimePickerLabels::PORTUGUESE),
            anchor,
            popup,
        ),
        (true, 2) => Widget::time_picker_with(
            true,
            TimeOfDay::new(9, 30, 0).unwrap(),
            TimePickerConfigWith::new(
                &["UTC", "America/Sao_Paulo"],
                1,
                (),
                (),
                |_| (),
                |_| (),
                |_| (),
                |_| (),
                "Time",
            )
            .unwrap()
            .with_labels(TimePickerLabels::PORTUGUESE),
            anchor,
            popup,
        ),
        (false, 3) => Widget::color_picker(
            state,
            ColorPickerIds::new(100).unwrap(),
            ColorPickerCallbacks::new(|_| (), |_| (), |_| (), |_| (), |_| (), [|_| (); 5])
                .with_copy_button(()),
            style,
        ),
        (true, 3) => Widget::color_picker_with(
            state,
            ColorPickerIds::new(100).unwrap(),
            ColorPickerCallbacksWith::new(
                |_| (),
                |_| (),
                |_| (),
                |_| (),
                |_| (),
                std::array::from_fn(|_| ChangeCallback::new(|_| ())),
            )
            .with_copy_button(()),
            style,
        ),
        (false, _) => activity::view::details(activity, |_, _| (), &Theme::light()),
        (true, _) => activity::view::details_with(activity, |_, _| (), &Theme::light()),
    }
}

fn geometry(
    tree: &TaffyTree<crate::layout::RutterContext>,
    root: NodeId,
) -> Vec<(f32, f32, f32, f32)> {
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
fn captured_composed_identity_geometry_accessibility_and_raster_parity() {
    let mut fonts = FontSystem::new();
    let mut swash = SwashCache::new();
    let mut font_cache = HashMap::new();
    let mut text_cache = TextBufferCache::default();
    let renderer = RichTextRenderer::new();
    let mut registry = activity::ActivityRegistry::new();
    let owner = registry.register_owner().unwrap();
    let id = registry
        .start(
            owner,
            activity::Transfer::new(
                "Long download 東京 القاهرة",
                activity::TransferDirection::Download,
                1,
                Some(100),
                activity::TransferState::Running,
            )
            .unwrap(),
        )
        .unwrap();
    for family in 0..5 {
        for (width, direction, theme, scale) in [
            (320, LayoutDirection::Ltr, Theme::light(), 1.0),
            (320, LayoutDirection::Rtl, Theme::dark(), 1.5),
            (800, LayoutDirection::Rtl, Theme::light(), 2.0),
            (800, LayoutDirection::Ltr, Theme::dark(), 1.0),
        ] {
            for disabled in [false, true] {
                // Sample invalid RGB and HEX drafts without multiplying the whole matrix.
                let mut state = ColorPickerState::new(PickerColor::from_rgb8(103, 80, 164));
                if width == 320 {
                    assert!(
                        state
                            .update(ColorPickerEvent::Channel {
                                index: 0,
                                text: "invalid localized 東京".into()
                            })
                            .is_err()
                    );
                } else {
                    state.update(ColorPickerEvent::Format(1)).unwrap();
                    assert!(
                        state
                            .update(ColorPickerEvent::Hex("invalid".into()))
                            .is_err()
                    );
                }
                let widgets = [
                    composed(false, family, &state, registry.get(id).unwrap()).enabled(!disabled),
                    composed(true, family, &state, registry.get(id).unwrap()).enabled(!disabled),
                ];
                assert_eq!(
                    validate_widget_id_snapshot(&widgets[0]).unwrap(),
                    validate_widget_id_snapshot(&widgets[1]).unwrap()
                );
                let mut geometries = Vec::new();
                let mut accessibility = Vec::new();
                let mut pixels = Vec::new();
                for widget in &widgets {
                    let mut tree = TaffyTree::new();
                    let root = build_layout_tree(&mut tree, widget, direction);
                    compute_layout_with_renderer(
                        &mut tree,
                        root,
                        PhysicalSize::new(width, 240),
                        &renderer,
                    );
                    let bounds = geometry(&tree, root);
                    assert!(bounds.iter().all(|(x, y, w, h)| x.is_finite()
                        && y.is_finite()
                        && *w >= 0.0
                        && *h >= 0.0));
                    assert!(bounds[0].2 > 0.0 && bounds[0].3 > 0.0);
                    geometries.push(bounds);
                    let inputs = HashMap::new();
                    let states = HashMap::new();
                    let update = build_accessibility_update(
                        &tree,
                        widget,
                        root,
                        AccessibilityInputs {
                            input_states: &inputs,
                            widget_states: &states,
                            focused_widget_id: Some(100),
                            viewport: (width as f32, 240.0),
                            direction,
                        },
                    );
                    accessibility.push(
                        update
                            .nodes
                            .iter()
                            .map(|(id, node)| {
                                (
                                    *id,
                                    node.role(),
                                    node.label().map(str::to_owned),
                                    node.bounds(),
                                    node.is_disabled(),
                                    node.value().map(str::to_owned),
                                )
                            })
                            .collect::<Vec<_>>(),
                    );
                    let mut surface = surfaces::raster_n32_premul((
                        (width as f32 * scale) as i32,
                        (240.0 * scale) as i32,
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
                        Some(100),
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
                assert_eq!(geometries[0], geometries[1], "family={family}");
                assert_eq!(accessibility[0], accessibility[1], "family={family}");
                assert!(pixels[0].iter().any(|b| *b != 0));
                assert!(
                    pixels[0] == pixels[1],
                    "family={family} width={width} disabled={disabled}"
                );
            }
        }
    }
}
