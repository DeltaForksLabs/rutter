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
        font_cache: &mut HashMap::new(),
        theme: &Theme::light(),
        scale: 1.0,
    });

    let pixel = surface.peek_pixels().unwrap().get_color((10, 50));
    assert_ne!(pixel, Color::RED);
}
