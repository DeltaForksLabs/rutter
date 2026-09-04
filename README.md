# Rutter

Rutter is an experimental Rust GUI framework built around immediate, strongly typed application logic, Taffy layout, Skia rendering, and native window integration through winit.

The project is focused on building a pragmatic foundation for desktop interfaces: predictable layout, rich widgets, keyboard navigation, GPU rendering paths, and a CPU fallback for restricted environments.

> Official documentation is currently under construction. This README is the primary project overview for now.

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Project Status](#project-status)
- [Demo](#demo)
- [Quick Start](#quick-start)
- [Architecture](#architecture)
- [Widgets](#widgets)
- [Rendering Backends](#rendering-backends)
- [Performance Notes](#performance-notes)
- [Repository Layout](#repository-layout)
- [Contributing / Feedback](#contributing--feedback)
- [License](#license)
- [Credits](#credits)

## Overview

Rutter is designed as a native GUI framework for Rust applications. It uses an `AppLogic` trait to keep state, messages, update logic, view construction, and theme configuration explicit.

The current implementation combines:

- `winit` for native window and event-loop integration.
- `taffy` for flexbox-style layout.
- `skia-safe` for drawing primitives, text, images, shadows, rounded rectangles, and GPU surfaces.
- `cosmic-text` for text shaping and editable text buffers.
- `fluent-rs` for locale-aware message formatting and RTL layout direction.
- `ash`, `glutin`, and `softbuffer` for Vulkan, OpenGL, and CPU fallback rendering paths.

The framework is still evolving, but it already includes a broad set of widgets, demos, input handling, layout synchronization, text editing, virtualized views, and overlay components.

## Features

### Core

- Typed application model based on state, messages, update, and view functions.
- Declarative widget tree with manual or automatic widget IDs.
- Taffy-backed layout tree with synchronization instead of full rebuilds on every layout update.
- State-aware Light/Dark theme support through a central `Theme` type.
- Locale and Project Fluent catalog helpers for i18n and RTL layout direction.
- Keyboard focus traversal for interactive widgets.
- Audited multi-window runtime with stable surface IDs and per-window event routing.
- Clipboard paste sanitization for text inputs.
- Safe image decoding limits to reduce malicious allocation risk.

### Accessibility

- AccessKit-backed accessibility tree integrated with `winit`.
- Semantic roles for buttons, text inputs, toggles, sliders, progress indicators, lists, grids, menus, dialogs, and status messages.
- Accessible labels, values, toggle state, numeric ranges, and focus propagation from the Rutter widget tree.
- Window events are forwarded only to the matching window's AccessKit adapter so platform assistive technologies observe the correct UI.
- On platforms with native visibility control, the window is kept hidden until the accessibility adapter is initialized, avoiding an initial inaccessible window snapshot.

### Widgets

- Text, rich text spans, image, spacer, divider, container, row, and column primitives.
- Text and rich-content button variants, checkbox, switch, radio, slider, counter, select, progress bar, and spinner.
- Text input, search bar, and multiline text area.
- Scroll view, virtual list, virtual grid, and horizontally virtualized carousel for large item sets.
- Calendar, date picker, dropdown menu, accordion, tab bar, modal, dialog, toast, context menu, and generic popover.

### Overlays

- Dialogs with floating positions: top, center, and bottom.
- Context menus triggered from right-click interactions.
- Generic popovers anchored to a widget and capable of rendering arbitrary widget content.
- Accessible dropdown menus with viewport-aware submenus and independent scrolling surfaces.
- Toast notifications with independent placement and timers.

### Rendering

- Vulkan backend attempted first.
- OpenGL backend used as GPU fallback.
- OpenGL contexts are rebound before every frame and resize operation, allowing independent surfaces to render alternately.
- CPU softbuffer backend used as final fallback.
- Skia canvas abstraction shared across rendering backends.
- Image decoding uses Skia by default through Rutter-owned decode limits.
- The `image-rs-decoder` Cargo feature switches raster decoding to the `image` crate.
- Cached text shaping buffers for repeated text rendering paths.

### Internationalization

- Fluent FTL resources can be loaded through `FluentCatalog`.
- `Locale` infers `LayoutDirection::Rtl` for Arabic, Hebrew, Persian, Urdu, and other RTL scripts.
- `AppLogic::locale()` drives the root Taffy layout direction without changing state/update/view flow.

## Project Status

Rutter is under active development. APIs may change while the framework settles.

Current focus areas include:

- Stabilizing widget APIs.
- Expanding demos and integration coverage.
- Improving GPU rendering reliability across platforms.
- Refining accessibility and keyboard workflows.
- Building official documentation.

The official documentation site is not available yet. It is planned and currently under construction.

## Demo

Run the integrated demo:

```bash
cargo run
```

Run a specific demo:

```bash
cargo run -- text_input
cargo run -- text_area
cargo run -- search_bar
cargo run -- controls
cargo run -- slider
cargo run -- counter
cargo run -- progress
cargo run -- scroll
cargo run -- tabs
cargo run -- accordion
cargo run -- dialog
cargo run -- modal_toast
cargo run -- vlist
cargo run -- vgrid
cargo run -- popover
cargo run -- dropdown_menu
cargo run -- calendar
cargo run -- time
cargo run -- carousel
cargo run -- multi_window
cargo run -- rich_text
cargo run -- advanced
```

Every widget example starts in Dark mode and exposes an accessible SVG sun/moon toggle in the upper-right corner. Applications can preserve the static `theme()` API or implement `theme_for(state)` when the active palette depends on application state; opaque windows are cleared with the resolved `theme.surface` color.

The widget demos live in `examples/widgets` and are intended to exercise isolated widgets and interaction patterns. The `examples/apps` directory is reserved for future complete example applications built with Rutter.

## Quick Start

Add Rutter as a dependency once the crate or repository dependency is available for your application.

A minimal application follows the framework shape:

```rust
use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{AppLogic, RutterRunner, Theme, Widget};
use taffy::prelude::Style;

#[derive(Clone, Default)]
struct State {
    count: usize,
}

#[derive(Debug, Clone)]
enum Msg {
    Increment,
}

struct App;

impl AppLogic for App {
    type State = State;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        State::default()
    }

    fn view<'a>(state: &'a mut State) -> Widget<'a, Msg> {
        Widget::Button {
            text: "Increment",
            on_press: Msg::Increment,
            style: Style::default(),
            color: None,
            variant: rutter::ButtonVariant::Primary,
        }
    }

    fn update(state: &mut State, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::Increment => state.count += 1,
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

fn main() {
    RutterRunner::<App>::run();
}
```

### Multi-window applications

`MultiWindowRunner` owns every native window, backend, AccessKit adapter, and input runtime. Applications use stable `SurfaceId` values and emit `SurfaceCommand` operations instead of creating Winit windows directly. Unknown events from failed backend probes are discarded before accessibility or rendering side effects. After the first surface commits, later windows reuse its backend type instead of repeating failed Vulkan/OpenGL probes on Wayland.

Run `cargo run -- multi_window` to open a centered **Open Second Window** button. The requested temporary inspector demonstrates position and size constraints, topmost level, deferred visibility, explicit redraw, focus events, and automatic closure after it gains and then loses focus.

```rust
use rutter::{
    CloseBehavior, MultiWindowAppLogic, MultiWindowRunner, SecondaryPointerContext,
    SurfaceCommand, SurfaceEvent, SurfaceId, SurfaceRequest, Widget, WindowConfig, WindowLevel,
};

const PANEL: SurfaceId = SurfaceId::new(1);
const POPUP: SurfaceId = SurfaceId::new(2);

// State is cloned into isolated per-window UI sessions after each update.
// Rc-backed fields can keep large shared models inexpensive to synchronize.
// State also stores `popup_open: bool` for the optional pointer popup below.
impl MultiWindowAppLogic for App {
    type State = State;
    type Message = Msg;

    fn new(_: &mut cosmic_text::FontSystem) -> State { State::default() }

    fn initial_surfaces() -> Vec<SurfaceRequest> {
        let panel = WindowConfig::default()
            .with_title("Panel")
            .with_position(160, 140)
            .with_min_inner_size(320, 200).expect("positive minimum size")
            .with_max_inner_size(900, 700).expect("maximum must exceed minimum")
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_close_on_focus_loss(true)
            .with_close_behavior(CloseBehavior::ExitApplication);
        vec![SurfaceRequest::new(PANEL, panel)]
    }

    fn view<'a>(state: &'a mut State, surface: SurfaceId) -> Widget<'a, Msg> {
        let _ = (state, surface);
        Widget::Spacer { style: Default::default() }
    }

    fn update(
        state: &mut State,
        source: SurfaceId,
        msg: Msg,
        _clipboard: &mut arboard::Clipboard,
    ) -> Vec<SurfaceCommand> {
        let _ = (state, source, msg);
        Vec::new()
    }

    fn surface_event(
        state: &mut State,
        surface: SurfaceId,
        event: SurfaceEvent,
    ) -> Vec<SurfaceCommand> {
        state.record_focus(surface, event);
        Vec::new()
    }

    fn surface_created(state: &mut State, surface: SurfaceId) {
        if surface == POPUP {
            state.popup_open = true;
        }
    }

    fn surface_closed(state: &mut State, surface: SurfaceId) {
        if surface == POPUP {
            state.popup_open = false;
        }
    }

    fn secondary_pointer_pressed_with_context(
        state: &mut State,
        source: SurfaceId,
        context: SecondaryPointerContext,
    ) -> Vec<SurfaceCommand> {
        if source != PANEL || state.popup_open {
            return Vec::new();
        }
        let Some(position) = context.desktop_position() else {
            return Vec::new(); // Use an in-surface overlay when absolute positioning is unavailable.
        };
        let popup = WindowConfig::default()
            .with_title("Pointer popup")
            .with_position(position.x(), position.y())
            .with_decorations(false)
            .with_inner_size(320, 240).expect("positive popup size")
            .with_close_on_focus_loss(true);
        vec![SurfaceCommand::Open(SurfaceRequest::new(POPUP, popup))]
    }
}

MultiWindowRunner::<App>::run();
```

Applications that load configuration or dependencies before entering the event loop can inject both state and a dynamically constructed surface registry. The fallible factory runs once before native event-loop creation and receives the same `FontSystem` later shared by every surface engine, so font-aware initialization does not create a detached text environment.

```rust
let initial_state = State::from_loaded_config(config);
let panel_surfaces = active_panels
    .into_iter()
    .map(|panel| SurfaceRequest::new(panel.id, panel.window))
    .collect();

MultiWindowRunner::<App>::run_with(
    move |font_system| {
        State::load_fonts(font_system)?;
        Ok::<_, ConfigError>(initial_state)
    },
    panel_surfaces,
);
```

If state construction is already complete and does not need the shared font database, use `MultiWindowRunner::<App>::run_with_state(initial_state, panel_surfaces)`.

Each committed surface has independent title, initial position, inner/minimum/maximum size, window level, visibility, decorations, resizability, transparency, close behavior, widget state, layout, graphics presentation, and accessibility routing. Sizes and positions use physical pixels; negative coordinates are valid for multi-monitor desktops. Position, visibility, and window level are platform hints and may be ignored, notably by Wayland compositors.

`SurfaceEvent::FocusChanged` reaches application logic after the matching native event is forwarded to AccessKit and the surface engine. `SurfaceCommand::SetVisible` changes native visibility and persists the desired value across suspension/resume. `SurfaceCommand::RequestRedraw` asks the compositor for an asynchronous frame without invalidating layout; redraw requests can be coalesced. Both commands reject unknown logical surface IDs, while redraw is a safe no-op for a registered surface during suspension.

An in-surface context-menu target first reaches `AppLogic::context_menu_opening` (or `MultiWindowAppLogic::context_menu_opening`), which can return a typed selection message for `update` before the overlay opens. `ContextMenuTarget::id()` is the resolved menu ID; assign stable manual IDs when mapping a target to application-owned items. When a menu wraps a virtual list or grid, `ContextMenuTarget::virtual_item()` exposes `ContextMenuVirtualItem::List` or `ContextMenuVirtualItem::Grid` with the resolved collection ID and pressed index, including the collection's current scroll offset. Unclaimed right-button presses reach `secondary_pointer_pressed_with_context` only after open select, dropdown, context-menu, and popover overlays have had dismissal priority; visible modals and dialogs consume the press. `SecondaryPointerContext` contains logical client coordinates, the source scale factor, and optional physical desktop coordinates. Absolute coordinates are unavailable on Wayland, Android, iOS, and Web, where `desktop_position()` returns `None` and applications should retain an in-surface overlay fallback. The original `secondary_pointer_pressed` callback remains supported through the default compatibility bridge. Track a fixed popup surface's lifecycle—as in the example—to avoid opening a duplicate `SurfaceId`.

Temporary panels can use `WindowConfig::with_close_on_focus_loss(true)`. The runtime waits until that window has gained focus at least once before closing it on focus loss, which avoids treating an initial unfocused notification as dismissal. Closing a normal secondary surface removes only that surface; the event loop exits when no surfaces remain or when an `ExitApplication` close policy/`SurfaceCommand::Exit` requests it.

## Architecture

### Application Model

Applications implement `AppLogic`. The framework owns the native event loop and calls into application code through:

- `new` to initialize state.
- `view` to build the widget tree.
- `update` to process messages.
- `context_menu_opening` to emit a selection message for a right-clicked context-menu target, including virtual collection item metadata, before its overlay opens.
- `theme` for a static palette or `theme_for` for state-selected colors, spacing, typography, and shape values.

### Password input security

`Widget::TextInput` password mode masks rendering, blocks clipboard copy, omits values from accessibility, and zeroizes framework-managed temporary buffers where possible. It is not a secret-storage boundary: password callbacks still transfer a `String` to application code. Applications must avoid logging, cloning, or retaining those messages, use a dedicated zeroizing secret type after receipt, and disable core dumps where their deployment requires stronger memory-disclosure protection.

### Transparent top-level surfaces

Applications can opt into compositor transparency by returning `SurfaceConfig::transparent()` from `AppLogic::surface_config`. Opaque white presentation remains the default. Transparent mode requires premultiplied-alpha support from Vulkan or confirmed alpha support from OpenGL; the CPU/Softbuffer backend is rejected rather than silently presenting an opaque surface. Rounded `Widget::Container` values clip their normal child subtree, allowing a full-window rounded panel to retain transparent corner pixels. Window input regions remain rectangular.

### Layout System

Rutter uses Taffy as its layout engine. Widget trees are converted into a synchronized layout blueprint so keyed nodes can be reused where possible. This reduces avoidable layout-tree churn and keeps layout state tied to widget identity.

### Runtime State

Interactive widgets store runtime state in engine-managed maps keyed by resolved widget IDs. This includes text editor state, scroll offsets, slider drag state, select state, modal/dialog state, toast timers, context-menu state, popover geometry, and virtualized selection state.

### Input And Events

The runner translates winit events into framework actions:

- Mouse clicks, drag tracking, wheel scrolling, and right-click context menus.
- Keyboard focus traversal and activation.
- Text input, cursor movement, deletion, selection, paste, copy, cut, and submit behavior.
- Overlay dismissal for dialogs, modals, context menus, and popovers.

### Rendering Pipeline

Rendering is performed through a Skia `Canvas`. The engine selects the best available backend at startup, prepares a frame, draws the widget tree and overlays, then presents through the backend.

## Widgets

### Layout Widgets

- `Column`
- `Row`
- `Container`
- `Spacer`
- `Divider`
- `ScrollView`

### Input Widgets

- `TextInput`
- `TextArea`
- `SearchBar` (with optional integrated fuzzy/custom suggestions)
- `Checkbox`
- `Button`
- `ButtonContent`
- `Switch`
- `Radio`
- `Slider`
- `Counter`
- `Select`

An open `Select` keeps its trigger in normal layout and renders its options in a dedicated overlay pass. The popup therefore covers later content without moving siblings, opens on the side with more usable space, and constrains long option lists to a viewport-safe window that follows keyboard and mouse-wheel selection.

`Counter` is a controlled signed-integer spin button rendered as `- value +`. It sends `on_change` only when decrement or increment produces a bounded value. Holding an on-screen action starts repeating after a short delay and progressively accelerates until release or a range boundary. With an automatic width, it measures its central value from its character count so the action buttons stay close as the number grows. Use `Widget::counter(value, min, max, step, on_change, style, label)` for direct construction or `Widget::try_counter` to validate the range, value, and step. Keyboard users can adjust it with `+`, `-`, Arrow keys, Home, End, Page Up, and Page Down; assistive technologies receive a `SpinButton` with increment, decrement, and numeric-range metadata.

`Select` chooses one value. `DropdownMenu` instead exposes commands and optional checkbox/radio state; it does not represent a selected form value.

### Search Suggestions

`SearchBar` gains an integrated suggestions popup when built through `Widget::search_bar_with_suggestions`. The popup opens while the field is focused and its text is non-empty, ranks a borrowed `&[&str]` item slice every frame, and closes on Escape until the text changes again. Keyboard users navigate with Arrow keys, Home, End, Page Up, and Page Down; Enter selects the highlighted row (falling back to the normal submit when nothing is highlighted), mouse clicks route to the original item index, and the wheel scrolls the row window. An empty result set renders an explicit no-matches label that consumes pointer input without activating a row or leaking clicks to covered widgets. Assistive technologies receive an editable combobox, list autocomplete metadata, a controlled listbox, active-option selection, and Focus/Expand/Collapse/Click actions.

Two matching modes share one ranked contract — `None` for a non-match, otherwise a deterministic score:

- `SearchMatcher::Fuzzy`: case- and accent-insensitive subsequence ranking (Portuguese diacritics fold onto their ASCII base) with bonuses for exact substrings, item starts, word boundaries, and consecutive runs.
- `SearchMatcher::Custom(fn(&str, &str) -> Option<u32>)`: fully pluggable scoring owned by the application.

The engine also exports the matcher standalone so lists outside the widget can reuse it: `SearchMatcher::score`, `filter_ranked(items, query, matcher, max_results)` returns `SearchMatch { index, score }` values ordered best-first, and `SearchSuggestions::new` validates `max_results >= 1` up front. Source positions are callback indices and accessibility identities; keep their order stable while a visible popup may have queued assistive-technology actions.

```rust
use rutter::{SearchMatcher, Widget};
use rutter::search::SearchSuggestions;

enum Msg { Query(String), Picked(usize) }

let movies = ["Matrix", "O Senhor dos Anéis"];
let suggestions = SearchSuggestions::new(
    &movies,
    SearchMatcher::Fuzzy,
    6,
    Some(Msg::Picked),
)
.unwrap();
let bar: Widget<'_, Msg> = Widget::search_bar_with_suggestions(
    Msg::Query,
    None,
    None,
    None,
    "Buscar...",
    suggestions,
    taffy::prelude::Style::default(),
);
```

See `examples/widgets/search_bar_demo.rs` or run `cargo run -- search_bar` for side-by-side fuzzy and custom-mode fields with Portuguese labels.

Migration note for `0.27.0`: `Widget::search_bar(...)` keeps its existing signature. Code that constructs the public `Widget::SearchBar` variant directly must add `suggestions: None` or provide a validated suggestion configuration.

### Display Widgets

- `Text`
- `RichText`
- `Image`
- `ProgressBar`
- `Spinner`
- `Clock`

`RichText` replaces `StrongText` with one accessible text leaf containing inherited span styles for weight, italic slant, underline, size, and color. Skia Paragraph provides matching shaping, wrapping, RTL layout, measurement, and painting.
See [`docs/RICH_TEXT_MIGRATION.md`](docs/RICH_TEXT_MIGRATION.md) for the 0.19 migration from `StrongText` and exhaustive `Widget`/`RutterContext` matches.

```rust
let content = RichText::from_spans([
    RichTextSpan::new("Rutter ").bold(),
    RichTextSpan::new("rich text").italic().underline(),
]);
let widget: Widget<'_, ()> = Widget::rich_text(content, Style::default());
```

### Composite Widgets

- `Accordion`
- `TabBar`
- `Modal`
- `Dialog`
- `Toast`
- `ContextMenu`
- `DropdownMenu`
- `Popover`
- `Calendar`
- `DatePicker`
- `TimePicker`
- `CarouselView`
- `VirtualList`
- `VirtualListContent`
- `VirtualGrid`
- `VirtualGridContent`

`VirtualListContent` and `VirtualGridContent` keep row and cell virtualization while allowing each visible item to render arbitrary widget content, including images, icons, and composed layouts.

`VirtualSelection` configures either single selection or controlled multiselection for virtual collections through `virtual_list_with_selection`, `virtual_list_content_with_selection`, `virtual_grid_with_selection`, and `virtual_grid_content_with_selection`. Use `VirtualSelection::multiple(&state.selected, Msg::SelectionChanged)` for file-manager-style selection: a plain click replaces the set, Shift selects a range, and Ctrl (or Command on macOS) toggles an item. Dragging selects a contiguous list range or a rectangular grid region; Ctrl/Command-drag adds that region to the selection. The multiselection callback receives a sorted, unique `Vec<usize>`; store it in application state and supply it again on the next view. Arrow keys replace or extend the selection with Shift, Ctrl/Command+Arrow moves the active item without changing selection, Ctrl/Command+Space toggles the active item, and Ctrl/Command+A selects all items.

```rust
use rutter::{VirtualSelection, Widget};
use taffy::prelude::Style;

enum Msg {
    SelectionChanged(Vec<usize>),
}

let selected = [0, 2];
let names = |index| Some(format!("File {index}"));
let files = Widget::virtual_list_with_selection(
    32.0,
    100,
    &names,
    VirtualSelection::multiple(&selected, Msg::SelectionChanged),
    Style::default(),
);
```

### DropdownMenu

`Widget::dropdown_menu` creates an engine-owned menu button. Entries are recursively owned and support commands, disabled commands, separators, checkboxes, radio items, and nested submenus. Root menus prefer the block-end side of the trigger, submenus prefer inline-end, and both flip or clamp against the viewport. Long menu levels scroll independently with the mouse wheel.

```rust
use rutter::{DropdownMenuEntry, Widget};

let menu = Widget::dropdown_menu(
    "Project actions",
    vec![
        DropdownMenuEntry::item("Open", Msg::Open),
        DropdownMenuEntry::checkbox("Show grid", state.show_grid, Msg::ToggleGrid),
        DropdownMenuEntry::separator(),
        DropdownMenuEntry::submenu(
            "Export",
            vec![
                DropdownMenuEntry::item("JSON", Msg::ExportJson),
                DropdownMenuEntry::disabled_item("PDF (not installed)"),
            ],
        ),
    ],
    trigger_style,
);
```

Sibling entries with the same kind, label, and enabled state must use distinct stable keys, such as `DropdownMenuEntry::item("Open", message).with_key(7)`. This lets the runtime reject stale actions safely when otherwise identical commands reorder.

Enter, Space, Arrow Down, and Arrow Up open the menu. Arrow keys, Home, End, and typeahead move focus; the locale-aware inline arrow opens submenus and the opposite arrow returns to the parent. Escape and item activation close the menu and restore trigger focus, while Tab closes it and continues normal traversal. Disabled entries remain focusable for announcement but cannot activate.

AccessKit exposes the trigger as a button with `HasPopup::Menu`, each surface as `Menu`, and command/check/radio descendants through the matching menu-item roles. Focus, click, expand, and collapse requests from assistive technology are routed through the same runtime transitions used by pointer and keyboard input. See `examples/widgets/dropdown_menu_demo.rs` or run `cargo run -- dropdown_menu`.

### CarouselView

`Widget::carousel_view` presents a finite, horizontally scrollable collection whose item builder runs only for the visible range plus one overscan item. `CarouselConfig::uncontained` gives every item a fixed extent, clamped to the viewport width for oversized cards. `CarouselConfig::weighted` accepts positive relative weights such as `[1, 6, 1]`; while scrolling, adjacent items interpolate between those extents so each item can occupy the largest slot.

Wheel input is routed to a main-tree scrollable under the cursor, including native horizontal trackpad deltas and carousels inside vertically scrolled content. Clicking an item focuses and selects it; Left/Right follow the locale's layout direction, while Home and End select the collection boundaries. Optional item snapping advances by one logical item boundary per scroll input.

```rust
let config = CarouselConfig::weighted([1, 6, 1])?
    .with_item_snapping(true)
    .with_accessibility_label("Featured projects");
let carousel = Widget::carousel_view(
    2_000,
    |index| Some(project_card(index)),
    Msg::SelectProject,
    config,
    carousel_style,
);
```

Carousel item widgets follow the same security contract as `VirtualListContent`: they are visual-only and receive isolated runtime-state maps. Nested buttons and inputs are therefore rendered but do not handle interaction; selection belongs to the carousel. The current release is horizontal and finite, and intentionally defers touch dragging, infinite looping, vertical layouts, public controllers, overlay-aware wheel routing, parent-scroll bubbling, and custom scroll physics.

AccessKit exposes the carousel as a labeled horizontal collection with its total item count and orientation. Accessibility actions and virtual item descendants are not routed yet; keyboard and pointer interaction remain available through Rutter's normal input path. See `examples/widgets/carousel_demo.rs` or run `cargo run -- carousel` for weighted and uncontained layouts.

### Calendar and Date Picker

`Widget::calendar` provides a six-week monthly grid with single-date selection and separate month/year navigation. `Widget::date_picker` reuses the same calendar inside an anchored `Popover`. Both are controlled widgets: the application stores the selected `CalendarDate`, visible `CalendarMonth`, and date-picker open state.

Dates use Rutter's validated proleptic Gregorian types and do not require a time-zone or third-party date dependency. English labels are used by default; `CalendarConfig`, `CalendarLabels`, and `WeekStart` provide explicit localization and first-weekday configuration. Adjacent-month cells remain selectable, so applications should update the visible month from `date.calendar_month()` after selection.

Choose a popup size that fits the application's minimum supported viewport. Popover placement is clamped to the window, but oversized popup content is clipped rather than reflowed.

The date picker's `accessibility_label` should include its controlled value, as demonstrated by the example; its anchor also exposes expanded/collapsed state. The composed calendar exposes its month heading, navigation controls, and individual day buttons to AccessKit. Grid-specific arrow navigation and a semantic selected-state announcement remain future accessibility work; keyboard users traverse the controls through the existing focus order.

```rust
let config = CalendarConfig::new(CalendarLabels::PORTUGUESE, WeekStart::Monday);
let calendar = Widget::calendar_with_config(
    state.visible_month,
    state.selected_date,
    Msg::SelectDate,
    Msg::NavigateMonth,
    config,
    calendar_style,
);
```

See `examples/widgets/calendar_demo.rs` or run `cargo run -- calendar` for standalone and popover usage.

### Clock and Time Picker

`Widget::clock` renders the current instant in a `TimeZone` and redraws at each second boundary. `TimeZone::iana` accepts a named IANA zone with daylight-saving rules, while `TimeZone::fixed_offset` creates a fixed offset east of UTC. `ClockFormat` selects 12- or 24-hour notation and whether seconds or the zone name are visible. The clock is exposed as dynamic text to AccessKit.

`Widget::time_picker` composes an anchored `Popover`, bounded `Counter` fields, and a `Select` for IANA timezone choices. It is controlled: keep `open`, `TimeOfDay`, and the selected zone index in application state. Hour, minute, and second callbacks remain separate so an application can choose its own carry and wrapping behavior. `TimePickerConfig::new` validates every zone name and returns a descriptive error instead of accepting an invalid identifier.

```rust
use rutter::{ClockConfig, ClockFormat, TimeOfDay, TimePickerConfig, TimeZone, Widget};
use taffy::prelude::Style;

#[derive(Clone)]
enum Msg { Toggle, Close, Hour(i64), Minute(i64), Second(i64), Zone(usize) }

const ZONES: &[&str] = &["UTC", "America/Sao_Paulo"];
let selected_time = TimeOfDay::new(9, 30, 0)?;
let picker_config = TimePickerConfig::new(
    ZONES, 0, Msg::Toggle, Msg::Close, Msg::Hour, Msg::Minute,
    Msg::Second, Msg::Zone, "Event time",
)?;
let clock: Widget<'_, Msg> = Widget::clock_with_config(
    TimeZone::UTC,
    ClockConfig::new(ClockFormat::twelve_hour(), 28.0)?,
    Style::default(),
    "Current UTC time",
);
let picker = Widget::time_picker(
    false, selected_time, picker_config, Style::default(), Style::default(),
);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Resolve a scheduled wall-clock value with `TimeOfDay::resolve(CalendarDate, TimeZone)` and handle `LocalTimeResolution::{Single, Ambiguous, Nonexistent}` explicitly. This prevents daylight-saving gaps and repeated local times from being silently converted to a different instant. See `examples/widgets/time_demo.rs` or run `cargo run -- time` for a live clock and Portuguese picker labels.

## Rendering Backends

Rutter tries to initialize rendering backends in this order:

1. Vulkan through `ash` and `ash-window`.
2. OpenGL through `glutin` and `glutin-winit`.
3. CPU rendering through `softbuffer`.

This keeps the application usable in environments where a high-performance GPU backend is not available.

## Performance Notes

Rutter includes several optimizations intended for responsive desktop UIs:

- Layout tree synchronization instead of unconditional full rebuilds.
- Runtime callback caches for common input paths.
- Text shaping cache for repeated text rendering.
- Virtualized list and grid widgets for large item counts.
- Fixed and weighted carousel layouts that materialize the visible range plus one overscan item.
- GPU-first rendering with CPU fallback.

Further performance work is expected as the framework matures.

## Repository Layout

```text
src/
  accessibility/          AccessKit tree construction and action routing
  widgets/calendar/       Gregorian date types and composed calendar widgets
  widgets/carousel/       Carousel configuration, geometry, and runtime state
  widgets/time/           Timezone-aware clock values and time-picker controls
  widgets/search/         Search suggestion matching engine and popup config
  app.rs                  AppLogic trait and application contract
  engine/                 Runtime engine, runner, GPU backends, widget state
  input/                  Limits, editable state, history, and normalization
  layout.rs               Taffy layout tree construction and synchronization
  multi_window/           Multi-surface lifecycle and window configuration
  render/                 Skia rendering, hit testing, text pipeline
  theme.rs                Theme values and visual defaults
  widget/                 Core widget definitions, IDs, and tree validation
examples/
  apps/                   Future complete example applications
  widgets/                Widget-focused demo modules used by the demo runner
tests/                    Integration tests and future black-box test suites
```

## Documentation

Official documentation is under construction.

Until the documentation site is available, use:

- This README for project orientation.
- `examples/widgets` for practical widget examples.
- The public exports in `src/lib.rs` for the current API surface.
- Tests as behavior references for layout, input, rendering, and widget state.

## Contributing / Feedback

Feedback, issue reports, experiments, and focused pull requests are welcome.

Good contributions should:

- Keep changes scoped and explain the technical impact.
- Preserve existing demos and tests.
- Add or update tests when behavior changes.
- Avoid unrelated formatting or refactors.
- Mention platform-specific rendering behavior when relevant.

The project is still moving quickly, so larger API changes should be discussed before implementation.

## Roadmap

Near-term priorities:

- Improve official documentation and examples.
- Stabilize the public widget API.
- Expand accessibility behavior.
- Expand locale-aware examples and text rendering coverage.
- Harden Vulkan/OpenGL backend behavior across more systems.
- Improve test coverage for overlays, keyboard navigation, and complex layouts.

## License

Rutter is licensed under either of:

- MIT License, see `LICENSE_MIT`.
- Apache License 2.0, see `LICENSE_APACHE_2.0`.

You may choose either license when using, modifying, or distributing the project.

## Credits

Built by https://github.com/DeltaForksLabs.
