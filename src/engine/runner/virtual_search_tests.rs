//! Window-level search portals from reconstructed keyed collection items.
use super::*;
use crate::accessibility::{AccessibilityInputs, build_accessibility_update};
use crate::engine::test_surface::{MemoryEventLoop, MemoryWindow};
use crate::render::search_overlay::{
    SearchOverlayHit, SearchOverlayHitInput, hit_test_search_overlay, search_option_rect,
    search_popup_layout,
};
use crate::render::select_overlay::collector::collect_open_search_overlays;
use crate::widget::{KeyedVirtualItems, VirtualItemKey, resolve_search_suggestion_id};
use crate::{ChangeCallback, SearchMatcher, SearchSuggestions, SearchSuggestionsWith, Widget};
use accesskit::{Action, ActionHandler, ActionRequest, NodeId, Role, TreeId};
use taffy::prelude::{Size, Style};
use winit::dpi::PhysicalSize;
use winit::keyboard::{KeyCode, KeyLocation, PhysicalKey};

mod live_offsets;
mod raster;

const VIEWPORT: PhysicalSize<u32> = PhysicalSize::new(320, 360);
const ITEMS: &[&str] = &[
    "Other",
    "São Paulo 東京 القاهرة — destination",
    "São Bento",
    "Tail",
];
const COLLECTION: u64 = 90;
const FIELD: u64 = 41;

#[derive(Clone, Copy, Debug)]
enum Family {
    List,
    Grid,
    Carousel,
}
const FAMILIES: [Family; 3] = [Family::List, Family::Grid, Family::Carousel];

#[derive(Clone, Debug, PartialEq)]
enum Message {
    Draft(u64, String),
    Pick(u64, usize),
    Card(usize),
}

fn item<'a, const COMBO: bool, const CAPTURED: bool, const EPOCH: u64>(
    index: usize,
) -> Option<Widget<'a, Message>> {
    let style = Style {
        size: Size {
            width: taffy::prelude::percent(1.0),
            height: taffy::prelude::length(44.0),
        },
        flex_shrink: 0.0,
        ..Style::default()
    };
    // Owned callbacks differ by view epoch while the keyed structure stays fixed.
    let epoch = EPOCH;
    let pick = move |value| Message::Pick(epoch, value);
    let change = move |value| Message::Draft(epoch, value);
    let id = FIELD + index as u64;
    let field = if COMBO {
        if CAPTURED {
            Widget::combo_box_with(
                id,
                "sao",
                change,
                crate::combo_box::ComboBoxWith::new(ITEMS, pick)
                    .with_label("Destination 東京")
                    .with_error("Choose a destination")
                    .with_visible_rows(2)
                    .unwrap(),
                style,
            )
        } else {
            Widget::combo_box(
                id,
                "sao",
                |v| Message::Draft(0, v),
                crate::combo_box::ComboBox::new(ITEMS, |v| Message::Pick(0, v))
                    .with_label("Destination 東京")
                    .with_error("Choose a destination")
                    .with_visible_rows(2)
                    .unwrap(),
                style,
            )
        }
    } else if CAPTURED {
        Widget::search_bar_with_suggestions_with(
            change,
            None,
            None,
            None,
            "Search 東京",
            SearchSuggestionsWith::new(
                ITEMS,
                SearchMatcher::Fuzzy,
                2,
                Some(ChangeCallback::new(pick)),
            )
            .unwrap(),
            style,
        )
        .with_id(id)
    } else {
        Widget::search_bar_with_suggestions(
            |v| Message::Draft(0, v),
            None,
            None,
            None,
            "Search 東京",
            SearchSuggestions::new(
                ITEMS,
                SearchMatcher::Fuzzy,
                2,
                Some(|v| Message::Pick(0, v)),
            )
            .unwrap(),
            style,
        )
        .with_id(id)
    };
    Some(Widget::Column {
        children: vec![field],
        style: Style {
            size: Size {
                width: taffy::prelude::percent(1.0),
                height: taffy::prelude::auto(),
            },
            padding: taffy::Rect {
                // Put the trigger near the card edge to exercise portal escape.
                top: taffy::prelude::length(crate::spacing::THREE_XL),
                left: taffy::prelude::length(0.0),
                right: taffy::prelude::length(0.0),
                bottom: taffy::prelude::length(0.0),
            },
            ..Style::default()
        },
    })
}

struct Model {
    keys: Vec<VirtualItemKey>,
    family: Family,
    combo: bool,
    captured: bool,
    fresh: bool,
    enabled: bool,
    blocked: bool,
    messages: Vec<Message>,
}
struct VirtualSearchApp;

fn builder<'a>(model: &Model) -> &'a dyn Fn(usize) -> Option<Widget<'a, Message>> {
    match (model.combo, model.captured, model.fresh) {
        (false, false, _) => &item::<false, false, 0>,
        (true, false, _) => &item::<true, false, 0>,
        (false, true, false) => &item::<false, true, 0>,
        (false, true, true) => &item::<false, true, 7>,
        (true, true, false) => &item::<true, true, 0>,
        (true, true, true) => &item::<true, true, 7>,
    }
}

impl AppLogic for VirtualSearchApp {
    type State = Model;
    type Message = Message;
    fn new(_: &mut FontSystem) -> Model {
        Model {
            keys: (1..=8).map(|v| VirtualItemKey::new(v).unwrap()).collect(),
            family: Family::List,
            combo: false,
            captured: false,
            fresh: false,
            enabled: true,
            blocked: false,
            messages: Vec::new(),
        }
    }
    fn view(model: &mut Model) -> Widget<'_, Message> {
        let items = KeyedVirtualItems::try_new(&model.keys, builder(model)).unwrap();
        let style = Style {
            size: Size {
                width: taffy::prelude::percent(1.0),
                height: taffy::prelude::length(140.0),
            },
            flex_shrink: 0.0,
            ..Style::default()
        };
        // Captured fields also exercise the captured collection traversal: popup
        // publication must escape the keyed item viewport for both API families.
        let collection = match (model.captured, model.family) {
            (true, Family::List) => {
                Widget::interactive_virtual_list_content_with(140.0, items, Message::Card, style)
            }
            (true, Family::Grid) => {
                Widget::interactive_virtual_grid_content_with(2, 140.0, items, Message::Card, style)
            }
            (true, Family::Carousel) => Widget::interactive_carousel_view_with(
                items,
                Message::Card,
                crate::CarouselConfig::uncontained(240.0).unwrap(),
                style,
            ),
            (false, Family::List) => {
                Widget::interactive_virtual_list_content(140.0, items, Message::Card, style)
            }
            (false, Family::Grid) => {
                Widget::interactive_virtual_grid_content(2, 140.0, items, Message::Card, style)
            }
            (false, Family::Carousel) => Widget::interactive_carousel_view(
                items,
                Message::Card,
                crate::CarouselConfig::uncontained(240.0).unwrap(),
                style,
            ),
        }
        .with_id(COLLECTION)
        .enabled(model.enabled);
        Widget::Column {
            children: vec![
                collection,
                Widget::modal(
                    model.blocked,
                    Widget::Column {
                        children: Vec::new(),
                        style: Style::default(),
                    },
                    None,
                    Style::default(),
                ),
            ],
            style: Style {
                size: Size::percent(1.0),
                padding: taffy::Rect::length(24.0),
                ..Style::default()
            },
        }
    }
    fn update(model: &mut Model, message: Message, _: &mut arboard::Clipboard) {
        model.messages.push(message);
    }
}

fn publish(r: &mut RutterRunner<VirtualSearchApp>) {
    r.engine.layout_dirty = true;
    r.engine.try_ensure_layout_for_input(VIEWPORT).unwrap();
}
fn field_id(r: &RutterRunner<VirtualSearchApp>) -> u64 {
    let mut path = vec![0]; // Collection is the first page child.
    crate::widget::push_interactive_virtual_item_path(&mut path, r.engine.app_state.keys[0]);
    path.push(0); // Field is the first child of the reconstructed item.
    let item = builder(&r.engine.app_state)(0).unwrap();
    let Widget::Column { children, .. } = item else {
        panic!("expected an item containing the search field");
    };
    children[0].resolved_id(&path).unwrap()
}
fn runner(family: Family, combo: bool, captured: bool) -> RutterRunner<VirtualSearchApp> {
    let mut engine = RutterEngine::<VirtualSearchApp>::new().expect("owned Xvfb clipboard");
    engine.app_state.family = family;
    engine.app_state.combo = combo;
    engine.app_state.captured = captured;
    engine.window = Some(Rc::new(MemoryWindow {
        size: VIEWPORT,
        scale: 1.0,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    let mut r = RutterRunner::with_engine(engine);
    r.active_window_id = Some(WindowId::from_raw(1));
    publish(&mut r);
    let id = field_id(&r);
    r.focus_widget(Some(id));
    r.engine
        .input_states
        .get_mut(&id)
        .unwrap()
        .set_text(&mut r.engine.font_system.borrow_mut(), "sao");
    action(&mut r, id, Action::Expand);
    publish(&mut r);
    r
}
fn event(r: &mut RutterRunner<VirtualSearchApp>, event: WindowEvent) {
    r.window_event(&MemoryEventLoop::default(), WindowId::from_raw(1), event);
    assert!(r.fatal_error.is_none(), "{:?}", r.fatal_error);
}
fn key(r: &mut RutterRunner<VirtualSearchApp>, key: NamedKey) {
    event(
        r,
        WindowEvent::KeyboardInput {
            device_id: None,
            is_synthetic: false,
            event: KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::Space),
                logical_key: Key::Named(key),
                text: None,
                location: KeyLocation::Standard,
                state: ElementState::Pressed,
                repeat: false,
                text_with_all_modifiers: None,
                key_without_modifiers: Key::Named(key),
            },
        },
    );
}
fn pointer(r: &mut RutterRunner<VirtualSearchApp>, point: Point) {
    for state in [ElementState::Pressed, ElementState::Released] {
        event(
            r,
            WindowEvent::PointerButton {
                device_id: None,
                state,
                primary: true,
                position: PhysicalPosition::new(point.x as f64, point.y as f64),
                button: ButtonSource::Mouse(MouseButton::Left),
                is_macos_activation_click: false,
            },
        );
    }
}
fn action(r: &mut RutterRunner<VirtualSearchApp>, id: u64, action: Action) {
    r.engine
        .accessibility_actions
        .handler()
        .do_action(ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: NodeId(id),
            data: None,
        });
    r.process_accessibility_actions();
    assert!(r.fatal_error.is_none());
}

fn popup_row(r: &mut RutterRunner<VirtualSearchApp>) -> skia_safe::Rect {
    let id = field_id(r);
    let view = VirtualSearchApp::view(&mut r.engine.app_state);
    let overlays = collect_open_search_overlays(
        &view,
        &r.engine.taffy,
        r.engine.last_root_node,
        &r.engine.widget_states,
        &r.engine.input_states,
        Some(id),
        (320.0, 360.0),
    );
    assert_eq!(overlays.len(), 1);
    let overlay = &overlays[0];
    assert_eq!(overlay.id, id);
    assert_eq!(overlay.matches[0].index, 1);
    let popup = search_popup_layout(overlay, (320.0, 360.0));
    assert!(popup.rect.left >= 0.0 && popup.rect.right <= 320.0);
    assert!(popup.rect.top >= 0.0 && popup.rect.bottom <= 360.0);
    let row = search_option_rect(popup, 0);
    assert_eq!(
        hit_test_search_overlay(SearchOverlayHitInput {
            widget: &view,
            taffy: &r.engine.taffy,
            root: r.engine.last_root_node,
            widget_states: &r.engine.widget_states,
            input_states: &r.engine.input_states,
            focused_id: Some(id),
            mouse: row.center(),
            viewport: (320.0, 360.0)
        }),
        Some(SearchOverlayHit::Suggestion { id, index: 1 })
    );
    row
}

fn published_options(r: &mut RutterRunner<VirtualSearchApp>) -> Vec<(u64, accesskit::Node)> {
    let id = field_id(r);
    let view = VirtualSearchApp::view(&mut r.engine.app_state);
    let update = build_accessibility_update(
        &r.engine.taffy,
        &view,
        r.engine.last_root_node,
        AccessibilityInputs {
            input_states: &r.engine.input_states,
            widget_states: &r.engine.widget_states,
            focused_widget_id: Some(id),
            viewport: (320.0, 360.0),
            direction: crate::i18n::LayoutDirection::Ltr,
        },
    );
    let mut ids = std::collections::HashSet::new();
    assert!(
        update.nodes.iter().all(|(id, _)| ids.insert(*id)),
        "duplicate accessibility nodes"
    );
    update
        .nodes
        .into_iter()
        .filter(|(_, node)| node.role() == Role::ListBoxOption)
        .map(|(id, node)| (id.0, node))
        .collect()
}

#[test]
fn virtual_search_window_pointer_keyboard_and_published_options_use_current_callbacks() {
    for family in FAMILIES {
        for combo in [false, true] {
            for captured in [false, true] {
                for input in ["pointer", "keyboard", "accessibility"] {
                    let mut r = runner(family, combo, captured);
                    let id = field_id(&r);
                    let row = popup_row(&mut r);
                    let options = published_options(&mut r);
                    assert_eq!(options.len(), 2);
                    assert_eq!(options[0].0, resolve_search_suggestion_id(id, 1));
                    assert_eq!(options[0].1.label(), Some(ITEMS[1]));
                    assert!(options[0].1.supports_action(Action::Click));
                    let bounds = options[0].1.bounds().unwrap();
                    assert!((bounds.x0 - row.left as f64).abs() <= 1.0);
                    assert!((bounds.y0 - row.top as f64).abs() <= 1.0);
                    r.engine.app_state.fresh = true;
                    r.engine.layout_dirty = true;
                    match input {
                        "pointer" => pointer(&mut r, row.center()),
                        "keyboard" => {
                            key(&mut r, NamedKey::ArrowDown);
                            key(&mut r, NamedKey::Enter);
                        }
                        _ => action(&mut r, options[0].0, Action::Click),
                    }
                    assert_eq!(
                        r.engine.app_state.messages,
                        vec![Message::Pick(if captured { 7 } else { 0 }, 1)],
                        "{family:?}, combo={combo}, captured={captured}, {input}"
                    );
                }
            }
        }
    }
}

#[test]
fn virtual_search_dirty_disable_and_full_tree_blocker_suppress_stale_selection() {
    for family in FAMILIES {
        for combo in [false, true] {
            for captured in [false, true] {
                for blocked in [false, true] {
                    for input in ["pointer", "keyboard", "accessibility"] {
                        let mut r = runner(family, combo, captured);
                        let id = field_id(&r);
                        let row = popup_row(&mut r);
                        key(&mut r, NamedKey::ArrowDown);
                        r.engine.app_state.enabled = blocked;
                        r.engine.app_state.blocked = blocked;
                        r.engine.layout_dirty = true;
                        match input {
                            "pointer" => pointer(&mut r, row.center()),
                            "keyboard" => key(&mut r, NamedKey::Enter),
                            _ => action(&mut r, resolve_search_suggestion_id(id, 1), Action::Click),
                        }
                        assert!(
                            r.engine.app_state.messages.is_empty(),
                            "{family:?}, combo={combo}, captured={captured}, blocked={blocked}, {input}"
                        );
                        publish(&mut r);
                        assert!(published_options(&mut r).is_empty());
                    }
                }
            }
        }
    }
}

#[test]
fn virtual_search_live_scroll_clipping_and_restoration_share_popup_exposure() {
    for family in [Family::List, Family::Grid] {
        for combo in [false, true] {
            for captured in [false, true] {
                let mut r = runner(family, combo, captured);
                let id = field_id(&r);
                let before = popup_row(&mut r);
                key(&mut r, NamedKey::ArrowDown);
                match r.engine.widget_states.get_mut(&COLLECTION).unwrap() {
                    WidgetState::VList(state) => state.scroll_y = 138.0,
                    WidgetState::VGrid(state) => state.scroll_y = 138.0,
                    _ => panic!("expected scrollable virtual collection"),
                }
                publish(&mut r);
                assert!(published_options(&mut r).is_empty());
                assert!(!r.engine.runtime_caches.searches[&id].popup_exposed);
                key(&mut r, NamedKey::Enter);
                action(&mut r, resolve_search_suggestion_id(id, 1), Action::Click);
                assert!(r.engine.app_state.messages.is_empty());
                match r.engine.widget_states.get_mut(&COLLECTION).unwrap() {
                    WidgetState::VList(state) => state.scroll_y = 0.0,
                    WidgetState::VGrid(state) => state.scroll_y = 0.0,
                    _ => unreachable!(),
                }
                publish(&mut r);
                assert_eq!(popup_row(&mut r), before);
                assert_eq!(published_options(&mut r).len(), 2);
            }
        }
    }
}
