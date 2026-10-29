use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::{FontSystem, SwashCache};
use rutter::layout::{RutterContext, build_taffy_tree_with_direction, compute_layout};
use rutter::render::{
    RichTextRenderer, draw_widgets,
    hit_test::{HitResult, hit_test},
    text::TextBufferCache,
};
use rutter::{ButtonVariant, DynamicIsland, LayoutDirection, Theme, Widget, WidgetIdSnapshot};
use skia_safe::{Color, Point, Rect, surfaces};
use taffy::prelude::*;
use winit::dpi::PhysicalSize;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Toggle,
    Child,
}

const SUMMARY: &str = "Download complete — عرض تفاصيل التنزيل — Dies ist eine lange lokalisierte Zusammenfassung, die vollständig umbrechen muss. Download complete — expand or collapse details.";

fn details() -> Widget<'static, Msg> {
    Widget::Button {
        text: "Child action",
        on_press: Msg::Child,
        style: Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(44.0),
            },
            flex_shrink: 0.0,
            ..Style::default()
        },
        color: None,
        variant: ButtonVariant::Primary,
    }
}

fn layout(
    view: &Widget<'_, Msg>,
    width: u32,
    direction: LayoutDirection,
) -> (TaffyTree<RutterContext>, NodeId) {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut tree = TaffyTree::new();
    let root =
        build_taffy_tree_with_direction(&mut tree, view, fonts.clone(), &HashMap::new(), direction);
    // A short viewport must not consume intrinsic document height.
    compute_layout(
        &mut tree,
        root,
        PhysicalSize::new(width, 100),
        fonts,
        &RichTextRenderer::default(),
    );
    (tree, root)
}

fn bounds(tree: &TaffyTree<RutterContext>, root: NodeId, path: &[usize]) -> Rect {
    let mut node = root;
    let mut origin = Point::default();
    for &index in path {
        let layout = tree.layout(node).unwrap();
        origin.x += layout.location.x;
        origin.y += layout.location.y;
        node = tree.children(node).unwrap()[index];
    }
    let layout = tree.layout(node).unwrap();
    Rect::from_xywh(
        origin.x + layout.location.x,
        origin.y + layout.location.y,
        layout.size.width,
        layout.size.height,
    )
}

#[test]
fn composed_geometry_and_child_hits_survive_expansion_summary_changes_and_direction() {
    for width in [320, 800] {
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            let mut state = DynamicIsland::interactive();
            let mut previous: Option<WidgetIdSnapshot> = None;
            for expanded in [false, true, false, true] {
                state.set_expanded(expanded).unwrap();
                let view = state
                    .interactive_view(SUMMARY, Msg::Toggle, details(), &Theme::light())
                    .unwrap();
                let snapshot = WidgetIdSnapshot::capture(&view).unwrap();
                if let Some(previous) = previous {
                    previous.validate_transition_to(&snapshot).unwrap();
                }
                previous = Some(snapshot);
                let (tree, root) = layout(&view, width, direction);
                let outer = bounds(&tree, root, &[]);
                let header = bounds(&tree, root, &[0, 0]);
                assert_eq!(outer.width(), width as f32);
                if width == 320 || expanded {
                    assert!(outer.height() > 100.0);
                }
                let mut tall_tree = tree.clone();
                compute_layout(
                    &mut tall_tree,
                    root,
                    PhysicalSize::new(width, 700),
                    Rc::new(RefCell::new(FontSystem::new())),
                    &RichTextRenderer::default(),
                );
                assert_eq!(
                    bounds(&tall_tree, root, &[]),
                    outer,
                    "viewport height must not truncate the document"
                );
                assert_eq!(header.left, 16.0);
                assert_eq!(outer.right - header.right, 16.0);
                assert!(header.height() >= 44.0);
                let hit = hit_test(
                    &view,
                    &tree,
                    root,
                    Point::new(header.center_x(), header.center_y()),
                    Point::default(),
                    &HashMap::new(),
                );
                assert!(matches!(
                    hit,
                    Some(HitResult::Message {
                        msg: Msg::Toggle,
                        focus_id: Some(_),
                        ..
                    })
                ));
                if expanded {
                    let child = bounds(&tree, root, &[0, 1]);
                    assert_eq!(child.left, header.left);
                    assert_eq!(child.right, header.right);
                    assert_eq!(child.top - header.bottom, 12.0);
                    assert!(child.bottom <= outer.bottom - 16.0);
                    assert!(matches!(
                        hit_test(
                            &view,
                            &tree,
                            root,
                            Point::new(child.center_x(), child.center_y()),
                            Point::default(),
                            &HashMap::new()
                        ),
                        Some(HitResult::Message {
                            msg: Msg::Child,
                            focus_id: Some(_),
                            ..
                        })
                    ));
                } else {
                    let column = tree.children(root).unwrap()[0];
                    assert_eq!(tree.children(column).unwrap().len(), 1);
                }
                let disabled = view.enabled(false);
                assert!(
                    hit_test(
                        &disabled,
                        &tree,
                        root,
                        Point::new(header.center_x(), header.center_y()),
                        Point::default(),
                        &HashMap::new()
                    )
                    .is_none()
                );
            }
        }
    }
}

#[test]
fn arbitrary_nested_details_keep_editor_identity_validation_and_independent_hits() {
    let mut island = DynamicIsland::interactive();
    island.set_expanded(true).unwrap();
    let editor = Widget::controlled_text_input(
        "draft",
        |_| Msg::Child,
        None,
        Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(40.0),
            },
            ..Style::default()
        },
        "Localized editor label",
        "",
        rutter::InputState::Error,
        Some("A complete validation message remains visible".into()),
        false,
    )
    .with_id(71);
    let details = Widget::Column {
        children: vec![editor, details()],
        style: Style {
            flex_direction: FlexDirection::Column,
            gap: Size::length(rutter::spacing::SM),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::auto(),
            },
            ..Style::default()
        },
    };
    let view = island
        .interactive_view(SUMMARY, Msg::Toggle, details, &Theme::dark())
        .unwrap();
    let (tree, root) = layout(&view, 320, LayoutDirection::Rtl);
    let editor = bounds(&tree, root, &[0, 1, 0]);
    let action = bounds(&tree, root, &[0, 1, 1]);
    assert!(editor.height() > 40.0, "annotations own allocated height");
    assert!(action.top >= editor.bottom + rutter::spacing::SM);
    let point = Point::new(editor.center_x(), editor.top + 36.0);
    assert!(matches!(
        hit_test(&view, &tree, root, point, Point::default(), &HashMap::new()),
        Some(HitResult::InputFocus { id: 71, .. })
    ));
    assert!(matches!(
        hit_test(
            &view,
            &tree,
            root,
            Point::new(action.center_x(), action.center_y()),
            Point::default(),
            &HashMap::new()
        ),
        Some(HitResult::Message {
            msg: Msg::Child,
            ..
        })
    ));
    let disabled = view.enabled(false);
    for point in [point, Point::new(action.center_x(), action.center_y())] {
        assert!(
            hit_test(
                &disabled,
                &tree,
                root,
                point,
                Point::default(),
                &HashMap::new()
            )
            .is_none()
        );
    }
}

#[test]
fn capsules_rasterize_wrapped_text_inside_allocation_in_both_themes_directions_and_scales() {
    for width in [320, 800] {
        for theme in [Theme::light(), Theme::dark()] {
            for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                for passive in [true, false] {
                    let mut state = if passive {
                        DynamicIsland::passive()
                    } else {
                        DynamicIsland::interactive()
                    };
                    if !passive {
                        state.set_expanded(true).unwrap();
                    }
                    let view = if passive {
                        state.passive_view(SUMMARY, &theme).unwrap()
                    } else {
                        state
                            .interactive_view(SUMMARY, Msg::Toggle, details(), &theme)
                            .unwrap()
                    };
                    let (tree, root) = layout(&view, width, direction);
                    let header = bounds(&tree, root, &[0, 0]);
                    let focus = match hit_test(
                        &view,
                        &tree,
                        root,
                        Point::new(header.center_x(), header.center_y()),
                        Point::default(),
                        &HashMap::new(),
                    ) {
                        Some(HitResult::Message { focus_id, .. }) => focus_id,
                        _ => None,
                    };
                    let outer = bounds(&tree, root, &[]);
                    let text = bounds(&tree, root, if passive { &[0, 0] } else { &[0, 0, 0] });
                    assert!(text.height() > 20.0, "long summary must wrap");
                    assert!(text.left >= 16.0 && text.right <= outer.right - 16.0);
                    assert!(text.top >= 16.0 && text.bottom <= outer.bottom - 16.0);
                    for scale in [1.0_f32, 1.5, 2.0] {
                        let mut surface = surfaces::raster_n32_premul((
                            ((outer.width() + 4.0) * scale).ceil() as i32,
                            ((outer.height() + 4.0) * scale).ceil() as i32,
                        ))
                        .unwrap();
                        surface.canvas().clear(Color::TRANSPARENT);
                        surface.canvas().scale((scale, scale));
                        draw_widgets(
                            surface.canvas(),
                            &tree,
                            root,
                            &view,
                            &mut FontSystem::new(),
                            &mut SwashCache::new(),
                            Point::new(-1.0, -1.0),
                            focus,
                            &HashMap::new(),
                            &HashMap::new(),
                            &mut HashMap::new(),
                            &mut TextBufferCache::default(),
                            true,
                            &theme,
                            scale,
                        );
                        let pixels = surface.peek_pixels().unwrap();
                        assert_eq!(
                            pixels.get_color((0, 0)).a(),
                            0,
                            "transparent rounded corner"
                        );
                        assert_eq!(
                            pixels.get_color((
                                (outer.center_x() * scale) as i32,
                                (8.0 * scale) as i32
                            )),
                            theme.surface
                        );
                        let mut upper_ink = 0;
                        let mut lower_ink = 0;
                        for y in
                            (text.top * scale).ceil() as i32..(text.bottom * scale).floor() as i32
                        {
                            for x in (text.left * scale).ceil() as i32
                                ..(text.right * scale).floor() as i32
                            {
                                let color = pixels.get_color((x, y));
                                if color.a() > 0 && color != theme.surface {
                                    if y as f32 / scale < text.center_y() {
                                        upper_ink += 1;
                                    } else {
                                        lower_ink += 1;
                                    }
                                }
                            }
                        }
                        assert!(
                            upper_ink > 20 && lower_ink > 20,
                            "wrapped lower lines must paint: {width} {direction:?} {scale}"
                        );
                        for y in 0..pixels.height() {
                            assert_eq!(
                                pixels
                                    .get_color((((outer.right + 2.0) * scale) as i32, y))
                                    .a(),
                                0,
                                "paint escaped allocation"
                            );
                        }
                    }
                    if passive {
                        assert!(
                            hit_test(
                                &view,
                                &tree,
                                root,
                                Point::new(text.center_x(), text.center_y()),
                                Point::default(),
                                &HashMap::new()
                            )
                            .is_none()
                        );
                    }
                }
            }
        }
    }
}
