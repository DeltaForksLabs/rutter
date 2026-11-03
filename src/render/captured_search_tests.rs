//! Owned search callbacks must preserve native field and popup geometry and pixels.
use super::*;
use crate::engine::widget_state::SearchState;
use crate::layout::{build_layout_tree, compute_layout_with_renderer};
use crate::render::hit_test::{find_input_callbacks, find_input_props_with};
use crate::render::search_overlay::{search_option_rect, search_popup_layout};
use crate::render::select_overlay::collector::collect_open_search_overlays;
use crate::widget::id::validate_widget_id_snapshot;
use crate::{ChangeCallback, SearchMatcher, SearchSuggestions, SearchSuggestionsWith};
use skia_safe::{Color, surfaces};
use taffy::prelude::{Size, Style};
use winit::dpi::PhysicalSize;

const ITEMS: &[&str] = &[
    "Other",
    "São Paulo 東京 القاهرة — a long localized destination",
    "Santos",
    "Tail",
];

fn field(captured: bool, combo: bool, error: bool) -> Widget<'static, ()> {
    let style = Style {
        size: Size::from_lengths(260.0, 44.0),
        flex_shrink: 0.0,
        ..Style::default()
    };
    if combo {
        if captured {
            let mut options = crate::combo_box::ComboBoxWith::new(ITEMS, |_| ())
                .with_label("Long destination label 東京 القاهرة")
                .with_placeholder("Type or browse")
                .with_selected_index(Some(1))
                .with_visible_rows(2)
                .unwrap();
            if error {
                options = options
                    .with_state(InputState::Error)
                    .with_error("Long validation annotation 東京 القاهرة");
            }
            Widget::combo_box_with(42, "sao", |_| (), options, style)
        } else {
            let mut options = crate::combo_box::ComboBox::new(ITEMS, |_| ())
                .with_label("Long destination label 東京 القاهرة")
                .with_placeholder("Type or browse")
                .with_selected_index(Some(1))
                .with_visible_rows(2)
                .unwrap();
            if error {
                options = options
                    .with_state(InputState::Error)
                    .with_error("Long validation annotation 東京 القاهرة");
            }
            Widget::combo_box(42, "sao", |_| (), options, style)
        }
    } else if captured {
        let suggestions = SearchSuggestionsWith::new(
            ITEMS,
            SearchMatcher::Fuzzy,
            3,
            Some(ChangeCallback::new(|_| ())),
        )
        .unwrap();
        Widget::search_bar_with_suggestions_with(
            |_| (),
            None,
            None,
            None,
            "Long search hint 東京 القاهرة",
            suggestions,
            style,
        )
        .with_id(42)
    } else {
        let suggestions =
            SearchSuggestions::new(ITEMS, SearchMatcher::Fuzzy, 3, Some(|_| ())).unwrap();
        Widget::search_bar_with_suggestions(
            |_| (),
            None,
            None,
            None,
            "Long search hint 東京 القاهرة",
            suggestions,
            style,
        )
        .with_id(42)
    }
}

#[test]
fn captured_search_native_geometry_raster_popup_and_lookup_parity() {
    let mut fonts = FontSystem::new();
    let mut swash = SwashCache::new();
    let mut font_cache = HashMap::new();
    let mut text_cache = TextBufferCache::default();
    let renderer = RichTextRenderer::new();
    for combo in [false, true] {
        for state in [
            "normal",
            "focus",
            "disabled",
            "error",
            "open",
            "open_error",
            "empty",
        ] {
            let disabled = state == "disabled";
            let focused = matches!(state, "focus" | "error" | "open" | "open_error" | "empty");
            let open = matches!(state, "open" | "open_error" | "empty");
            let widgets = [
                field(false, combo, matches!(state, "error" | "open_error")).enabled(!disabled),
                field(true, combo, matches!(state, "error" | "open_error")).enabled(!disabled),
            ];
            assert_eq!(
                validate_widget_id_snapshot(&widgets[0]).unwrap(),
                validate_widget_id_snapshot(&widgets[1]).unwrap()
            );
            assert_eq!(widgets[0].resolved_id(&[1]), widgets[1].resolved_id(&[1]));
            for width in [320, 800] {
                for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                    for theme in [Theme::light(), Theme::dark()] {
                        let mut input = InputWidgetState::new(&mut fonts);
                        input.set_text(
                            &mut fonts,
                            if state == "empty" { "no match" } else { "sao" },
                        );
                        let inputs = HashMap::from([(42, input)]);
                        let states = HashMap::from([(
                            42,
                            WidgetState::Search(SearchState {
                                hovered_option: Some(0),
                                dismissed: !open,
                            }),
                        )]);
                        for scale in [1.0_f32, 1.5, 2.0] {
                            let mut expected_layout = None;
                            let mut pixels = Vec::new();
                            for (captured, widget) in widgets.iter().enumerate() {
                                let mut tree = TaffyTree::new();
                                let root = build_layout_tree(&mut tree, widget, direction);
                                compute_layout_with_renderer(
                                    &mut tree,
                                    root,
                                    PhysicalSize::new(width, 160),
                                    &renderer,
                                );
                                let layout = *tree.layout(root).unwrap();
                                if let Some(expected) = expected_layout {
                                    assert_eq!(layout, expected);
                                }
                                expected_layout = Some(layout);
                                let body = crate::layout::field::FieldInsets::for_widget(widget)
                                    .body((layout.size.width, layout.size.height));
                                assert_eq!(body.height(), 44.0);
                                assert!(body.width() > 0.0);
                                assert!(body.top >= 0.0 && body.bottom <= layout.size.height);
                                let props = find_input_props_with(widget, 42);
                                assert_eq!(props.is_some(), !disabled);
                                if let Some((change, submit, password, multiline)) = props {
                                    change.call("draft".into());
                                    assert!(submit.is_none() && !password && !multiline);
                                }
                                assert_eq!(
                                    find_input_callbacks(widget, 42).is_some(),
                                    captured == 0 && !disabled
                                );
                                let overlays = collect_open_search_overlays(
                                    widget,
                                    &tree,
                                    root,
                                    &states,
                                    &inputs,
                                    focused.then_some(42),
                                    (width as f32, 160.0),
                                );
                                assert_eq!(overlays.len(), usize::from(open && !disabled));
                                if let Some(overlay) = overlays.first() {
                                    assert_eq!(overlay.anchor.height(), 44.0);
                                    assert_eq!(overlay.anchor.top, layout.location.y + body.top);
                                    assert_eq!(overlay.combo_box, combo);
                                    assert!(overlay.selectable);
                                    assert_eq!(
                                        overlay.matches.first().map(|m| m.index),
                                        if state == "empty" { None } else { Some(1) }
                                    );
                                    let popup = search_popup_layout(overlay, (width as f32, 160.0));
                                    assert!(
                                        popup.rect.left >= 0.0 && popup.rect.right <= width as f32
                                    );
                                    assert!(popup.rect.top >= 0.0 && popup.rect.bottom <= 160.0);
                                    if state != "empty" {
                                        let row = search_option_rect(popup, 0);
                                        assert!(row.width() > 0.0 && row.height() > 0.0);
                                        assert_eq!(
                                            widget.search_suggestion_id(&[], 1),
                                            Some(crate::widget::resolve_search_suggestion_id(
                                                42, 1
                                            ))
                                        );
                                    }
                                }
                                let mut surface = surfaces::raster_n32_premul((
                                    (width as f32 * scale) as i32,
                                    (160.0 * scale) as i32,
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
                                    focused.then_some(42),
                                    &inputs,
                                    &states,
                                    &mut font_cache,
                                    &mut text_cache,
                                    true,
                                    &theme,
                                    scale,
                                );
                                pixels
                                    .push(surface.peek_pixels().unwrap().bytes().unwrap().to_vec());
                            }
                            assert!(pixels[0].iter().any(|byte| *byte != 0));
                            assert!(
                                pixels[0] == pixels[1],
                                "combo={combo}, state={state}, width={width}, scale={scale}, direction={direction:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn captured_search_configuration_preserves_legacy_literal_copy_and_const_contracts() {
    const SUGGESTIONS: SearchSuggestions<'static, ()> = SearchSuggestions {
        items: ITEMS,
        matcher: SearchMatcher::Fuzzy,
        max_results: 3,
        labels: crate::SearchLabels::ENGLISH,
        on_select: Some(|_| ()),
    };
    const fn copy_legacy(
        value: SearchSuggestions<'static, ()>,
    ) -> [SearchSuggestions<'static, ()>; 2] {
        [value, value]
    }
    const COPIES: [SearchSuggestions<'static, ()>; 2] = copy_legacy(SUGGESTIONS);
    // ComboBox is Clone in this tree (its owned error string prevents Copy).
    // Its original fields still support constant literals and legacy constructors.
    const COMBO: crate::combo_box::ComboBox<'static, ()> = crate::combo_box::ComboBox {
        suggestions: SUGGESTIONS,
        label: "Destination",
        placeholder: "Browse",
        state: InputState::Idle,
        error_msg: None,
        selected_index: Some(1),
    };
    let legacy = Widget::combo_box(42, "", |_| (), COMBO.clone(), Style::default());
    let adapted_combo = Widget::combo_box_with(42, "", |_| (), COMBO, Style::default());
    assert_eq!(
        validate_widget_id_snapshot(&legacy).unwrap(),
        validate_widget_id_snapshot(&adapted_combo).unwrap()
    );
    assert_eq!(COPIES[0].max_results(), COPIES[1].max_results());
    let adapted: SearchSuggestionsWith<'_, ()> = COPIES[0].into();
    assert_eq!(
        adapted.matcher().score("sao", ITEMS[1]),
        COPIES[0].matcher().score("sao", ITEMS[1])
    );
    assert_eq!(adapted.max_results(), 3);
    adapted.on_select.unwrap().call(1);
    assert!(SearchSuggestionsWith::<()>::new(ITEMS, SearchMatcher::Fuzzy, 0, None).is_err());
    let info = SearchSuggestionsWith::<()>::new(ITEMS, SearchMatcher::Fuzzy, 2, None)
        .unwrap()
        .with_labels(crate::SearchLabels::PORTUGUESE);
    assert!(info.on_select.is_none());
    assert_eq!(info.labels, crate::SearchLabels::PORTUGUESE);
    assert!(
        crate::combo_box::ComboBoxWith::new(ITEMS, |_| ())
            .with_visible_rows(0)
            .is_err()
    );
}

#[test]
fn captured_search_accessibility_preserves_full_annotations_and_original_option_ids() {
    use crate::accessibility::{AccessibilityInputs, build_accessibility_update};
    use accesskit::{Action, Invalid, Role};
    for combo in [false, true] {
        for disabled in [false, true] {
            let mut expected = None;
            for captured in [false, true] {
                let widget = field(captured, combo, true).enabled(!disabled);
                let mut tree = TaffyTree::new();
                let root = build_layout_tree(&mut tree, &widget, LayoutDirection::Rtl);
                compute_layout_with_renderer(
                    &mut tree,
                    root,
                    PhysicalSize::new(320, 160),
                    &RichTextRenderer::new(),
                );
                let mut fonts = FontSystem::new();
                let mut input = InputWidgetState::new(&mut fonts);
                input.set_text(&mut fonts, "sao");
                let inputs = HashMap::from([(42, input)]);
                let states = HashMap::from([(
                    42,
                    WidgetState::Search(SearchState {
                        hovered_option: Some(0),
                        dismissed: false,
                    }),
                )]);
                let update = build_accessibility_update(
                    &tree,
                    &widget,
                    root,
                    AccessibilityInputs {
                        input_states: &inputs,
                        widget_states: &states,
                        focused_widget_id: Some(42),
                        viewport: (320.0, 160.0),
                        direction: LayoutDirection::Rtl,
                    },
                );
                let (_, node) = update
                    .nodes
                    .iter()
                    .find(|(_, node)| node.role() == Role::EditableComboBox)
                    .unwrap();
                assert_eq!(node.is_disabled(), disabled);
                assert_eq!(node.supports_action(Action::Focus), !disabled);
                if combo {
                    assert_eq!(node.label(), Some("Long destination label 東京 القاهرة"));
                    assert_eq!(node.invalid(), Some(Invalid::True));
                    assert_eq!(
                        node.description(),
                        Some("Long validation annotation 東京 القاهرة")
                    );
                }
                let option_id = crate::widget::resolve_search_suggestion_id(42, 1);
                let option = update.nodes.iter().find(|(id, _)| id.0 == option_id);
                assert_eq!(option.is_some(), !disabled);
                if let Some((_, option)) = option {
                    assert_eq!(option.role(), Role::ListBoxOption);
                    assert_eq!(option.label(), Some(ITEMS[1]));
                    assert!(option.supports_action(Action::Click));
                    assert_eq!(option.is_selected(), Some(true));
                }
                let summary: Vec<_> = update
                    .nodes
                    .iter()
                    .map(|(id, node)| {
                        (
                            *id,
                            node.role(),
                            node.label().map(str::to_owned),
                            node.description().map(str::to_owned),
                            node.bounds(),
                            node.is_disabled(),
                            node.is_selected(),
                            node.invalid(),
                            node.supports_action(Action::Click),
                        )
                    })
                    .collect();
                if captured {
                    assert_eq!(Some(summary), expected);
                } else {
                    expected = Some(summary);
                }
            }
        }
    }
}

#[test]
fn captured_search_popup_anchor_tracks_live_scroll_offsets_clipping_and_restoration() {
    use crate::engine::widget_state::ScrollState;
    for combo in [false, true] {
        for captured in [false, true] {
            let widget = Widget::scroll_view(
                Widget::Column {
                    children: vec![
                        Widget::Spacer {
                            style: Style {
                                size: Size::from_lengths(260.0, 100.0),
                                flex_shrink: 0.0,
                                ..Style::default()
                            },
                        },
                        field(captured, combo, true),
                        Widget::Spacer {
                            style: Style {
                                size: Size::from_lengths(260.0, 240.0),
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
                    size: Size::from_lengths(320.0, 160.0),
                    ..Style::default()
                },
            )
            .with_id(7);
            let mut tree = TaffyTree::new();
            let root = build_layout_tree(&mut tree, &widget, LayoutDirection::Rtl);
            compute_layout_with_renderer(
                &mut tree,
                root,
                PhysicalSize::new(320, 160),
                &RichTextRenderer::new(),
            );
            let document = tree.children(root).unwrap()[0];
            let content_h = tree.layout(document).unwrap().size.height;
            assert!(content_h > 160.0);
            let mut fonts = FontSystem::new();
            let mut input = InputWidgetState::new(&mut fonts);
            input.set_text(&mut fonts, "sao");
            let inputs = HashMap::from([(42, input)]);
            let mut states = HashMap::from([(
                42,
                WidgetState::Search(SearchState {
                    hovered_option: Some(0),
                    dismissed: false,
                }),
            )]);
            let mut origin = None;
            for offset in [0.0, 40.0, 260.0, 40.0] {
                states.insert(
                    7,
                    WidgetState::Scroll(ScrollState {
                        offset_y: offset,
                        content_height: content_h,
                        viewport_h: 160.0,
                    }),
                );
                let overlays = collect_open_search_overlays(
                    &widget,
                    &tree,
                    root,
                    &states,
                    &inputs,
                    Some(42),
                    (320.0, 160.0),
                );
                if offset == 260.0 {
                    assert!(
                        overlays.is_empty(),
                        "fully clipped triggers cannot expose stale options"
                    );
                    continue;
                }
                let overlay = &overlays[0];
                let top = *origin.get_or_insert(overlay.anchor.top);
                assert!((overlay.anchor.top - (top - offset)).abs() <= 1.0);
                assert_eq!(overlay.matches[0].index, 1);
                let popup = search_popup_layout(overlay, (320.0, 160.0));
                assert!(popup.rect.top >= 0.0 && popup.rect.bottom <= 160.0);
                assert!(popup.visible_options > 0);
            }
        }
    }
}

#[test]
fn captured_search_informational_accessibility_options_have_focus_without_click_actions() {
    use crate::accessibility::{AccessibilityInputs, build_accessibility_update};
    use accesskit::{Action, Role};
    let suggestions =
        SearchSuggestionsWith::<()>::new(ITEMS, SearchMatcher::Fuzzy, 3, None).unwrap();
    let widget = Widget::search_bar_with_suggestions_with(
        |_| (),
        None,
        None,
        None,
        "Search",
        suggestions,
        Style {
            size: Size::from_lengths(260.0, 44.0),
            ..Style::default()
        },
    )
    .with_id(42);
    let mut tree = TaffyTree::new();
    let root = build_layout_tree(&mut tree, &widget, LayoutDirection::Ltr);
    compute_layout_with_renderer(
        &mut tree,
        root,
        PhysicalSize::new(320, 160),
        &RichTextRenderer::new(),
    );
    let mut fonts = FontSystem::new();
    let mut input = InputWidgetState::new(&mut fonts);
    input.set_text(&mut fonts, "sao");
    let inputs = HashMap::from([(42, input)]);
    let states = HashMap::from([(
        42,
        WidgetState::Search(SearchState {
            hovered_option: None,
            dismissed: false,
        }),
    )]);
    let update = build_accessibility_update(
        &tree,
        &widget,
        root,
        AccessibilityInputs {
            input_states: &inputs,
            widget_states: &states,
            focused_widget_id: Some(42),
            viewport: (320.0, 160.0),
            direction: LayoutDirection::Ltr,
        },
    );
    let (id, option) = update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::ListBoxOption)
        .unwrap();
    assert_eq!(id.0, crate::widget::resolve_search_suggestion_id(42, 1));
    assert!(option.supports_action(Action::Focus));
    assert!(!option.supports_action(Action::Click));
}
