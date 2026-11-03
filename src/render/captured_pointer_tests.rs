//! Pointer callback ownership must not alter composition, identity or painting.
use super::*;
use crate::accessibility::{AccessibilityInputs, build_accessibility_update};
use crate::widget::id::validate_widget_id_snapshot;
use crate::*;
use skia_safe::{Color, surfaces};
use taffy::prelude::*;

fn region(captured: bool) -> Widget<'static, ()> {
    let child = Widget::text_input(
        |_| (),
        None,
        Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::auto(),
            },
            ..Style::default()
        },
        "Localized selection 東京 القاهرة",
        "Long placeholder for pointer region",
        InputState::Idle,
        Some("Validation message remains inside the input".into()),
        false,
    );
    let style = Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::auto(),
        },
        padding: Rect::length(crate::spacing::LG),
        ..Style::default()
    };
    if captured {
        Widget::pointer_region_with(
            WidgetId::manual(100).unwrap(),
            child,
            PointerRegionConfigWith::new(|_| ()).with_hover(|_| ()),
            style,
        )
    } else {
        Widget::pointer_region(
            WidgetId::manual(100).unwrap(),
            child,
            PointerRegionConfig::new(|_| ()).with_hover(|_| ()),
            style,
        )
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
fn pointer_region_captured_identity_geometry_accessibility_hit_and_raster_parity() {
    let mut fonts = FontSystem::new();
    let mut swash = SwashCache::new();
    let mut font_cache = HashMap::new();
    let mut text_cache = TextBufferCache::default();
    let renderer = RichTextRenderer::new();
    for (width, direction, theme, scale) in [
        (320, LayoutDirection::Ltr, Theme::light(), 1.0),
        (320, LayoutDirection::Rtl, Theme::dark(), 1.5),
        (800, LayoutDirection::Rtl, Theme::light(), 2.0),
        (800, LayoutDirection::Ltr, Theme::dark(), 1.0),
    ] {
        for disabled in [false, true] {
            let widgets = [
                region(false).enabled(!disabled),
                region(true).enabled(!disabled),
            ];
            assert_eq!(
                validate_widget_id_snapshot(&widgets[0]).unwrap(),
                validate_widget_id_snapshot(&widgets[1]).unwrap()
            );
            let mut geometries = Vec::new();
            let mut pixels = Vec::new();
            let mut semantics = Vec::new();
            let mut hits = Vec::new();
            for widget in &widgets {
                let mut tree = TaffyTree::new();
                let root = build_layout_tree(&mut tree, widget, direction);
                compute_layout_with_renderer(
                    &mut tree,
                    root,
                    PhysicalSize::new(width, 120),
                    &renderer,
                );
                let rectangles = geometry(&tree, root);
                assert!(
                    rectangles
                        .iter()
                        .all(|(_, _, width, height)| *width > 0.0 && *height > 0.0)
                );
                let (x, y, child_width, child_height) = rectangles[1];
                assert!((x - spacing::LG).abs() <= 1.0 && (y - spacing::LG).abs() <= 1.0);
                assert!(x + child_width <= rectangles[0].2 + 1.0);
                assert!(y + child_height <= rectangles[0].3 + 1.0);
                geometries.push(rectangles);
                let inputs = HashMap::new();
                let states = HashMap::new();
                hits.push(
                    match crate::render::hit_test::hit_test(
                        widget,
                        &tree,
                        root,
                        Point::new(24.0, 40.0),
                        Point::new(0.0, 0.0),
                        &states,
                    ) {
                        Some(crate::render::hit_test::HitResult::InputFocus {
                            id,
                            local_x,
                            local_y,
                            width,
                            height,
                        }) => Some((id, local_x, local_y, width, height)),
                        None => None,
                        _ => {
                            panic!(
                                "hover-only pointer region must preserve child input hit testing"
                            )
                        }
                    },
                );
                let update = build_accessibility_update(
                    &tree,
                    widget,
                    root,
                    AccessibilityInputs {
                        input_states: &inputs,
                        widget_states: &states,
                        focused_widget_id: None,
                        viewport: (width as f32, 120.0),
                        direction,
                    },
                );
                semantics.push(
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
                            )
                        })
                        .collect::<Vec<_>>(),
                );
                let mut surface = surfaces::raster_n32_premul((
                    (width as f32 * scale) as i32,
                    (120.0 * scale) as i32,
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
                    None,
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
            assert_eq!(geometries[0], geometries[1]);
            assert_eq!(semantics[0], semantics[1]);
            assert_eq!(hits[0], hits[1]);
            assert!(pixels[0].iter().any(|byte| *byte != 0));
            assert!(
                pixels[0] == pixels[1],
                "width={width} scale={scale} disabled={disabled}"
            );
        }
    }
}
