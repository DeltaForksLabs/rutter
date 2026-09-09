# Development Log

## [2026-09-09T02:23:28-03:00] - Semantic Text Table - 0.33.0

**Context:** Add a keyed textual table with sticky headers, controlled interaction, bidirectional scrolling, RTL layout, and assistive-technology semantics.

**Challenge:** Rendering only visible rows while preserving stable row and column identity required layout, hit testing, runtime focus, selection anchors, scrollbar geometry, and AccessKit nodes to share one keyed coordinate model.

**Alternatives:** Embedding arbitrary widgets in cells would enable rich content but turn the table into a nested layout tree and substantially expand virtualization and focus complexity. Editable cells were also considered, but require an IME-capable overlay editor, controlled commit/cancel behavior, and additional accessibility semantics, so both remain outside the 0.33.0 textual-table scope.

**Decision:** Keep `Table` as a focused leaf widget with validated textual cells, fixed or weighted-flex columns, controlled single/multiple selection and sorting, one composite keyboard focus target, and stable derived accessibility IDs. Use an axis-aware scrollbar path shared with existing vertical scrolling while mirroring horizontal geometry in RTL.

**Files Changed:** `src/widgets/table/`, `src/widget/`, `src/layout.rs`, `src/render/table.rs`, `src/render/hit_test.rs`, `src/engine/table_runtime.rs`, `src/engine/runner/table.rs`, `src/accessibility/table.rs`, public exports, tests, the standalone table demo, documentation, and package version metadata.

**Validation:** `cargo fmt --all -- --check`; `cargo check --locked --all-targets`; `cargo test --locked --quiet` (534 library tests, 33 binary tests, integration suites, and 275 doctests); `cargo clippy --locked --all-targets -- -D warnings`; and `git diff --check`.

## [2026-08-28T00:36:37-03:00] - Descriptive Source Module Organization - Unreleased

**Context:** Reduce root-level source clutter by grouping related input, widget identity, accessibility, and multi-window modules into descriptive directories.

**Challenge:** The physical migration must retain established public paths such as `rutter::input_limits`, `rutter::input_state`, and root widget-ID exports, while preserving source-relative unit-test paths.

**Alternatives:** Replacing existing public paths with only new directory-derived paths would break downstream users and doctests. Grouping singleton cross-cutting modules under a vague umbrella would add indirection without improving cohesion.

**Decision:** Place input limits and editable-state concerns under `input/`, place core widget identity support under `widget/id/`, and use `mod.rs` roots for the existing accessibility and multi-window domains. Retain public input aliases and root widget-ID re-exports; leave cohesive singleton modules and the already-organized `widgets/` families in place.

**Files Changed:** `src/input/`, `src/widget/`, `src/accessibility/mod.rs`, `src/multi_window/mod.rs`, `src/lib.rs`, dependent engine/accessibility imports, and `README.md`.

**Validation:** `cargo fmt --all -- --check`; targeted input-state and widget-ID integration tests; `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo check --locked --all-targets --all-features`; `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked` (450 library tests, 28 binary tests, integration suites, and 194 doctests); `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo clippy --locked --all-targets --all-features -- -D warnings`; and `git diff --check`.

## [2026-08-27T23:39:52-03:00] - Canonical Text Control Rendering - 0.28.3

**Context:** Render text controls without missing-glyph boxes while retaining intentional multiline content in plain and rich text.

**Challenge:** Input, Skia drawing, Cosmic Text shaping, cache keys, and styled RichText spans previously interpreted controls independently; CRLF can also cross span or input-fragment boundaries.

**Alternatives:** Per-renderer sanitization would drift between direct drawing and shaping. Flattening every line ending would break TextArea and multiline layout. A shared stateful policy keeps canonicalization consistent while allowing line-oriented controls to flatten text.

**Decision:** Add a shared normalizer that canonicalizes line separators, tabs, and controls; preserve line breaks for multiline text and flatten them for single-line controls. Apply it at programmatic input boundaries, runner text ingestion, RichText ownership, shaping/cache keys, layout measurement, and direct Skia drawing. Measure shaped layout runs so explicit line breaks reserve their actual height.

**Files Changed:** `src/text_controls.rs`, `src/input_state_text_controls.rs`, `src/input_state_edit.rs`, `src/engine/runner.rs`, `src/layout.rs`, `src/render/text.rs`, `src/render/text_cache.rs`, `src/render/mod.rs`, `src/widgets/rich_text/owned.rs`, related one-line renderers, regression tests, `Cargo.toml`, and `Cargo.lock`.

**Validation:** `cargo fmt --all`; focused text-rendering tests (3 passed); `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo check --locked --all-targets`; `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked` (450 library tests, 28 binary tests, integration suites, and 194 doctests); `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo clippy --locked --all-targets -- -D warnings`; and `git diff --check`.

## [2026-08-26T11:21:56-03:00] - Accordion Child Layout Offset - 0.28.1

**Context:** Restore fully visible Accordion body content and responsive demo sizing.

**Challenge:** The layout reserved the Accordion header through Taffy padding while rendering, hit testing, and overlay collection applied the same vertical offset again.

**Alternatives:** Changing layout padding would disrupt the established collapsed-header geometry. Keeping the duplicate translation would retain clipped or overlapping body content. Reusing the child node's Taffy location preserves one authoritative coordinate system.

**Decision:** Remove duplicate header offsets from body rendering and interaction traversal, retain the layout-owned header reservation, and let Accordion demos fill available width up to their desktop maximum while expanded content uses automatic height.

**Files Changed:** `src/render/mod.rs`, `src/render/hit_test.rs`, `src/render/select_overlay/collector.rs`, Accordion demos, `tests/unit/select_overlay_unit_tests.rs`, `Cargo.toml`, and `Cargo.lock`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked expanded_accordion --lib` (3 passed); `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo check --locked --all-targets`; `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked` (434 library tests, 28 binary tests, integration suites, and 193 doctests); `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo clippy --locked --all-targets -- -D warnings`; and `git diff --check`.

## [2026-08-26T10:51:58-03:00] - Accelerated Counter Hold - 0.28.0

**Context:** Make held Counter decrement and increment controls change a bounded value progressively faster.

**Challenge:** Repeat changes without a busy event loop, stop reliably on pointer release or boundary, and preserve controlled-widget callbacks when a render cache briefly lags application state.

**Alternatives:** Repeating at a fixed interval is simpler but makes large bounded ranges slow. Increasing the numeric step would skip intermediate values and make controlled callbacks less predictable. Accelerating the repeat cadence retains each configured step and standard spin-button behavior.

**Decision:** Apply the first pointer adjustment immediately, wait 400 ms, then repeat at 180 ms, 100 ms, and 50 ms tiers through the existing `WaitUntil` scheduler. Cancel pending repeats on left-button release, cursor exit, focus loss, surface release, or a range boundary; accept `+` and `-` as focused Counter keyboard actions.

**Files Changed:** `src/engine/runner/counter.rs`, `src/engine/runner.rs`, `examples/widgets/counter_demo.rs`, `README.md`, `Cargo.toml`, and `Cargo.lock`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo check --locked --all-targets`; `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked` (431 library tests, 28 binary tests, integration suites, and 193 doctests); `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo clippy --locked --all-targets -- -D warnings`; and `git diff --check`.

## [2026-08-26T00:11:24-03:00] - Font-Independent Control Icons - 0.27.0

**Context:** Replace font-dependent SearchBar, Select, and accordion symbols that could render as missing or inconsistent glyphs.

**Challenge:** Preserve open/closed direction, RTL placement, anti-aliased appearance, and aligned SearchBar pointer coordinates while reserving readable gaps around icons without relying on installed font coverage.

**Alternatives:** Hardcoded SVG would be deterministic but add parsing and SVG rendering work for primitive shapes. A bundled icon font would retain glyph shaping and introduce an asset-loading dependency. Direct Skia geometry keeps the icons lightweight and independent of font availability.

**Decision:** Centralize stroked magnifier and directional-chevron primitives in a focused renderer module, use them for every symbolic glyph found in control rendering, reserve a SearchBar text inset in both rendering and pointer mapping, and clip long Select or accordion text before the icon gap.

**Files Changed:** `src/render/control_icons.rs`, `src/render/mod.rs`, `src/render/dropdown_menu_overlay.rs`, `src/widgets/search/mod.rs`, `src/engine/mod.rs`, `src/engine/runner.rs`, `tests/unit/control_icons_unit_tests.rs`, and `tests/unit/dropdown_menu_overlay_unit_tests.rs`.

**Validation:** `cargo fmt --all`; targeted control-icon, dropdown-label, input-width, and pointer-mapping tests; source scan for remaining symbolic control glyphs; `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo check --locked --all-targets`; `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked` (428 library tests, 28 binary tests, integration suites, and 193 doctests); `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo clippy --locked --all-targets -- -D warnings`; and `git diff --check`.

## [2026-08-25T23:35:08-03:00] - Integrated Search Suggestions - 0.27.0

**Context:** Extend SearchBar with ranked fuzzy or application-defined suggestions and complete pointer, keyboard, wheel, and assistive-technology interaction.

**Challenge:** Overlay rendering and hit testing must preserve original source indices while text edits, application callbacks, focus correction, clipping, dismissal, and accessibility actions continually rebuild runtime metadata.

**Alternatives:** A separate autocomplete widget would duplicate SearchBar input behavior and break API cohesion. A fuzzy-matching dependency would add transitive cost for a small deterministic scorer. Borrowing suggestion slices inside the runtime cache would tie engine metadata to each temporary view tree, so the runtime instead owns strings while the render collector uses the live borrowed slice.

**Decision:** Add validated `SearchSuggestions` to the existing SearchBar, rank accent-insensitive subsequences or custom scores without new dependencies, render a viewport-safe overlay with original-index activation, and expose an AccessKit editable combobox/listbox with stable derived IDs and guarded actions. Refresh layout before post-edit keyboard routing so callbacks cannot leave stale suggestion metadata active.

**Files Changed:** `src/widgets/search/`, `src/widget.rs`, `src/widget_id.rs`, `src/widget_id/search.rs`, `src/engine/mod.rs`, `src/engine/runner.rs`, `src/engine/runner/search.rs`, `src/render/search_overlay.rs`, `src/render/select_overlay/collector/`, `src/accessibility.rs`, `src/accessibility/search.rs`, SearchBar examples and tests, `README.md`, `Cargo.toml`, and `Cargo.lock`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; targeted SearchBar, accessibility, secondary-pointer, and widget-ID tests; `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo check --locked --all-targets`; `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked` (423 library tests, 28 binary tests, integration suites, and 193 doctests); `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo clippy --locked --all-targets -- -D warnings`; and `git diff --check`.

## [2026-08-23T18:47:11-03:00] - Timezone-aware Clock and Time Picker - 0.26.0

**Context:** Add a live, timezone-aware clock and a controlled picker for wall-clock time and IANA timezone choices.

**Challenge:** The renderer must refresh at wall-clock second boundaries without creating a redraw loop, and wall-clock scheduling must preserve DST gaps and overlaps instead of silently resolving them.

**Alternatives:** Polling every frame would redraw unnecessarily and consume idle CPU. A new engine-owned picker state would duplicate the existing Popover, Counter, and Select state paths.

**Decision:** Add a native Clock leaf with IANA/fixed-offset formatting, schedule only the next boundary through the runner, and compose TimePicker from existing controlled widgets. Represent DST resolution explicitly as Single, Ambiguous, or Nonexistent.

**Files Changed:** `src/widgets/time/`, `src/widget.rs`, `src/layout.rs`, `src/render/clock.rs`, `src/engine/mod.rs`, `src/engine/runner.rs`, plus repository-wide strict-Clippy cleanups in the engine, renderers, examples, and tests, `README.md`, `Cargo.toml`, and `Cargo.lock`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 nice -n 10 cargo check --locked --all-targets`; targeted time tests; `cargo test --doc --locked`; `cargo test --bin rutter --locked`; `cargo test --locked`; `cargo clippy --locked --all-targets -- -D warnings` now passes after cleaning every pre-existing warning; and `git diff --check`. The cleanup introduced let-chains (stabilized in Rust 1.88), so `rust-version = "1.88"` is declared in `Cargo.toml` instead of silently raising the toolchain floor.

## [2026-08-23T02:42:11-03:00] - Scrollbar Track Positioning - 0.25.1

**Context:** Make clicks anywhere on a vertical scrollbar track move the visible viewport to that location.

**Challenge:** Track geometry must match rendering for scroll views and virtual collections while preserving the existing thumb-drag coordinate system.

**Alternatives:** Page-sized track steps would retain the existing hit boundary but would not take the viewport to the requested position. Direct mapping from the pointer to the thumb center satisfies the requested placement and keeps the press continuous for dragging.

**Decision:** Treat the full rendered scrollbar track as a drag hit, compute a clamped offset from the centered thumb position, apply it before starting the existing drag, and share the state write path with cursor-driven dragging.

**Files Changed:** `src/render/hit_test.rs`, `src/engine/runner.rs`, `Cargo.toml`, and `Cargo.lock`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `CARGO_BUILD_JOBS=2 nice -n 10 cargo test --locked scrollbar`; `CARGO_BUILD_JOBS=2 nice -n 10 cargo check --locked --all-targets`; `CARGO_BUILD_JOBS=2 nice -n 10 cargo test --locked`; and `git diff --check`. Strict `cargo clippy --locked --all-targets -- -D warnings` remains blocked by 103 pre-existing lint errors outside this change.

## [2026-08-23T02:06:36-03:00] - Typed Virtual Collection Multiselection - 0.25.0

**Context:** Add controlled multiselection to virtual lists and grids without breaking their single-selection APIs.

**Challenge:** The controlled selection callback must remain synchronized with virtual viewport state so pointer hits, focus rings, keyboard navigation, scrolling, and drag selection all use the same active item.

**Alternatives:** A boolean flag cannot express the distinct single-index and full-selection callback contracts. Separate duplicate collection implementations would risk drift from existing scrolling and focus behavior.

**Decision:** Use `VirtualSelection` as a typed configuration, retain legacy constructors, synchronize configured collection viewports into existing runtime state, and implement pointer range/rectangle selection in the engine.

**Files Changed:** `src/widget.rs`, `src/engine/mod.rs`, `src/engine/virtual_selection.rs`, `src/engine/runner.rs`, `src/engine/runner/virtual_selection.rs`, `src/render/mod.rs`, `src/render/hit_test.rs`, `src/accessibility.rs`, `examples/widgets/vlist_demo.rs`, `examples/widgets/vgrid_demo.rs`, `README.md`, and multiselection tests.

**Validation:** `cargo fmt --all`; `CARGO_BUILD_JOBS=2 nice -n 10 cargo check --locked --all-targets`; focused virtual-selection tests; `CARGO_BUILD_JOBS=2 nice -n 10 cargo test --test virtual_selection_widget_tests`; `CARGO_BUILD_JOBS=2 nice -n 10 cargo test --bin rutter`; `CARGO_BUILD_JOBS=2 nice -n 10 cargo test --doc`; and `CARGO_BUILD_JOBS=2 nice -n 10 cargo test --locked`. Strict Clippy remains blocked by 101 pre-existing warnings outside this change.

## [2026-08-15T00:51:02-03:00] - Secondary Pointer Routing and Desktop Context - 0.24.1

**Context:** Correct secondary-button routing and provide coordinates suitable for native popup placement.

**Challenge:** Existing overlays must consume or dismiss right-clicks before application callbacks, while absolute client origins are unavailable on platforms such as Wayland.

**Alternatives:** Exposing raw Winit events would leak backend details; replacing the original callback would break 0.24.0 implementations. A compatibility bridge retains the logical callback while adding a framework-owned context.

**Decision:** Classify overlay ownership before context-menu targets, preserve physical cursor precision, and resolve optional physical desktop coordinates from the native client origin plus the pointer offset. Wayland, Android, iOS, and Web receive logical coordinates and scale with no desktop position.

**Files Changed:** `src/app.rs`, `src/multi_window.rs`, `src/engine/runner.rs`, `src/engine/runner/secondary_pointer.rs`, `src/engine/runner/dropdown_pointer.rs`, `src/engine/multi_runner/app_adapter.rs`, `src/lib.rs`, `README.md`, `Cargo.toml`, and `Cargo.lock`.

**Validation:** `cargo fmt --all -- --check`; `CARGO_BUILD_JOBS=2 nice -n 10 cargo check --locked --all-targets`; `CARGO_BUILD_JOBS=2 nice -n 10 cargo check --release --all-targets`; `CARGO_BUILD_JOBS=2 nice -n 10 cargo test --locked`; and `git diff --check`. Strict `cargo clippy --all-targets -- -D warnings` remains blocked by pre-existing warnings outside this change, including unchanged dropdown hover logic.

## [2026-08-13T23:56:52-03:00] - Source-aware secondary pointer events - 0.24.0

**Context:** Applications need to open native popup surfaces without clipping them to a thin source window.

**Challenge:** Widget context menus are intentionally rendered inside their source surface, while multi-window applications need the originating surface and pointer coordinates to position an independent popup.

**Alternatives:** Extending the context-menu renderer would still clip at the native surface boundary; exposing raw Winit events would leak backend details and bypass Rutter's application contract.

**Decision:** Add a logical secondary-pointer callback to `AppLogic` and a source-aware command-producing callback to `MultiWindowAppLogic`, preserving the existing in-surface context-menu priority.

**Files Changed:** `src/app.rs`, `src/multi_window.rs`, `src/engine/runner.rs`, `src/engine/multi_runner/app_adapter.rs`, `src/lib.rs`, `Cargo.toml`, and `Cargo.lock`.

**Validation:** `cargo fmt --all -- --check`; `cargo check --locked`; targeted secondary-pointer unit test; `cargo test --doc --locked`; full tests and Clippy completed before commit.
