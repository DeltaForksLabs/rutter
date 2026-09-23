// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{
    AppLogic, ButtonVariant, DragBadge, DragBadgeIcon, DragEvent, DragPayload, DragPayloadKind,
    DragPhase, DragSource, DropTarget, InputState, LogicalPointerPosition, PointerEvent,
    PointerRegionConfig, RutterRunner, Theme, Widget, WidgetId,
};
use skia_safe::Color;
use taffy::prelude::*;

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

const ITEM_KIND: DragPayloadKind = DragPayloadKind::new(39);
const AMBER: u64 = 1;
const BLUE: u64 = 2;
const EMERALD: u64 = 3;
const CORAL: u64 = 4;
const TEXT_VALUE: u64 = 5;
const AMBER_REGION: u64 = 510;
const BLUE_REGION: u64 = 511;
const TARGET_REGION: u64 = 512;
const CARD_SCROLL_ID: u64 = 514;
const EMERALD_REGION: u64 = 515;
const CORAL_REGION: u64 = 516;
const TEXT_REGION: u64 = 517;
const TEXT_INPUT_ID: u64 = 518;
const AMBER_COLOR: Color = Color::from_rgb(255, 191, 0);
const BLUE_COLOR: Color = Color::from_rgb(31, 93, 190);
const EMERALD_COLOR: Color = Color::from_rgb(10, 105, 70);
const CORAL_COLOR: Color = Color::from_rgb(181, 53, 73);
const TEXT_COLOR: Color = Color::from_rgb(106, 65, 168);

// NPS Yosemite Falls photo (public domain); source and resizing details are in README.md.
const LANDSCAPE_JPEG: &[u8] = include_bytes!("yosemite_falls.jpg");
const LANDSCAPE_BADGE_JPEG: &[u8] = include_bytes!("yosemite_falls_badge.jpg");
const CARD_HEIGHT: f32 = 160.0;

#[derive(Clone, Copy)]
enum BadgeExample {
    Status,
    Move,
    Text,
    Image,
}

#[derive(Clone, Copy)]
struct DemoCard {
    value: u64,
    region_id: u64,
    label: &'static str,
    action_label: &'static str,
    background: Color,
    foreground: Color,
    badge: BadgeExample,
}

const CARDS: [DemoCard; 4] = [
    DemoCard {
        value: AMBER,
        region_id: AMBER_REGION,
        label: "Amber card",
        action_label: "Place Amber",
        background: AMBER_COLOR,
        foreground: Color::BLACK,
        badge: BadgeExample::Status,
    },
    DemoCard {
        value: BLUE,
        region_id: BLUE_REGION,
        label: "Blue card",
        action_label: "Place Blue",
        background: BLUE_COLOR,
        foreground: Color::WHITE,
        badge: BadgeExample::Move,
    },
    DemoCard {
        value: EMERALD,
        region_id: EMERALD_REGION,
        label: "Emerald card",
        action_label: "Place Emerald",
        background: EMERALD_COLOR,
        foreground: Color::WHITE,
        badge: BadgeExample::Text,
    },
    DemoCard {
        value: CORAL,
        region_id: CORAL_REGION,
        label: "Coral card",
        action_label: "Place Coral",
        background: CORAL_COLOR,
        foreground: Color::WHITE,
        badge: BadgeExample::Image,
    },
];

pub struct DragDropDemoState {
    theme: ExampleTheme,
    selected: Option<u64>,
    dragging: Option<u64>,
    over_target: bool,
    last_pointer: Option<LogicalPointerPosition>,
    status: String,
    coral_badge: DragBadge,
    text_draft: String,
    armed_text: Option<String>,
    dragging_text: Option<String>,
    placed_text: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    Pointer(PointerEvent),
    Source(DragEvent),
    Target(DragEvent),
    Place(u64),
    TextChanged(String),
    SelectText,
    Reset,
}

pub struct DragDropDemo;

impl AppLogic for DragDropDemo {
    type State = DragDropDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        DragDropDemoState {
            theme: ExampleTheme::Dark,
            selected: None,
            dragging: None,
            over_target: false,
            last_pointer: None,
            status: "Drag a tile or use Place below.".into(),
            coral_badge: DragBadge::new(CORAL_COLOR, Color::WHITE)
                .with_image(LANDSCAPE_BADGE_JPEG)
                .expect("embedded landscape badge must fit the decoder limits"),
            text_draft: String::new(),
            armed_text: None,
            dragging_text: None,
            placed_text: None,
        }
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Msg> {
        let theme = state.theme.resolve();
        Widget::Column {
            style: page_style(),
            children: vec![
                example_theme_selector(state.theme, Msg::ThemeChanged),
                label("Drag and drop", 26.0),
                label("Drag a tile onto the drop zone.", 14.0),
                drop_zone(state, &theme),
                label(&format!("Status: {}", state.status), 14.0),
                accessible_actions(),
                source_grid(state),
            ],
        }
    }

    fn update(state: &mut Self::State, message: Self::Message, _: &mut Clipboard) {
        apply_demo_message(state, message);
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn apply_demo_message(state: &mut DragDropDemoState, message: Msg) {
    match message {
        Msg::ThemeChanged(theme) => state.theme = theme,
        Msg::Pointer(event) => state.last_pointer = Some(event.position),
        Msg::Source(event) => update_source(state, event),
        Msg::Target(event) => update_target(state, event),
        Msg::Place(item) => place_item(state, item),
        Msg::TextChanged(value) => state.text_draft = value,
        Msg::SelectText => select_text(state),
        Msg::Reset => {
            state.selected = None;
            state.placed_text = None;
            state.status = "Drop zone cleared.".into();
        }
    }
}

fn update_source(state: &mut DragDropDemoState, event: DragEvent) {
    match event.phase {
        DragPhase::Started => {
            state.dragging = Some(event.payload.value());
            state.dragging_text = if event.payload.value() == TEXT_VALUE {
                state.armed_text.clone()
            } else {
                None
            };
            state.status = format!("Dragging {}.", item_label(event.payload.value()));
        }
        DragPhase::Dropped => {
            state.dragging = None;
            if event.target_id.is_none() {
                state.dragging_text = None;
                state.status = "Released outside the drop zone; nothing changed.".into();
            }
        }
        DragPhase::Cancelled(reason) => {
            state.dragging = None;
            state.dragging_text = None;
            state.over_target = false;
            state.status = format!("Drag cancelled ({reason:?}).");
        }
        DragPhase::Entered | DragPhase::Moved | DragPhase::Exited => {}
    }
}

fn update_target(state: &mut DragDropDemoState, event: DragEvent) {
    match event.phase {
        DragPhase::Entered => {
            state.over_target = true;
            state.status = "Release to place the card.".into();
        }
        DragPhase::Exited | DragPhase::Cancelled(_) => {
            state.over_target = false;
            state.status = "Move back over the drop zone to place the card.".into();
        }
        DragPhase::Dropped => {
            state.over_target = false;
            if event.target_id == Some(TARGET_REGION)
                && event.payload.kind() == ITEM_KIND
                && (event.payload.value() != TEXT_VALUE || state.dragging_text.is_some())
            {
                place_item(state, event.payload.value());
            }
            state.dragging_text = None;
        }
        DragPhase::Started | DragPhase::Moved => {}
    }
}

fn place_item(state: &mut DragDropDemoState, item: u64) {
    if item == TEXT_VALUE {
        let Some(text) = state.dragging_text.as_ref().or(state.armed_text.as_ref()) else {
            return;
        };
        state.placed_text = Some(text.clone());
        state.selected = Some(TEXT_VALUE);
        state.status = "Selected text placed in the drop zone.".into();
        return;
    }
    if card_spec(item).is_none() {
        return;
    }
    state.selected = Some(item);
    state.placed_text = None;
    state.status = format!("{} placed in the drop zone.", item_label(item));
}

fn select_text(state: &mut DragDropDemoState) {
    let text = state.text_draft.trim();
    if DragBadge::new(TEXT_COLOR, Color::WHITE)
        .with_text(text)
        .is_err()
    {
        state.armed_text = None;
        state.status = "Enter 1–64 printable UTF-8 bytes, then select text.".into();
        return;
    }
    state.armed_text = Some(text.into());
    state.status = "Text selected. Drag its handle or use Place Text.".into();
}

fn item_label(item: u64) -> &'static str {
    if item == TEXT_VALUE {
        return "selected text";
    }
    card_spec(item)
        .map(|card| card.label)
        .unwrap_or("Unknown card")
}

fn card_badge(item: u64) -> Option<DragBadge> {
    if item == TEXT_VALUE {
        return Some(DragBadge::new(TEXT_COLOR, Color::WHITE));
    }
    card_spec(item).map(|card| DragBadge::new(card.background, card.foreground))
}

fn card_spec(item: u64) -> Option<DemoCard> {
    CARDS.iter().copied().find(|card| card.value == item)
}

fn source_grid<'a>(state: &DragDropDemoState) -> Widget<'a, Msg> {
    Widget::ScrollView {
        id: CARD_SCROLL_ID,
        child: Box::new(Widget::Column {
            children: vec![
                Widget::Row {
                    children: CARDS
                        .iter()
                        .copied()
                        .map(|card| source_card(card, &state.coral_badge))
                        .chain(std::iter::once(text_tile(state)))
                        .collect(),
                    style: Style {
                        flex_wrap: FlexWrap::Wrap,
                        flex_shrink: 0.0,
                        size: Size {
                            width: Dimension::percent(1.0),
                            height: Dimension::auto(),
                        },
                        gap: Size::length(12.0_f32),
                        ..Style::default()
                    },
                },
                label(&pointer_caption(state.last_pointer), 12.0),
            ],
            style: Style {
                flex_shrink: 0.0,
                size: Size {
                    width: Dimension::percent(1.0),
                    height: Dimension::auto(),
                },
                gap: Size::length(8.0_f32),
                ..Style::default()
            },
        }),
        style: Style {
            flex_grow: 1.0,
            flex_shrink: 1.0,
            align_items: Some(AlignItems::FlexStart),
            min_size: Size {
                width: Dimension::auto(),
                height: Dimension::length(90.0),
            },
            max_size: Size {
                width: Dimension::length(680.0),
                height: Dimension::length(360.0),
            },
            ..responsive_width(680.0, Dimension::auto())
        },
    }
}

fn source_card<'a>(card: DemoCard, coral_badge: &DragBadge) -> Widget<'a, Msg> {
    let appearance = DragBadge::new(card.background, card.foreground);
    let badge = match card.badge {
        BadgeExample::Status => appearance,
        BadgeExample::Move => appearance.with_icon(DragBadgeIcon::Move),
        BadgeExample::Text => appearance
            .with_text("EmeraldLeaf")
            .expect("demo badge text must be printable and within the byte limit"),
        BadgeExample::Image => coral_badge.clone(),
    };
    Widget::pointer_region(
        WidgetId::manual(card.region_id).expect("demo source IDs must use the manual namespace"),
        source_card_face(card),
        PointerRegionConfig::new(Msg::Pointer)
            .with_drag_source(DragSource {
                payload: DragPayload::new(ITEM_KIND, card.value),
                on_drag: Msg::Source,
            })
            .with_drag_badge(badge),
        grid_tile_style(),
    )
}

fn grid_tile_style() -> Style {
    Style {
        flex_basis: Dimension::length(180.0),
        flex_grow: 1.0,
        min_size: Size {
            width: Dimension::length(128.0),
            height: Dimension::length(CARD_HEIGHT),
        },
        max_size: Size {
            width: Dimension::length(320.0),
            height: Dimension::length(CARD_HEIGHT),
        },
        ..responsive_width(320.0, Dimension::length(CARD_HEIGHT))
    }
}

fn text_tile<'a>(state: &DragDropDemoState) -> Widget<'a, Msg> {
    let drag_source = match state.armed_text.as_deref() {
        Some(text) => Widget::pointer_region(
            WidgetId::manual(TEXT_REGION).expect("demo text drag ID must be manual"),
            colored_card("Drag selected text", TEXT_COLOR, Color::WHITE, 38.0),
            PointerRegionConfig::new(Msg::Pointer)
                .with_drag_source(DragSource {
                    payload: DragPayload::new(ITEM_KIND, TEXT_VALUE),
                    on_drag: Msg::Source,
                })
                .with_drag_badge(
                    DragBadge::new(TEXT_COLOR, Color::WHITE)
                        .with_text(text)
                        .expect("selected text was validated before arming the drag source"),
                ),
            responsive_width(300.0, Dimension::length(38.0)),
        ),
        None => label("Type text, then select it to drag.", 12.0),
    };
    Widget::Container {
        child: Box::new(Widget::Column {
            children: vec![
                Widget::TextInput {
                    id: TEXT_INPUT_ID,
                    on_change: Msg::TextChanged,
                    on_submit: Some(Msg::SelectText),
                    style: responsive_width(300.0, Dimension::length(40.0)),
                    label: "Text to drag",
                    placeholder: "Enter text",
                    state: InputState::Idle,
                    error_msg: None,
                    is_password: false,
                },
                action_button("Select text", Msg::SelectText),
                drag_source,
            ],
            style: Style {
                size: Size::percent(1.0_f32),
                align_items: Some(AlignItems::Center),
                justify_content: Some(JustifyContent::Center),
                gap: Size::length(6.0_f32),
                ..Style::default()
            },
        }),
        style: grid_tile_style(),
        color: Some(TEXT_COLOR),
        radius: 10.0,
    }
}

fn source_card_face<'a>(card: DemoCard) -> Widget<'a, Msg> {
    let mut children = Vec::new();
    if matches!(card.badge, BadgeExample::Image) {
        children.push(Widget::Image {
            data: LANDSCAPE_JPEG,
            style: Style {
                aspect_ratio: Some(464.0 / 360.0),
                ..responsive_width(144.0, Dimension::auto())
            },
            radius: 8.0,
        });
    }
    children.push(Widget::Text {
        content: card.label.into(),
        style: Style::default(),
        color: Some(card.foreground),
        size: 16.0,
    });
    Widget::Container {
        child: Box::new(Widget::Column {
            children,
            style: Style {
                size: Size::percent(1.0_f32),
                align_items: Some(AlignItems::Center),
                justify_content: Some(JustifyContent::Center),
                gap: Size::length(6.0_f32),
                ..Style::default()
            },
        }),
        style: Style {
            size: Size::percent(1.0_f32),
            padding: Rect::length(8.0_f32),
            ..Style::default()
        },
        color: Some(card.background),
        radius: 10.0,
    }
}

fn drop_zone<'a>(state: &DragDropDemoState, theme: &Theme) -> Widget<'a, Msg> {
    let preview = state.dragging.filter(|_| state.over_target);
    let shown_card = preview.or(state.selected);
    let title = if preview == Some(TEXT_VALUE) {
        format!(
            "Release text here: {}",
            state.dragging_text.as_deref().unwrap_or("")
        )
    } else if state.selected == Some(TEXT_VALUE) && preview.is_none() {
        format!("Drop zone: {}", state.placed_text.as_deref().unwrap_or(""))
    } else if let Some(item) = preview {
        format!("Release {} here!", item_label(item))
    } else if let Some(item) = state.selected {
        format!("Drop zone: {}", item_label(item))
    } else {
        "Drop zone: empty".into()
    };
    let colors = shown_card.and_then(card_badge);
    let (background, foreground) = colors
        .map(|badge| (badge.background, badge.foreground))
        .unwrap_or((Theme::alpha(theme.on_surface, 65), theme.on_surface));
    Widget::pointer_region(
        WidgetId::manual(TARGET_REGION).expect("demo target ID must use the manual namespace"),
        if shown_card == Some(CORAL) {
            landscape_drop_zone(&title, background, foreground)
        } else {
            colored_card(&title, background, foreground, 96.0)
        },
        PointerRegionConfig::new(Msg::Pointer).with_drop_target(DropTarget {
            accepted_kind: ITEM_KIND,
            on_drag: Msg::Target,
        }),
        responsive_width(460.0, Dimension::length(96.0)),
    )
}

fn landscape_drop_zone<'a>(title: &str, background: Color, foreground: Color) -> Widget<'a, Msg> {
    Widget::Container {
        child: Box::new(Widget::Row {
            children: vec![
                Widget::Image {
                    data: LANDSCAPE_JPEG,
                    style: Style {
                        size: Size::from_lengths(93.0, 72.0),
                        flex_shrink: 1.0,
                        ..Style::default()
                    },
                    radius: 8.0,
                },
                Widget::Text {
                    content: title.into(),
                    style: Style {
                        flex_grow: 1.0,
                        flex_basis: Dimension::length(0.0),
                        ..Style::default()
                    },
                    color: Some(foreground),
                    size: 16.0,
                },
            ],
            style: Style {
                align_items: Some(AlignItems::Center),
                size: Size::percent(1.0_f32),
                gap: Size::length(8.0_f32),
                ..Style::default()
            },
        }),
        style: Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(96.0),
            },
            padding: Rect::length(8.0_f32),
            ..Style::default()
        },
        color: Some(background),
        radius: 10.0,
    }
}

fn colored_card<'a>(
    title: &str,
    background: skia_safe::Color,
    foreground: skia_safe::Color,
    height: f32,
) -> Widget<'a, Msg> {
    Widget::Container {
        child: Box::new(Widget::Text {
            content: title.into(),
            style: Style::default(),
            color: Some(foreground),
            size: 18.0,
        }),
        style: Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(height),
            },
            align_items: Some(AlignItems::Center),
            justify_content: Some(JustifyContent::Center),
            ..Style::default()
        },
        color: Some(background),
        radius: 10.0,
    }
}

fn accessible_actions<'a>() -> Widget<'a, Msg> {
    Widget::Row {
        children: CARDS
            .iter()
            .map(|card| action_button(card.action_label, Msg::Place(card.value)))
            .chain(std::iter::once(action_button(
                "Place Text",
                Msg::Place(TEXT_VALUE),
            )))
            .chain(std::iter::once(action_button("Clear", Msg::Reset)))
            .collect(),
        style: Style {
            flex_wrap: FlexWrap::Wrap,
            gap: Size::length(6.0_f32),
            ..Style::default()
        },
    }
}

fn action_button<'a>(title: &'static str, message: Msg) -> Widget<'a, Msg> {
    Widget::Button {
        text: title,
        on_press: message,
        style: Style {
            size: Size::from_lengths(96.0, 32.0),
            ..Style::default()
        },
        color: None,
        variant: ButtonVariant::Primary,
    }
}

fn pointer_caption(position: Option<LogicalPointerPosition>) -> String {
    match position {
        Some(position) => format!(
            "Last pointer (logical): {:.0}, {:.0}",
            position.x(),
            position.y()
        ),
        None => "Last pointer (logical): none yet".into(),
    }
}

fn label<'a>(content: &str, size: f32) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.into(),
        style: responsive_width(560.0, Dimension::auto()),
        color: None,
        size,
    }
}

fn page_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::FlexStart),
        size: Size::percent(1.0_f32),
        padding: Rect::length(12.0_f32),
        gap: Size::length(8.0_f32),
        ..Style::default()
    }
}

pub fn run() {
    RutterRunner::<DragDropDemo>::run();
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use super::*;
    use rutter::{
        DragCancelReason, PointerModifiers, WidgetIdSnapshot,
        engine::widget_state::{ScrollState, WidgetState},
        layout::{build_taffy_tree, compute_layout},
        render::{
            RichTextRenderer,
            hit_test::{HitResult, hit_test},
        },
    };
    use taffy::prelude::TaffyTree;
    use winit::dpi::PhysicalSize;

    #[test]
    fn landscape_card_has_distinct_view_and_badge_images() {
        let photo =
            skia_safe::Image::from_encoded(skia_safe::Data::new_copy(LANDSCAPE_JPEG)).unwrap();
        let badge = skia_safe::Image::from_encoded(skia_safe::Data::new_copy(LANDSCAPE_BADGE_JPEG))
            .unwrap();
        assert!(photo.width() > badge.width());
        assert!(photo.height() > badge.height());
        assert!(badge.width() <= 128 && badge.height() <= 128);
        let demo_state = state();
        let Widget::PointerRegion { child, .. } = source_card(CARDS[3], &demo_state.coral_badge)
        else {
            panic!("photo must be a drag source");
        };
        let Widget::Container { child, .. } = *child else {
            panic!("photo needs a card face")
        };
        let Widget::Column { children, .. } = *child else {
            panic!("photo needs a caption")
        };
        assert!(matches!(&children[0], Widget::Image { data, .. } if *data == LANDSCAPE_JPEG));
        let mut selected = state();
        apply_demo_message(&mut selected, Msg::Place(CORAL));
        let Widget::PointerRegion { child, .. } = drop_zone(&selected, &selected.theme.resolve())
        else {
            panic!("drop zone must remain a pointer region")
        };
        let Widget::Container { child, .. } = *child else {
            panic!("drop zone needs a face")
        };
        assert!(
            matches!(*child, Widget::Row { children, .. } if matches!(&children[0], Widget::Image { data, .. } if *data == LANDSCAPE_JPEG))
        );
    }

    fn event(phase: DragPhase, item: u64, target_id: Option<u64>) -> DragEvent {
        DragEvent {
            phase,
            payload: DragPayload::new(ITEM_KIND, item),
            source_id: AMBER_REGION,
            target_id,
            position: LogicalPointerPosition::new(40.0, 80.0),
            modifiers: PointerModifiers::default(),
        }
    }

    fn state() -> DragDropDemoState {
        DragDropDemo::new(&mut FontSystem::new())
    }

    fn drop_zone_color(state: &DragDropDemoState) -> Color {
        let zone = drop_zone(state, &state.theme.resolve());
        let Widget::PointerRegion { child, .. } = zone else {
            panic!("demo drop zone must be a pointer region");
        };
        let Widget::Container {
            color: Some(color), ..
        } = *child
        else {
            panic!("demo drop zone must have a visible color");
        };
        color
    }

    fn drop_zone_shows_landscape(state: &DragDropDemoState) -> bool {
        let Widget::PointerRegion { child, .. } = drop_zone(state, &state.theme.resolve()) else {
            panic!("drop zone must be a pointer region");
        };
        let Widget::Container { child, .. } = *child else {
            panic!("drop zone must have a card face");
        };
        matches!(*child, Widget::Row { children, .. }
            if matches!(&children[0], Widget::Image { data, .. } if *data == LANDSCAPE_JPEG))
    }

    fn drop_zone_title(state: &DragDropDemoState) -> String {
        let Widget::PointerRegion { child, .. } = drop_zone(state, &state.theme.resolve()) else {
            panic!("drop zone must be a pointer region");
        };
        let Widget::Container { child, .. } = *child else {
            panic!("drop zone needs a card face");
        };
        let Widget::Text { content, .. } = *child else {
            panic!("text drop zone needs a text label");
        };
        content
    }

    #[test]
    fn typed_text_must_be_selected_and_drag_preserves_its_snapshot() {
        let mut state = state();
        apply_demo_message(&mut state, Msg::Place(TEXT_VALUE));
        assert_eq!(state.selected, None);
        apply_demo_message(&mut state, Msg::TextChanged("x".repeat(65)));
        apply_demo_message(&mut state, Msg::SelectText);
        assert!(state.armed_text.is_none());
        apply_demo_message(&mut state, Msg::TextChanged("unsafe\u{202e}name".into()));
        apply_demo_message(&mut state, Msg::SelectText);
        assert!(state.armed_text.is_none());
        apply_demo_message(&mut state, Msg::TextChanged(" ".into()));
        apply_demo_message(&mut state, Msg::SelectText);
        assert!(state.armed_text.is_none());

        apply_demo_message(&mut state, Msg::TextChanged("  River  ".into()));
        apply_demo_message(&mut state, Msg::SelectText);
        assert_eq!(state.armed_text.as_deref(), Some("River"));
        update_source(&mut state, event(DragPhase::Started, TEXT_VALUE, None));
        apply_demo_message(&mut state, Msg::TextChanged("Forest".into()));
        apply_demo_message(&mut state, Msg::SelectText);
        update_target(
            &mut state,
            event(DragPhase::Entered, TEXT_VALUE, Some(TARGET_REGION)),
        );
        assert_eq!(drop_zone_title(&state), "Release text here: River");
        update_source(
            &mut state,
            event(DragPhase::Dropped, TEXT_VALUE, Some(TARGET_REGION)),
        );
        update_target(
            &mut state,
            event(DragPhase::Dropped, TEXT_VALUE, Some(TARGET_REGION)),
        );
        assert_eq!(state.placed_text.as_deref(), Some("River"));
        assert_eq!(drop_zone_title(&state), "Drop zone: River");
        apply_demo_message(&mut state, Msg::Place(TEXT_VALUE));
        assert_eq!(state.placed_text.as_deref(), Some("Forest"));
        apply_demo_message(&mut state, Msg::Place(AMBER));
        assert!(state.placed_text.is_none());
    }

    #[test]
    fn text_input_and_select_button_do_not_get_swallowed_by_drag_region() {
        let mut transition_state = state();
        let before = WidgetIdSnapshot::capture(&DragDropDemo::view(&mut transition_state)).unwrap();
        apply_demo_message(&mut transition_state, Msg::TextChanged("Waterfall".into()));
        apply_demo_message(&mut transition_state, Msg::SelectText);
        let after = WidgetIdSnapshot::capture(&DragDropDemo::view(&mut transition_state)).unwrap();
        before.validate_transition_to(&after).unwrap();
        let Widget::Container { child, .. } = text_tile(&state()) else {
            panic!("text entry must be an independent tile");
        };
        let Widget::Column { children, .. } = *child else {
            panic!("text tile must expose input and selection separately");
        };
        assert!(
            matches!(&children[0], Widget::TextInput { on_change, on_submit: Some(Msg::SelectText), .. }
            if matches!(on_change("River".into()), Msg::TextChanged(value) if value == "River"))
        );
        assert!(matches!(&children[2], Widget::Text { .. }));
        let mut state = state();
        apply_demo_message(&mut state, Msg::TextChanged("Waterfall".into()));
        apply_demo_message(&mut state, Msg::SelectText);
        let view = DragDropDemo::view(&mut state);
        let mut states =
            HashMap::from([(CARD_SCROLL_ID, WidgetState::Scroll(ScrollState::default()))]);
        let fonts = Rc::new(RefCell::new(FontSystem::new()));
        let mut layout = TaffyTree::new();
        let root = build_taffy_tree(&mut layout, &view, fonts.clone(), &states);
        compute_layout(
            &mut layout,
            root,
            PhysicalSize::new(480, 600),
            fonts,
            &RichTextRenderer::default(),
        );
        let scroll_node = layout.children(root).unwrap()[6];
        let content_node = layout.children(scroll_node).unwrap()[0];
        let grid_node = layout.children(content_node).unwrap()[0];
        let tile_node = *layout.children(grid_node).unwrap().last().unwrap();
        let column_node = layout.children(tile_node).unwrap()[0];
        let controls = layout.children(column_node).unwrap();
        let scroll = layout.layout(scroll_node).unwrap();
        let content = layout.layout(content_node).unwrap();
        let grid = layout.layout(grid_node).unwrap();
        let tile = layout.layout(tile_node).unwrap();
        let column = layout.layout(column_node).unwrap();
        let offset = content.size.height - scroll.size.height;
        states.insert(
            CARD_SCROLL_ID,
            WidgetState::Scroll(ScrollState {
                offset_y: offset,
                content_height: content.size.height,
                viewport_h: scroll.size.height,
            }),
        );
        let origin = skia_safe::Point::new(
            scroll.location.x
                + content.location.x
                + grid.location.x
                + tile.location.x
                + column.location.x,
            scroll.location.y
                + content.location.y
                + grid.location.y
                + tile.location.y
                + column.location.y
                - offset,
        );
        let control_hit = |index: usize| {
            let position = layout.layout(controls[index]).unwrap();
            let point = skia_safe::Point::new(
                origin.x + position.location.x + position.size.width * 0.5,
                origin.y + position.location.y + position.size.height * 0.5,
            );
            hit_test(
                &view,
                &layout,
                root,
                point,
                skia_safe::Point::default(),
                &states,
            )
        };
        assert!(matches!(control_hit(0), Some(HitResult::InputFocus { .. })));
        assert!(matches!(
            control_hit(1),
            Some(HitResult::Message {
                msg: Msg::SelectText,
                ..
            })
        ));
        assert!(matches!(
            control_hit(2),
            Some(HitResult::PointerRegion(TEXT_REGION))
        ));
    }

    #[test]
    fn landscape_preview_and_placed_photo_follow_the_same_drag_payload() {
        let mut state = state();
        assert!(!drop_zone_shows_landscape(&state));
        update_source(&mut state, event(DragPhase::Started, CORAL, None));
        update_target(
            &mut state,
            event(DragPhase::Entered, CORAL, Some(TARGET_REGION)),
        );
        assert!(drop_zone_shows_landscape(&state));
        update_target(
            &mut state,
            event(DragPhase::Exited, CORAL, Some(TARGET_REGION)),
        );
        assert!(!drop_zone_shows_landscape(&state));
        update_target(
            &mut state,
            event(DragPhase::Dropped, CORAL, Some(TARGET_REGION)),
        );
        assert!(drop_zone_shows_landscape(&state));
        apply_demo_message(&mut state, Msg::Reset);
        assert!(!drop_zone_shows_landscape(&state));
    }

    #[test]
    fn demo_declares_stable_region_ids_and_an_accessible_alternative() {
        let mut state = state();
        let view = DragDropDemo::view(&mut state);
        WidgetIdSnapshot::capture(&view).unwrap();
        let Widget::Column { children, .. } = view else {
            panic!("drag/drop demo must have a column root");
        };
        assert!(
            matches!(&children[3], Widget::PointerRegion { id, config, .. }
            if id.get() == TARGET_REGION
                && config.drop_target.as_ref().is_some_and(|target| target.accepted_kind == ITEM_KIND))
        );
        let Widget::ScrollView { id, child, .. } = &children[6] else {
            panic!("demo cards must be in a scroll view");
        };
        assert_eq!(*id, CARD_SCROLL_ID);
        let Widget::Column {
            children: content, ..
        } = child.as_ref()
        else {
            panic!("scroll content must contain the grid and keyboard actions");
        };
        let Widget::Row {
            children: sources, ..
        } = &content[0]
        else {
            panic!("demo sources must form a wrapping grid");
        };
        assert_eq!(sources.len(), CARDS.len() + 1);
        for (source, card) in sources.iter().zip(CARDS.iter()) {
            assert!(matches!(source, Widget::PointerRegion { id, config, .. }
                    if id.get() == card.region_id
                        && config.drag_source.as_ref().is_some_and(|source|
                            source.payload == DragPayload::new(ITEM_KIND, card.value))
                        && config.drag_badge.as_ref().map(|badge| (badge.background, badge.foreground))
                            == card_badge(card.value).as_ref().map(|badge| (badge.background, badge.foreground))));
        }
        assert!(matches!(sources.last(), Some(Widget::Container { .. })));
        let Widget::Row { children, .. } = &children[5] else {
            panic!("keyboard-equivalent actions must be accessible buttons");
        };
        assert_eq!(children.len(), CARDS.len() + 2);
        for (button, card) in children.iter().zip(CARDS.iter()) {
            assert!(
                matches!(button, Widget::Button { on_press: Msg::Place(value), text, .. }
                if *value == card.value && *text == card.action_label)
            );
        }
        assert!(matches!(
            &children[CARDS.len()],
            Widget::Button {
                on_press: Msg::Place(TEXT_VALUE),
                ..
            }
        ));
        assert!(matches!(
            children.last(),
            Some(Widget::Button {
                on_press: Msg::Reset,
                ..
            })
        ));
    }

    #[test]
    fn grid_cards_are_separate_pointer_targets_even_after_scrolling() {
        for (width, height) in [(480, 600), (300, 600), (300, 440), (240, 440)] {
            assert_grid_hits_at_size(width, height);
        }
    }

    fn assert_grid_hits_at_size(width: u32, height: u32) {
        let mut state = state();
        if width <= 240 {
            state.status = "Enter 1–64 printable UTF-8 bytes, then select text.".into();
            state.selected = Some(CORAL);
        }
        let view = DragDropDemo::view(&mut state);
        let mut states =
            HashMap::from([(CARD_SCROLL_ID, WidgetState::Scroll(ScrollState::default()))]);
        let fonts = Rc::new(RefCell::new(FontSystem::new()));
        let mut layout = TaffyTree::new();
        let root = build_taffy_tree(&mut layout, &view, fonts.clone(), &states);
        compute_layout(
            &mut layout,
            root,
            PhysicalSize::new(width, height),
            fonts,
            &RichTextRenderer::default(),
        );
        let children = layout.children(root).unwrap();
        let target = layout.layout(children[3]).unwrap();
        let scroll = layout.layout(children[6]).unwrap();
        let content_node = layout.children(children[6]).unwrap()[0];
        let content = layout.layout(content_node).unwrap();
        let grid_node = layout.children(content_node).unwrap()[0];
        let grid = layout.layout(grid_node).unwrap();
        let cards = layout.children(grid_node).unwrap();

        assert!(target.location.y + target.size.height < scroll.location.y);
        assert!(matches!(
            hit_test(
                &view,
                &layout,
                root,
                skia_safe::Point::new(
                    target.location.x + target.size.width * 0.5,
                    target.location.y + target.size.height * 0.5,
                ),
                skia_safe::Point::default(),
                &states,
            ),
            Some(HitResult::PointerRegion(TARGET_REGION))
        ));
        assert!(scroll.size.height >= 90.0);
        assert!(
            scroll.location.y + scroll.size.height <= height as f32,
            "grid outside {width}×{height} viewport: top={}, height={}",
            scroll.location.y,
            scroll.size.height
        );
        assert!(content.size.height > scroll.size.height, "grid must scroll");
        let first = layout.layout(cards[0]).unwrap();
        let second = layout.layout(cards[1]).unwrap();
        if width == 480 {
            assert!(first.location.x < second.location.x);
        } else {
            assert!(first.location.y < second.location.y);
        }
        for (card_node, card) in cards.iter().zip(CARDS) {
            let geometry = layout.layout(*card_node).unwrap();
            let offset = (content.location.y
                + grid.location.y
                + geometry.location.y
                + geometry.size.height * 0.5
                - scroll.size.height * 0.5)
                .clamp(0.0, content.size.height - scroll.size.height);
            states.insert(
                CARD_SCROLL_ID,
                WidgetState::Scroll(ScrollState {
                    offset_y: offset,
                    content_height: content.size.height,
                    viewport_h: scroll.size.height,
                }),
            );
            let point = skia_safe::Point::new(
                scroll.location.x
                    + content.location.x
                    + grid.location.x
                    + geometry.location.x
                    + geometry.size.width * 0.5,
                scroll.location.y
                    + content.location.y
                    + grid.location.y
                    + geometry.location.y
                    + geometry.size.height * 0.5
                    - offset,
            );
            assert!(
                matches!(hit_test(&view, &layout, root, point, skia_safe::Point::new(0.0, 0.0), &states), Some(HitResult::PointerRegion(id)) if id == card.region_id),
                "card {} did not receive its own hit",
                card.label
            );
        }
        let actions_node = children[5];
        let clear_node = *layout.children(actions_node).unwrap().last().unwrap();
        let actions = layout.layout(actions_node).unwrap();
        let clear = layout.layout(clear_node).unwrap();
        let clear_position = skia_safe::Point::new(
            actions.location.x + clear.location.x + clear.size.width * 0.5,
            actions.location.y + clear.location.y + clear.size.height * 0.5,
        );
        assert!(
            matches!(
                hit_test(
                    &view,
                    &layout,
                    root,
                    clear_position,
                    skia_safe::Point::default(),
                    &states
                ),
                Some(HitResult::Message {
                    msg: Msg::Reset,
                    ..
                })
            ),
            "fixed Clear button must remain clickable at {width}×{height}"
        );
    }

    #[test]
    fn dragging_into_target_places_payload_and_exiting_does_not() {
        let mut state = state();
        update_source(&mut state, event(DragPhase::Started, AMBER, None));
        update_target(
            &mut state,
            event(DragPhase::Entered, AMBER, Some(TARGET_REGION)),
        );
        assert!(state.over_target);
        update_target(
            &mut state,
            event(DragPhase::Exited, AMBER, Some(TARGET_REGION)),
        );
        assert!(!state.over_target);
        update_source(&mut state, event(DragPhase::Dropped, AMBER, None));
        assert_eq!(state.selected, None);

        update_target(
            &mut state,
            event(DragPhase::Dropped, BLUE, Some(TARGET_REGION)),
        );
        assert_eq!(state.selected, Some(BLUE));
        assert_eq!(state.status, "Blue card placed in the drop zone.");
    }

    #[test]
    fn cards_and_drop_zone_use_the_named_card_colors() {
        let mut state = state();
        for card in CARDS {
            assert_ne!(drop_zone_color(&state), card.background);
            let Widget::PointerRegion { child, .. } = source_card(card, &state.coral_badge) else {
                panic!("demo card must be a pointer region");
            };
            assert!(
                matches!(*child, Widget::Container { color: Some(color), .. } if color == card.background)
            );
        }

        apply_demo_message(
            &mut state,
            Msg::Source(event(DragPhase::Started, AMBER, None)),
        );
        apply_demo_message(
            &mut state,
            Msg::Target(event(DragPhase::Entered, AMBER, Some(TARGET_REGION))),
        );
        assert_eq!(drop_zone_color(&state), AMBER_COLOR);
        apply_demo_message(
            &mut state,
            Msg::Source(event(DragPhase::Dropped, AMBER, Some(TARGET_REGION))),
        );
        apply_demo_message(
            &mut state,
            Msg::Target(event(DragPhase::Dropped, AMBER, Some(TARGET_REGION))),
        );
        assert_eq!(drop_zone_color(&state), AMBER_COLOR);

        apply_demo_message(
            &mut state,
            Msg::Source(event(DragPhase::Started, BLUE, None)),
        );
        apply_demo_message(
            &mut state,
            Msg::Target(event(DragPhase::Entered, BLUE, Some(TARGET_REGION))),
        );
        assert_eq!(drop_zone_color(&state), BLUE_COLOR);
        apply_demo_message(
            &mut state,
            Msg::Source(event(
                DragPhase::Cancelled(DragCancelReason::FocusLost),
                BLUE,
                None,
            )),
        );
        assert_eq!(drop_zone_color(&state), AMBER_COLOR);
        apply_demo_message(&mut state, Msg::Place(BLUE));
        assert_eq!(drop_zone_color(&state), BLUE_COLOR);
        for card in CARDS {
            apply_demo_message(&mut state, Msg::Place(card.value));
            assert_eq!(drop_zone_color(&state), card.background);
        }
        apply_demo_message(&mut state, Msg::Reset);
        assert_ne!(drop_zone_color(&state), TEXT_COLOR);
    }

    #[test]
    fn invalid_payloads_and_cancellation_never_place_a_card() {
        let mut state = state();
        let mut invalid = event(DragPhase::Dropped, AMBER, Some(TARGET_REGION));
        invalid.payload = DragPayload::new(DragPayloadKind::new(7), AMBER);
        update_target(&mut state, invalid);
        update_target(
            &mut state,
            event(DragPhase::Dropped, 999, Some(TARGET_REGION)),
        );
        assert_eq!(state.selected, None);

        update_target(
            &mut state,
            event(DragPhase::Entered, AMBER, Some(TARGET_REGION)),
        );
        update_source(
            &mut state,
            event(
                DragPhase::Cancelled(DragCancelReason::FocusLost),
                AMBER,
                None,
            ),
        );
        assert!(!state.over_target);
        assert_eq!(state.selected, None);
    }

    #[test]
    fn accessible_buttons_place_and_clear_the_same_items() {
        let mut state = state();
        for card in CARDS {
            apply_demo_message(&mut state, Msg::Place(card.value));
            assert_eq!(state.selected, Some(card.value));
        }
        apply_demo_message(&mut state, Msg::Reset);
        assert_eq!(state.status, "Drop zone cleared.");
        apply_demo_message(&mut state, Msg::Place(999));
        assert_eq!(state.selected, None);
        apply_demo_message(&mut state, Msg::Place(BLUE));
        assert_eq!(state.selected, Some(BLUE));
        apply_demo_message(
            &mut state,
            Msg::Pointer(PointerEvent {
                phase: rutter::PointerPhase::Moved,
                position: LogicalPointerPosition::new(40.0, 80.0),
                modifiers: PointerModifiers::default(),
            }),
        );
        assert_eq!(
            pointer_caption(state.last_pointer),
            "Last pointer (logical): 40, 80"
        );
    }
}
