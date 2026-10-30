use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::{FontSystem, SwashCache};
use skia_safe::{Color, Point, surfaces};

use super::*;
use crate::{
    LayoutDirection,
    activity::{ActivityRegistry, Media, MediaState, Transfer, TransferState},
    layout::{RutterContext, build_taffy_tree, compute_layout},
    render::{
        RichTextRenderer, draw_widgets,
        hit_test::{HitResult, hit_test},
        text::TextBufferCache,
    },
};

type Message = (ActivityId, ActivityAction);

fn activity(content: impl Into<ActivityContent>) -> Activity {
    let mut registry = ActivityRegistry::new();
    let owner = registry.register_owner().unwrap();
    let id = registry.start(owner, content).unwrap();
    registry.get(id).unwrap().clone()
}

fn transfer(total: Option<u64>, state: TransferState) -> Activity {
    activity(Transfer::new("Simulation · téléchargement طويل", TransferDirection::Download, 10, total, state).unwrap()
        .with_supporting("Long supporting text wraps naturally without consuming the controls. Eine längere Beschreibung für schmale Fenster. تفاصيل إضافية.").unwrap())
}

fn layout(widget: &Widget<'_, Message>, width: u32) -> (TaffyTree<RutterContext>, NodeId) {
    let mut tree = TaffyTree::new();
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let root = build_taffy_tree(&mut tree, widget, fonts.clone(), &HashMap::new());
    compute_layout(
        &mut tree,
        root,
        winit::dpi::PhysicalSize::new(width, 120),
        fonts,
        &RichTextRenderer::default(),
    );
    (tree, root)
}

fn action_hits(
    widget: &Widget<'_, Message>,
    tree: &TaffyTree<RutterContext>,
    root: NodeId,
) -> Vec<(Message, Option<u64>)> {
    let children = tree.children(root).unwrap();
    let Widget::Column {
        children: widgets, ..
    } = widget
    else {
        panic!()
    };
    let Some((index, Widget::Row { .. })) = widgets
        .iter()
        .enumerate()
        .find(|(_, w)| matches!(w, Widget::Row { .. }))
    else {
        return vec![];
    };
    let row = children[index];
    let origin = tree.layout(row).unwrap().location;
    tree.children(row)
        .unwrap()
        .iter()
        .map(|&node| {
            let bounds = tree.layout(node).unwrap();
            assert!(bounds.size.width >= 88.0 && bounds.size.height >= 40.0);
            let position = Point::new(
                origin.x + bounds.location.x + bounds.size.width / 2.0,
                origin.y + bounds.location.y + bounds.size.height / 2.0,
            );
            match hit_test(
                widget,
                tree,
                root,
                position,
                Point::default(),
                &HashMap::new(),
            ) {
                Some(HitResult::Message { msg, focus_id }) => (msg, focus_id),
                _ => panic!("painted action body is not interactive"),
            }
        })
        .collect()
}

fn assert_geometry(tree: &TaffyTree<RutterContext>, node: NodeId) {
    let parent = tree.layout(node).unwrap();
    let children = tree.children(node).unwrap();
    for (index, &child) in children.iter().enumerate() {
        let bounds = tree.layout(child).unwrap();
        assert!(bounds.size.width > 0.0 && bounds.size.height > 0.0);
        assert!(bounds.location.x >= -1.0 && bounds.location.y >= -1.0);
        assert!(bounds.location.x + bounds.size.width <= parent.size.width + 1.0);
        assert!(bounds.location.y + bounds.size.height <= parent.size.height + 1.0);
        for &other in &children[index + 1..] {
            let other = tree.layout(other).unwrap();
            assert!(
                bounds.location.x + bounds.size.width <= other.location.x + 1.0
                    || other.location.x + other.size.width <= bounds.location.x + 1.0
                    || bounds.location.y + bounds.size.height <= other.location.y + 1.0
                    || other.location.y + other.size.height <= bounds.location.y + 1.0,
                "overlapping siblings"
            );
        }
        assert_geometry(tree, child);
    }
}

#[test]
fn switching_activities_and_replacing_state_keeps_typed_actions_current() {
    let first = transfer(Some(100), TransferState::Running);
    let second = transfer(None, TransferState::Paused);
    for activity in [&first, &second, &first] {
        let view = details(activity, |id, action| (id, action), &Theme::light());
        let (tree, root) = layout(&view, 320);
        let hits = action_hits(&view, &tree, root);
        assert_eq!(
            hits.iter().map(|(message, _)| *message).collect::<Vec<_>>(),
            activity
                .supported_actions()
                .iter()
                .map(|&action| (activity.id(), action))
                .collect::<Vec<_>>()
        );
        assert!(hits.iter().all(|(_, id)| id.is_some()));
    }
    let completed = transfer(Some(10), TransferState::Completed);
    let view = details(&completed, |id, action| (id, action), &Theme::light());
    let (tree, root) = layout(&view, 320);
    assert!(action_hits(&view, &tree, root).is_empty());
}

#[test]
fn summaries_preserve_unknown_zero_totals_and_media_times() {
    assert!(summary(&transfer(None, TransferState::Running)).contains("total unknown"));
    let zero = activity(
        Transfer::new(
            "Empty",
            TransferDirection::Upload,
            0,
            Some(0),
            TransferState::Completed,
        )
        .unwrap(),
    );
    assert!(summary(&zero).contains("100%"));
    let media = activity(
        Media::new(
            "Track",
            "Artist",
            Duration::from_secs(61),
            Some(Duration::from_secs(185)),
            MediaState::Paused,
        )
        .unwrap(),
    );
    assert!(summary(&media).contains("1:01 / 3:05"));
    let live =
        activity(Media::new("Live", "Artist", Duration::ZERO, None, MediaState::Playing).unwrap());
    assert!(summary(&live).contains("live / unknown duration"));
}

#[test]
fn progress_representation_matches_known_unknown_and_paused_snapshots() {
    for (total, state, expected) in [
        (Some(100), TransferState::Running, false),
        (None, TransferState::Running, true),
        (None, TransferState::Paused, false),
        (None, TransferState::Completed, false),
    ] {
        let activity = transfer(total, state);
        let Widget::Column { children, .. } =
            details(&activity, |id, action| (id, action), &Theme::light())
        else {
            panic!()
        };
        let (value, indeterminate) = children
            .iter()
            .find_map(|w| match w {
                Widget::ProgressBar {
                    value,
                    indeterminate,
                    ..
                } => Some((*value, *indeterminate)),
                _ => None,
            })
            .unwrap();
        assert_eq!(indeterminate, expected);
        assert!(value.is_finite() && (0.0..=1.0).contains(&value));
    }
}

#[test]
fn details_geometry_hits_accessibility_and_raster_at_narrow_desktop_and_scales() {
    let activities = [
        transfer(Some(100), TransferState::Running),
        transfer(None, TransferState::Paused),
        activity(
            Media::new(
                "Simulation · A long localized track title for a narrow window",
                "Artist · no playback",
                Duration::from_secs(61),
                Some(Duration::from_secs(180)),
                MediaState::Playing,
            )
            .unwrap(),
        ),
    ];
    for theme in [Theme::light(), Theme::dark()] {
        for width in [320, 800] {
            for activity in &activities {
                let widget = details(activity, |id, action| (id, action), &theme);
                let (tree, root) = layout(&widget, width);
                assert_geometry(&tree, root);
                let height = tree.layout(root).unwrap().size.height;
                assert!(height > 120.0, "short viewports need a scrolling parent");
                let hits = action_hits(&widget, &tree, root);
                let focused = hits[0].1;
                let update = crate::accessibility::build_accessibility_update(
                    &tree,
                    &widget,
                    root,
                    crate::accessibility::AccessibilityInputs {
                        input_states: &HashMap::new(),
                        widget_states: &HashMap::new(),
                        focused_widget_id: focused,
                        viewport: (width as f32, height),
                        direction: LayoutDirection::Ltr,
                    },
                );
                assert_eq!(
                    update
                        .nodes
                        .iter()
                        .filter(|(_, node)| node.supports_action(accesskit::Action::Click))
                        .count(),
                    activity.supported_actions().len()
                );
                for scale in [1.0, 1.5, 2.0] {
                    let mut surface = surfaces::raster_n32_premul((
                        ((width as f32 + 16.0) * scale).ceil() as i32,
                        ((height + 16.0) * scale).ceil() as i32,
                    ))
                    .unwrap();
                    let canvas = surface.canvas();
                    canvas.clear(Color::TRANSPARENT);
                    canvas.scale((scale, scale));
                    canvas.translate((8.0, 8.0));
                    draw_widgets(
                        canvas,
                        &tree,
                        root,
                        &widget,
                        &mut FontSystem::new(),
                        &mut SwashCache::new(),
                        Point::new(-100.0, -100.0),
                        focused,
                        &HashMap::new(),
                        &HashMap::new(),
                        &mut HashMap::new(),
                        &mut TextBufferCache::default(),
                        false,
                        &theme,
                        scale,
                    );
                    let pixels = surface.peek_pixels().unwrap();
                    for &child in &tree.children(root).unwrap() {
                        let bounds = tree.layout(child).unwrap();
                        let rows = ((bounds.location.y + 8.0) * scale) as i32
                            ..((bounds.location.y + bounds.size.height + 8.0) * scale) as i32;
                        assert!(
                            rows.into_iter().any(|y| {
                                (8..((width as f32 + 8.0) * scale) as i32)
                                    .any(|x| pixels.get_color((x, y)) != Color::TRANSPARENT)
                            }),
                            "allocated content must actually paint"
                        );
                    }
                    for y in 0..pixels.height() {
                        assert_eq!(pixels.get_color((0, y)), Color::TRANSPARENT);
                        assert_eq!(
                            pixels.get_color((pixels.width() - 1, y)),
                            Color::TRANSPARENT
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn short_scroll_view_reveals_action_bodies_and_clips_the_document() {
    use crate::engine::widget_state::{ScrollState, WidgetState};
    for theme in [Theme::light(), Theme::dark()] {
        for width in [320, 800] {
            let activity = transfer(Some(100), TransferState::Running);
            let widget = Widget::ScrollView {
                id: 900,
                style: Style {
                    size: Size::from_lengths(width as f32, 120.0),
                    ..Style::default()
                },
                child: Box::new(details(&activity, |id, action| (id, action), &theme)),
            };
            let (tree, root) = layout(&widget, width);
            let document = tree.children(root).unwrap()[0];
            let document_bounds = tree.layout(document).unwrap();
            assert!(document_bounds.size.height > 120.0);
            let offset = document_bounds.size.height - 120.0;
            let states = HashMap::from([(
                900,
                WidgetState::Scroll(ScrollState {
                    offset_y: offset,
                    content_height: document_bounds.size.height,
                    viewport_h: 120.0,
                }),
            )]);
            let row = *tree.children(document).unwrap().last().unwrap();
            let row_bounds = tree.layout(row).unwrap();
            let button = tree.children(row).unwrap()[0];
            let bounds = tree.layout(button).unwrap();
            let target = Point::new(
                document_bounds.location.x
                    + row_bounds.location.x
                    + bounds.location.x
                    + bounds.size.width / 2.0,
                document_bounds.location.y
                    + row_bounds.location.y
                    + bounds.location.y
                    + bounds.size.height / 2.0
                    - offset,
            );
            assert!((0.0..120.0).contains(&target.y));
            assert!(
                matches!(hit_test(&widget, &tree, root, target, Point::default(), &states), Some(HitResult::Message { msg: (id, ActivityAction::Pause), .. }) if id == activity.id())
            );
            let mut surface = surfaces::raster_n32_premul((width as i32 + 16, 160)).unwrap();
            let canvas = surface.canvas();
            canvas.clear(Color::TRANSPARENT);
            canvas.translate((8.0, 8.0));
            draw_widgets(
                canvas,
                &tree,
                root,
                &widget,
                &mut FontSystem::new(),
                &mut SwashCache::new(),
                Point::new(-100.0, -100.0),
                None,
                &HashMap::new(),
                &states,
                &mut HashMap::new(),
                &mut TextBufferCache::default(),
                false,
                &theme,
                1.0,
            );
            let pixels = surface.peek_pixels().unwrap();
            assert!((8..128).any(|y| {
                (8..width as i32).any(|x| pixels.get_color((x, y)) != Color::TRANSPARENT)
            }));
            for y in 129..160 {
                for x in 0..width as i32 + 16 {
                    assert_eq!(
                        pixels.get_color((x, y)),
                        Color::TRANSPARENT,
                        "scroll content escaped viewport"
                    );
                }
            }
            let disabled = widget.enabled(false);
            assert!(!matches!(
                hit_test(&disabled, &tree, root, target, Point::default(), &states),
                Some(HitResult::Message { .. })
            ));
        }
    }
}

#[test]
fn interactive_capsule_retains_summary_but_hides_detail_accessibility_when_collapsed() {
    let activity = transfer(Some(100), TransferState::Running);
    let error = "Pause delivery failed. Try again.";
    let mut island = crate::DynamicIsland::interactive();
    for theme in [Theme::light(), Theme::dark()] {
        for width in [320, 800] {
            for expanded in [true, false, true] {
                island.set_expanded(expanded).unwrap();
                let summary = format!(
                    "{} · {} details",
                    summary(&activity),
                    if expanded { "Collapse" } else { "Expand" }
                );
                let widget = island
                    .interactive_view(
                        &summary,
                        (activity.id(), ActivityAction::Next),
                        Widget::Column {
                            children: vec![
                                details(&activity, |id, action| (id, action), &theme),
                                text(error, &theme),
                            ],
                            style: Style {
                                gap: Size::length(spacing::MD),
                                ..fluid()
                            },
                        },
                        &theme,
                    )
                    .unwrap();
                let (tree, root) = layout(&widget, width);
                assert_geometry(&tree, root);
                let height = tree.layout(root).unwrap().size.height;
                let update = crate::accessibility::build_accessibility_update(
                    &tree,
                    &widget,
                    root,
                    crate::accessibility::AccessibilityInputs {
                        input_states: &HashMap::new(),
                        widget_states: &HashMap::new(),
                        focused_widget_id: None,
                        viewport: (width as f32, height),
                        direction: LayoutDirection::Ltr,
                    },
                );
                assert!(
                    update
                        .nodes
                        .iter()
                        .any(|(_, node)| node.label() == Some(summary.as_str())
                            && node.supports_action(accesskit::Action::Click))
                );
                for label in ["Pause", "Cancel", error] {
                    assert_eq!(
                        update
                            .nodes
                            .iter()
                            .any(|(_, node)| node.label() == Some(label)),
                        expanded,
                        "detail accessibility for {label}"
                    );
                }
                assert_eq!(
                    update
                        .nodes
                        .iter()
                        .filter(|(_, node)| node.supports_action(accesskit::Action::Click))
                        .count(),
                    if expanded { 3 } else { 1 }
                );
                let mut surface =
                    surfaces::raster_n32_premul((width as i32 + 16, height.ceil() as i32 + 16))
                        .unwrap();
                let canvas = surface.canvas();
                canvas.clear(Color::TRANSPARENT);
                canvas.translate((8.0, 8.0));
                draw_widgets(
                    canvas,
                    &tree,
                    root,
                    &widget,
                    &mut FontSystem::new(),
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
                let pixels = surface.peek_pixels().unwrap();
                assert_eq!(
                    pixels.get_color((8 + width as i32 / 2, 10)),
                    theme.surface,
                    "capsule surface must paint"
                );
                assert_eq!(pixels.get_color((0, 0)), Color::TRANSPARENT);
            }
        }
    }
}
