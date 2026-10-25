// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::FontSystem;
use skia_safe::{Color, surfaces};
use taffy::prelude::*;
use winit::dpi::PhysicalSize;

use super::*;
use crate::engine::widget_state::{SearchState, WidgetState};
use crate::input_limits::{InputKind, InputLimits};
use crate::input_state::InputWidgetState;
use crate::layout::{build_taffy_tree, compute_layout};
use crate::render::RichTextRenderer;
use crate::widgets::search::{SearchMatcher, SearchSuggestions};

const VIEWPORT: (f32, f32) = (320.0, 240.0);
const SEARCH_ID: u64 = 41;

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestMessage {
    Query(String),
    Picked(usize),
}

fn suggestions<'a>() -> SearchSuggestions<'a, TestMessage> {
    SearchSuggestions::new(
        &["Matrix", "O Senhor dos Anéis", "Star Wars"],
        SearchMatcher::Fuzzy,
        5,
        Some(TestMessage::Picked),
    )
    .expect("test suggestions use a valid configuration")
}

fn non_selectable_suggestions<'a>() -> SearchSuggestions<'a, TestMessage> {
    SearchSuggestions::new(
        &["Matrix", "O Senhor dos Anéis", "Star Wars"],
        SearchMatcher::Fuzzy,
        5,
        None,
    )
    .expect("test suggestions use a valid configuration")
}

fn search_bar<'a>(suggestions: SearchSuggestions<'a, TestMessage>) -> Widget<'a, TestMessage> {
    Widget::SearchBar {
        id: SEARCH_ID,
        on_change: TestMessage::Query,
        on_submit: None,
        on_search: None,
        on_clear: None,
        placeholder: "Buscar",
        style: Style {
            size: Size::from_lengths(120.0, 40.0),
            ..Style::default()
        },
        suggestions: Some(suggestions),
    }
}

fn focused_states() -> HashMap<u64, WidgetState> {
    HashMap::from([(
        SEARCH_ID,
        WidgetState::Search(SearchState {
            hovered_option: None,
            dismissed: false,
        }),
    )])
}

/// An input state carrying `query`, as the engine would after typing.
fn typed_input(query: &str) -> InputWidgetState {
    let mut fonts = FontSystem::new();
    let mut state =
        InputWidgetState::new_with_limits(&mut fonts, InputLimits::for_kind(InputKind::SearchBar));
    state
        .try_insert_text(&mut fonts, query)
        .expect("short test queries always fit");
    state
}

fn layout_widget<Message: Clone>(
    widget: &Widget<'_, Message>,
    states: &HashMap<u64, WidgetState>,
) -> (TaffyTree<RutterContext>, NodeId) {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, widget, fonts.clone(), states);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(320, 240),
        fonts,
        &RichTextRenderer::default(),
    );
    (taffy, root)
}

fn hit_at(query: &str, mouse: Point, focused: Option<u64>) -> Option<SearchOverlayHit> {
    let widget = search_bar(suggestions());
    let states = focused_states();
    let inputs = HashMap::from([(SEARCH_ID, typed_input(query))]);
    let (taffy, root) = layout_widget(&widget, &states);

    hit_test_search_overlay(SearchOverlayHitInput {
        widget: &widget,
        taffy: &taffy,
        root,
        widget_states: &states,
        input_states: &inputs,
        focused_id: focused,
        mouse,
        viewport: VIEWPORT,
    })
}

#[test]
fn popup_opens_below_the_focused_nonempty_field() {
    // Anchor bottom edge sits at y=40; the first row spans OPTION_HEIGHT.
    let hit = hit_at("star", Point::new(20.0, 50.0), Some(SEARCH_ID));

    assert_eq!(
        hit,
        Some(SearchOverlayHit::Suggestion {
            id: SEARCH_ID,
            index: 2,
        })
    );
}

#[test]
fn hit_rows_map_back_to_original_item_positions() {
    // Query "senhor" ranks only the accented title (original index 1).
    let top_row = Point::new(20.0, 44.0);

    let hit = hit_at("senhor", top_row, Some(SEARCH_ID));

    assert_eq!(
        hit,
        Some(SearchOverlayHit::Suggestion {
            id: SEARCH_ID,
            index: 1,
        })
    );
}

#[test]
fn empty_results_render_a_single_consuming_label_row() {
    // "zzz" matches nothing: the popup shows the no-results label and no row
    // accepts clicks.
    let hit = hit_at("zzz", Point::new(20.0, 50.0), Some(SEARCH_ID));

    assert_eq!(hit, Some(SearchOverlayHit::Consume { id: SEARCH_ID }));
}

#[test]
fn informational_suggestions_consume_clicks_without_activation() {
    let widget = search_bar(non_selectable_suggestions());
    let states = focused_states();
    let inputs = HashMap::from([(SEARCH_ID, typed_input("star"))]);
    let (taffy, root) = layout_widget(&widget, &states);

    let hit = hit_test_search_overlay(SearchOverlayHitInput {
        widget: &widget,
        taffy: &taffy,
        root,
        widget_states: &states,
        input_states: &inputs,
        focused_id: Some(SEARCH_ID),
        mouse: Point::new(20.0, 50.0),
        viewport: VIEWPORT,
    });

    assert_eq!(hit, Some(SearchOverlayHit::Consume { id: SEARCH_ID }));
}

#[test]
fn unfocused_or_dismissed_fields_produce_no_popup() {
    assert_eq!(hit_at("star", Point::new(20.0, 50.0), None), None);

    let widget = search_bar(suggestions());
    let mut states = focused_states();
    if let Some(WidgetState::Search(state)) = states.get_mut(&SEARCH_ID) {
        state.dismissed = true;
    }
    let inputs = HashMap::from([(SEARCH_ID, typed_input("star"))]);
    let (taffy, root) = layout_widget(&widget, &states);

    let overlays = collect_open_search_overlays(
        &widget,
        &taffy,
        root,
        &states,
        &inputs,
        Some(SEARCH_ID),
        VIEWPORT,
    );

    assert!(overlays.is_empty());
}

#[test]
fn popup_draws_pixels_below_the_trigger() {
    let widget = search_bar(suggestions());
    let states = focused_states();
    let inputs = HashMap::from([(SEARCH_ID, typed_input("star"))]);
    let (taffy, root) = layout_widget(&widget, &states);

    let mut surface = surfaces::raster_n32_premul((320, 240)).unwrap();
    surface.canvas().clear(Color::RED);

    draw_search_overlays(SearchOverlayDrawInput {
        canvas: surface.canvas(),
        taffy: &taffy,
        root,
        widget: &widget,
        widget_states: &states,
        input_states: &inputs,
        focused_id: Some(SEARCH_ID),
        mouse: Point::new(0.0, 0.0),
        shows_interaction_effects: true,
        paragraph_renderer: &RichTextRenderer::default(),
        theme: &Theme::light(),
        scale: 1.0,
    });

    let pixel = surface.peek_pixels().unwrap().get_color((10, 50));
    assert_ne!(pixel, Color::RED);
}

#[test]
fn covered_popup_hides_retained_option_hover() {
    let idle = search_popup_option_pixel(None, true);
    let highlighted = search_popup_option_pixel(Some(0), true);
    let masked = search_popup_option_pixel(Some(0), false);

    assert_ne!(highlighted, idle);
    assert_eq!(masked, idle);
}

fn search_popup_option_pixel(
    hovered_option: Option<usize>,
    shows_interaction_effects: bool,
) -> Color {
    let widget = search_bar(suggestions());
    let states = HashMap::from([(
        SEARCH_ID,
        WidgetState::Search(SearchState {
            hovered_option,
            dismissed: false,
        }),
    )]);
    let inputs = HashMap::from([(SEARCH_ID, typed_input("star"))]);
    let (taffy, root) = layout_widget(&widget, &states);
    let mut surface = surfaces::raster_n32_premul((320, 240)).unwrap();
    draw_search_overlays(SearchOverlayDrawInput {
        canvas: surface.canvas(),
        taffy: &taffy,
        root,
        widget: &widget,
        widget_states: &states,
        input_states: &inputs,
        focused_id: Some(SEARCH_ID),
        mouse: Point::new(-10.0, -10.0),
        shows_interaction_effects,
        paragraph_renderer: &RichTextRenderer::default(),
        theme: &Theme::light(),
        scale: 1.0,
    });
    surface.peek_pixels().unwrap().get_color((110, 50))
}

#[test]
fn combo_box_popup_window_keeps_tail_reachable_with_body_anchors_and_contained_hits() {
    use crate::combo_box::ComboBox;
    use crate::widget::InputState;
    let items = [
        "Alpha",
        "Beta",
        "Gamma",
        "Delta",
        "Epsilon",
        "Tail — 東京 القاهرة São Paulo long option",
    ];
    for (width, height) in [(320, 240), (800, 240), (320, 120), (800, 120)] {
        let widget = Widget::combo_box(
            SEARCH_ID,
            "",
            TestMessage::Query,
            ComboBox::new(&items, TestMessage::Picked)
                .with_visible_rows(2)
                .unwrap()
                .with_label("Long localized label — 行き先 القاهرة")
                .with_state(InputState::Error)
                .with_error("Validation feedback remains in its allocated annotation row"),
            Style {
                size: Size::from_lengths(width as f32 - 32.0, 40.0),
                ..Style::default()
            },
        );
        let states = HashMap::from([(
            SEARCH_ID,
            WidgetState::Search(SearchState {
                hovered_option: Some(5),
                dismissed: false,
            }),
        )]);
        let inputs = HashMap::from([(SEARCH_ID, typed_input(""))]);
        let fonts = Rc::new(RefCell::new(FontSystem::new()));
        let mut tree = TaffyTree::new();
        let root = build_taffy_tree(&mut tree, &widget, fonts.clone(), &states);
        compute_layout(
            &mut tree,
            root,
            PhysicalSize::new(width, height),
            fonts,
            &RichTextRenderer::default(),
        );
        let overlays = collect_open_search_overlays(
            &widget,
            &tree,
            root,
            &states,
            &inputs,
            Some(SEARCH_ID),
            (width as f32, height as f32),
        );
        let overlay = &overlays[0];
        assert_eq!(overlay.matches.len(), 6);
        assert_eq!(overlay.anchor.top, 20.0);
        assert_eq!(overlay.anchor.height(), 40.0);
        let popup = search_popup_layout(overlay, (width as f32, height as f32));
        assert!(popup.visible_options <= 2 && popup.visible_options > 0);
        assert!(popup.first_option + popup.visible_options == 6);
        assert!(popup.rect.left >= 0.0 && popup.rect.right <= width as f32);
        assert!(popup.rect.top >= 0.0 && popup.rect.bottom <= height as f32);
        let point = Point::new(
            popup.rect.center_x(),
            popup.rect.bottom - OPTION_HEIGHT / 2.0,
        );
        assert_eq!(
            search_overlay_hit(overlay, point, (width as f32, height as f32)),
            Some(SearchOverlayHit::Suggestion {
                id: SEARCH_ID,
                index: 5
            })
        );
        assert_eq!(
            search_overlay_hit(
                overlay,
                Point::new(popup.rect.right + 1.0, point.y),
                (width as f32, height as f32)
            ),
            None
        );
        for theme in [Theme::light(), Theme::dark()] {
            for scale in [1.0, 1.5, 2.0] {
                let mut surface = surfaces::raster_n32_premul((
                    (width as f32 * scale) as i32,
                    (height as f32 * scale) as i32,
                ))
                .unwrap();
                surface.canvas().clear(Color::RED);
                draw_search_overlays(SearchOverlayDrawInput {
                    canvas: surface.canvas(),
                    taffy: &tree,
                    root,
                    widget: &widget,
                    widget_states: &states,
                    input_states: &inputs,
                    focused_id: Some(SEARCH_ID),
                    mouse: Point::new(-10.0, -10.0),
                    shows_interaction_effects: true,
                    paragraph_renderer: &RichTextRenderer::default(),
                    theme: &theme,
                    scale,
                });
                let pixels = surface.peek_pixels().unwrap();
                assert_ne!(
                    pixels.get_color(((point.x * scale) as i32, (point.y * scale) as i32)),
                    Color::RED
                );
                assert_eq!(
                    pixels.get_color((
                        ((popup.rect.right + 2.0) * scale) as i32,
                        (point.y * scale) as i32
                    )),
                    Color::RED
                );
            }
        }
    }
}

#[test]
fn combo_box_popup_follows_live_scroll_anchor_and_hides_clipped_trigger_without_relayout() {
    use crate::engine::widget_state::ScrollState;
    let field = Widget::combo_box(
        SEARCH_ID,
        "",
        TestMessage::Query,
        crate::combo_box::ComboBox::new(&["One", "Two"], TestMessage::Picked).with_label("Label"),
        Style {
            size: Size::from_lengths(200.0, 40.0),
            flex_shrink: 0.0,
            ..Style::default()
        },
    );
    let widget = Widget::scroll_view(
        Widget::Column {
            children: vec![
                field,
                Widget::Spacer {
                    style: Style {
                        size: Size::from_lengths(200.0, 400.0),
                        flex_shrink: 0.0,
                        ..Style::default()
                    },
                },
            ],
            style: Style {
                flex_shrink: 0.0,
                ..Style::default()
            },
        },
        Style {
            size: Size::from_lengths(200.0, 80.0),
            ..Style::default()
        },
    )
    .with_id(99);
    let mut states = focused_states();
    states.insert(99, WidgetState::Scroll(ScrollState::default()));
    let inputs = HashMap::from([(SEARCH_ID, typed_input(""))]);
    let (tree, root) = layout_widget(&widget, &states);
    let collect = |states: &HashMap<u64, WidgetState>| {
        collect_open_search_overlays(
            &widget,
            &tree,
            root,
            states,
            &inputs,
            Some(SEARCH_ID),
            VIEWPORT,
        )
    };
    let original = collect(&states)[0].anchor;
    states
        .get_mut(&99)
        .unwrap()
        .as_scroll_mut()
        .unwrap()
        .offset_y = 10.0;
    assert_eq!(collect(&states)[0].anchor.top, original.top - 10.0);
    states
        .get_mut(&99)
        .unwrap()
        .as_scroll_mut()
        .unwrap()
        .offset_y = 200.0;
    assert!(collect(&states).is_empty());
    states
        .get_mut(&99)
        .unwrap()
        .as_scroll_mut()
        .unwrap()
        .offset_y = 0.0;
    assert_eq!(collect(&states)[0].anchor, original);
}
