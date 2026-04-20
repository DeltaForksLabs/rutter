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
- `ash`, `glutin`, and `softbuffer` for Vulkan, OpenGL, and CPU fallback rendering paths.

The framework is still evolving, but it already includes a broad set of widgets, demos, input handling, layout synchronization, text editing, virtualized views, and overlay components.

## Features

### Core

- Typed application model based on state, messages, update, and view functions.
- Declarative widget tree with manual or automatic widget IDs.
- Taffy-backed layout tree with synchronization instead of full rebuilds on every layout update.
- Theme support through a central `Theme` type.
- Keyboard focus traversal for interactive widgets.
- Clipboard paste sanitization for text inputs.
- Safe image decoding limits to reduce malicious allocation risk.

### Widgets

- Text, image, spacer, divider, container, row, and column primitives.
- Button variants, checkbox, switch, radio, slider, select, progress bar, and spinner.
- Text input, search bar, and multiline text area.
- Scroll view, virtual list, and virtual grid for large item sets.
- Accordion, tab bar, modal, dialog, toast, context menu, and generic popover.

### Overlays

- Dialogs with floating positions: top, center, and bottom.
- Context menus triggered from right-click interactions.
- Generic popovers anchored to a widget and capable of rendering arbitrary widget content.
- Toast notifications with independent placement and timers.

### Rendering

- Vulkan backend attempted first.
- OpenGL backend used as GPU fallback.
- CPU softbuffer backend used as final fallback.
- Skia canvas abstraction shared across rendering backends.
- Cached text shaping buffers for repeated text rendering paths.

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
cargo run -- progress
cargo run -- scroll
cargo run -- tabs
cargo run -- accordion
cargo run -- dialog
cargo run -- modal_toast
cargo run -- vlist
cargo run -- vgrid
cargo run -- popover
cargo run -- advanced
```

The demos live in `src/demos` and are intended to exercise isolated widgets and interaction patterns.

## Quick Start

Add Rutter as a dependency once the crate or repository dependency is available for your application.

A minimal application follows the framework shape:

```rust
use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{AppLogic, RutterRunner, Theme, Widget};
use taffy::prelude::Style;

#[derive(Default)]
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

## Architecture

### Application Model

Applications implement `AppLogic`. The framework owns the native event loop and calls into application code through:

- `new` to initialize state.
- `view` to build the widget tree.
- `update` to process messages.
- `theme` to provide colors, spacing, typography, and shape values.

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
- `SearchBar`
- `Checkbox`
- `Switch`
- `Radio`
- `Slider`
- `Select`

### Display Widgets

- `Text`
- `Image`
- `ProgressBar`
- `Spinner`

### Composite Widgets

- `Accordion`
- `TabBar`
- `Modal`
- `Dialog`
- `Toast`
- `ContextMenu`
- `Popover`
- `VirtualList`
- `VirtualGrid`

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
- GPU-first rendering with CPU fallback.

Further performance work is expected as the framework matures.

## Repository Layout

```text
src/
  app.rs                  AppLogic trait and application contract
  engine/                 Runtime engine, runner, GPU backends, widget state
  input_state.rs          Editable text state and cursor/selection helpers
  layout.rs               Taffy layout tree construction and synchronization
  render/                 Skia rendering, hit testing, text pipeline
  theme.rs                Theme values and visual defaults
  widget.rs               Widget definitions and constructors
  demos/                  Demo applications for individual widgets
```

## Documentation

Official documentation is under construction.

Until the documentation site is available, use:

- This README for project orientation.
- `src/demos` for practical examples.
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
- Harden Vulkan/OpenGL backend behavior across more systems.
- Improve test coverage for overlays, keyboard navigation, and complex layouts.

## License

Rutter is licensed under either of:

- MIT License, see `LICENSE_MIT`.
- Apache License 2.0, see `LICENSE_APACHE_2.0`.

You may choose either license when using, modifying, or distributing the project.

## Credits

Built by https://github.com/DeltaForksLabs.

