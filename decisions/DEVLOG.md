# Development Log

## [2026-09-23T19:12:08-03:00] - Disabled Interactive Widgets - 0.38.0

**Context:** Let applications declare unavailable built-in controls without no-op messages, while retaining accessible labels and default enabled behavior.

**Challenge:** Public widget variants support struct literals, and input, overlay, focus, keyed virtual items, pointer capture, and AccessKit have separate interaction paths.

**Alternatives:** Adding a required `enabled` field to each enum variant would break existing literals. A wrapper preserves existing declarations and ID/layout structure, at the cost of explicit disabled-subtree checks across render and runtime traversals.

**Decision:** Add `Widget::enabled(bool)` as a transparent wrapper. Skip disabled subtrees in hit testing, runtime callback collection, overlay discovery and focus order, mark their AccessKit descendants disabled without actions, and draw them with the theme's composited disabled alpha. Close inactive select overlays, clear defunct slider drag and input focus on layout refresh, and suppress both direct and collection-fallback activation for individually disabled keyed virtual items. Rebuild only the addressed keyed item on keyboard selection to preserve lazy collection behavior.

**Files Changed:** `src/widget/mod.rs`, `src/widget/virtual_items.rs`, `src/widget/id/`, `src/layout.rs`, `src/engine/mod.rs`, `src/engine/runner.rs`, `src/render/`, `src/accessibility/mod.rs`, `src/theme.rs`, `src/widgets/table_of_contents.rs`, `Cargo.toml`, `Cargo.lock`, `README.md`, `decisions/DEVLOG.md`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo check --offline --all-targets` (regenerated the package version in `Cargo.lock`); `cargo check --locked --all-targets`; `cargo check --locked --all-targets --features image-rs-decoder`; `cargo test --locked --lib disabled_ -- --test-threads=2` (10 passed); `cargo test --locked -- --test-threads=2` (598 library tests, 43 binary tests, integration suites, 301 doctests passed); `cargo clippy --locked --all-targets -- -D warnings`; `git diff --check`. An earlier full test attempt failed at link time while the disk was full; after space was freed, the complete suite passed.

## [2026-09-23T13:12:28-03:00] - Fixed Placement Actions and Selectable Text Drag - Unreleased

**Context:** Keep Place/Clear actions outside the grid scroll and replace the violet card with an editable, draggable text example.

**Challenge:** Pointer regions claim primary hits before children, so wrapping a `TextInput` would prevent editing and selecting text; the input's highlighted range is runtime-owned and not available to `AppLogic` callbacks.

**Alternatives:** Exposing the input's highlighted range as a new public API would expand runtime/input semantics beyond this demo. A separate validated Select text action and drag handle preserves ordinary TextInput behavior and makes the snapshot explicit.

**Decision:** Move keyboard placement buttons above the scroll viewport, keep the typed text input independent of the pointer drag handle, and arm the latter only after explicit selection of a nonempty printable value of up to 64 UTF-8 bytes. Retain an immutable drag snapshot across input edits and source-before-target drop callbacks; use the same text value for keyboard placement, preview and retained drop. Keep the compact layout usable at narrow widths.

**Files Changed:** `examples/widgets/drag_drop_demo.rs`, `README.md`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo test --locked --bin rutter drag_drop_demo` (10 passed); `cargo test --locked --features image-rs-decoder --bin rutter drag_drop_demo` (10 passed); `cargo check --locked --all-targets`; `cargo clippy --locked --all-targets -- -D warnings`; `cargo test --locked -- --test-threads=2` (593 library tests, 43 binary tests, integration suites, and 300 doctests); `git diff --check`. An earlier `cargo test --locked --quiet` exceeded its 300-second timeout without a result; the complete suite was rerun with two test threads and passed.

## [2026-09-23T12:43:35-03:00] - Responsive Drag Grid and Yosemite Image - Unreleased

**Context:** Arrange drag sources in a responsive grid with reliable per-card pointer areas and a draggable landscape image that also appears in the drop zone.

**Challenge:** ScrollView painted children at a translated offset but hit-tested them at their original position; a wrapping row needs intrinsic content height and enough space for accessible controls on narrow windows.

**Alternatives:** Replacing the scroll view with a virtual grid would change the demo's small, stable set of pointer regions and could require separate keyed interaction plumbing. A wrapping row inside an intrinsic-height column preserves the existing IDs and lets Taffy select one or multiple columns.

**Decision:** Apply the ScrollView offset in primary hit testing, keep each tile a whole-card pointer region, and put a flex-wrapped grid and the keyboard buttons inside scrollable intrinsic-height content below the fixed drop zone. Display an NPS public-domain Yosemite Falls photo through `Widget::Image` in the source and matching drop preview; retain a separately resized 128×99 JPEG for the existing bounded badge decoder. Remove the superseded flower icon asset.

**Files Changed:** `src/render/hit_test.rs`, `examples/widgets/drag_drop_demo.rs`, `examples/widgets/yosemite_falls.jpg`, `examples/widgets/yosemite_falls_badge.jpg`, `examples/widgets/local_florist.png` (removed), `README.md`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo test --locked scroll_view_hits_the_visible_region_at_the_painted_offset --lib` (1 passed); `cargo test --locked --bin rutter drag_drop_demo` (8 passed); `cargo test --locked --features image-rs-decoder --bin rutter drag_drop_demo` (8 passed); `cargo check --locked --all-targets`; `cargo clippy --locked --all-targets -- -D warnings`; `cargo test --locked --quiet` (593 library tests, 41 binary tests, integration suites, and 300 doctests); `git diff --check`.

## [2026-09-23T11:48:42-03:00] - Readable Drag Badge Text and Licensed Flower Image - Unreleased

**Context:** Make drag badge text legible and replace the demo's featureless embedded raster with a recognizable, permissively licensed image.

**Challenge:** Widen only text badges without obscuring their drop indicator or painting beyond narrow surfaces, while preserving the existing image decoder's strict size limits.

**Alternatives:** Enlarging every badge would needlessly change status and icon feedback. Keeping the fixed circle would still truncate readable labels to roughly one character. A text-only pill preserves the existing circular badge geometry for other content.

**Decision:** Measure text into a bounded, viewport-aware pill, reserve space for the acceptance mark, and truncate by grapheme at the available painted width. Replace the coral demo's PNG bytes with Google's pinned 48×48 Material Icons `local_florist` PNG under Apache-2.0, retain the existing 20×20 raster decoding path, and document its provenance and license.

**Files Changed:** `src/render/drag_badge.rs`, `examples/widgets/drag_drop_demo.rs`, `examples/widgets/local_florist.png`, `README.md`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo test --locked drag_badge --lib` (8 passed); `cargo test --locked --bin rutter drag_drop_demo` (7 passed); `cargo test --locked --features image-rs-decoder --bin rutter drag_drop_demo` (7 passed); `cargo check --locked --all-targets`; `cargo clippy --locked --all-targets -- -D warnings`; `cargo test --locked --quiet` (592 library tests, 40 binary tests, integration suites, and 300 doctests); `git hash-object examples/widgets/local_florist.png` (upstream blob SHA verified); and `git diff --check`.

## [2026-09-23T03:14:09-03:00] - Expanded Drag Badge Examples - Unreleased

**Context:** Add hardcoded drag/drop cards that exercise each badge content option in the interactive demo.

**Challenge:** Keep the drop target and keyboard alternatives usable when adding enough colored sources to exceed the window height.

**Decision:** Define five stable-ID card presets for default status, move icon, truncated text, low-resolution embedded raster image, and copy icon badges. Decode the image once during app initialization; keep the drop zone and accessible actions above a bounded scrollable source list so long cards do not displace the target. Use the same palette for each source, badge, preview, and retained drop.

**Files Changed:** `examples/widgets/drag_drop_demo.rs`, `README.md`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo check --locked --all-targets`; `cargo clippy --locked --all-targets -- -D warnings`; `cargo test --locked --bin rutter drag_drop_demo` (6 passed); `cargo test --locked --quiet` (590 library tests, 39 binary tests, integration suites, and 300 doctests); `cargo test --locked --features image-rs-decoder --bin rutter drag_drop_demo` (6 passed); and `git diff --check`.

## [2026-09-23T02:57:05-03:00] - Configurable Drag Badge and Color-Matched Drop Zone - Unreleased

**Context:** Move drag feedback from the standalone example into an optional pointer-region API; show amber and blue cards with matching drop-zone colors, and accept icon, text, or image badge content.

**Challenge:** Draw feedback over overlays without changing hit testing, AccessKit, native cursors, or the existing unconfigured drag behavior; bound badge text and image memory independently of user-supplied assets.

**Alternatives:** Keeping a visual-only custom widget in each application would duplicate state, IDs, and layout updates. Native cursors would be platform-specific and cannot consistently depict target matching. Keeping original full-size images in active badges would retain unnecessary memory.

**Decision:** Add `DragBadge` to `PointerRegionConfig` via `.with_drag_badge`, keep it inactive without a drag source, and paint per-surface feedback after widgets and overlays from the runner's capture state. Limit text to 64 bytes and truncate by grapheme/painted width; decode raster images with a strict budget and retain only a 20×20 copy. Built-in geometric icons avoid external decoding. Update the demo to use the new source configuration and color the zone with the hovered or selected card's palette while preserving keyboard controls.

**Files Changed:** `src/pointer.rs`, `src/engine/mod.rs`, `src/engine/runner/pointer_region.rs`, `src/render/drag_badge.rs`, `src/render/mod.rs`, `src/lib.rs`, `examples/widgets/drag_drop_demo.rs`, `README.md`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo check --locked --all-targets`; `cargo clippy --locked --all-targets -- -D warnings`; `cargo test --locked pointer_region --lib`; `cargo test --locked drag_badge --lib`; `cargo test --locked --quiet` (590 library tests, 39 binary tests, integration suites, and 300 doctests); `cargo test --locked --features image-rs-decoder pointer::tests::badge_keeps_only_a_small_decoded_copy_of_an_embedded_image --lib` (1 passed); and `git diff --check`.

## [2026-09-23T02:27:25-03:00] - Non-Interactive Drag Pointer Badge - Unreleased

**Context:** Show a small drag feedback icon beside the pointer while using the drag/drop widget example.

**Challenge:** Keep the indicator above the demo's cards without allowing it to intercept hits on the drop zone or add a decorative accessibility node.

**Alternatives:** A regular text/Container badge would add a transient accessibility node. A native OS cursor change cannot express the same in-surface hover feedback without platform-specific behavior. A visual-only custom widget reuses the existing clipped drawing boundary and stays outside pointer routing.

**Decision:** Append a small absolute-positioned visual-only custom badge to the demo during active drags. Track its position from typed logical pointer events, offset it from the native hotspot, display a dot in transit and a plus over the matching target, and remove it on drop or cancellation. Keep the keyboard-accessible placement controls unchanged.

**Superseded:** The configurable, per-surface badge described above replaces the example-local custom widget while retaining its non-interactive behavior.

**Files Changed:** `examples/widgets/drag_drop_demo.rs`, `README.md`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo check --locked --all-targets`; `cargo clippy --locked --all-targets -- -D warnings`; `cargo test --locked --bin rutter drag_drop_demo` (7 passed); `cargo test --locked --quiet` (581 library tests, 40 binary tests, integration suites, and 300 doctests); and `git diff --check`.

## [2026-09-23T02:11:05-03:00] - Drag and Drop Widget Example - Unreleased

**Context:** Demonstrate how to build an in-surface drag/drop interface using typed pointer regions and application-owned opaque payloads.

**Challenge:** Show source, target, cancellation, and hover feedback without allowing a pointer-only operation or changing the existing demo launch convention.

**Decision:** Add a standalone `drag_drop` demo with two stable-ID source regions, one kind-matched target region, state-driven visual and status feedback, and accessible buttons that place the same cards. Resolve payload IDs only against known application cards before changing selection; register the demo through the standard launcher and theme tests.

**Files Changed:** `examples/widgets/drag_drop_demo.rs`, `examples/widgets/mod.rs`, `src/main.rs`, `tests/unit/all_examples_theme_unit_tests.rs`, and `README.md`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo check --locked --all-targets`; `cargo clippy --locked --all-targets -- -D warnings`; `cargo test --locked --bin rutter drag_drop_demo` (4 passed); `cargo test --locked --quiet` (581 library tests, 37 binary tests, integration suites, and 300 doctests); and `git diff --check`.

## [2026-09-23T01:52:15-03:00] - Typed Pointer Regions and In-Surface Drag - Unreleased

**Context:** Add opt-in primary-pointer events, capture, and application-owned drag/drop without changing the interaction contract of existing widgets.

**Challenge:** Carry source and matching target identity through layout changes, cancellation, overlay precedence, surface lifecycle, and per-surface runtime caches while keeping native drag handles and untrusted external payloads out of the API.

**Alternatives:** Exposing native drag/drop would permit cross-process data and require platform-specific security and ownership rules. Reusing custom widget callbacks would couple drag sources to the custom paint extension. A transparent, explicitly keyed wrapper keeps the boundary opt-in and uses ordinary application messages.

**Decision:** Introduce `Widget::pointer_region` with a required manual ID, typed logical pointer events and optional capture, plus opaque application-owned payload identifiers and kind-matched drop targets. Hit testing gives the wrapper primary-pointer ownership; the surface runner tracks capture and target enter/exit/drop phases, cancels on removal, focus loss, cursor exit, blocking overlays, and surface closure, and preserves the normal `AppLogic::update` path. AccessKit remains transparent; keyboard equivalents remain the application's responsibility.

**Files Changed:** `src/pointer.rs`, widget ID/layout/render/hit-test traversals, engine runtime caches, `src/engine/runner/pointer_region.rs`, surface shutdown routing, tests, and README.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo check --locked --all-targets`; `cargo clippy --locked --all-targets -- -D warnings`; `cargo test --locked pointer_region --lib`; `cargo test --locked --doc` (300 passed); `cargo test --locked --quiet` (581 library tests, 33 binary tests, integration suites, and 300 doctests); and `git diff --check`.

## [2026-09-20T11:50:02-03:00] - Keyed Interactive Virtual Collections - 0.37.0

**Context:** Let virtual lists, grids, and carousels opt into stable-keyed interactive child widgets without weakening the existing visual-only default.

**Challenge:** Materialize child layout, hit testing, runtime callbacks, focus identities, retained state, and AccessKit nodes only for visible and overscanned items while ensuring an item key—not its changing index—owns descendant identity.

**Alternatives:** Adding fields to visual-only variants would change their security and performance contract. Building every item to validate or retain descendants would defeat virtualization. Separate keyed constructors backed by a validated key slice preserve the old behavior and allow only the currently visible item set to participate in the runtime.

**Decision:** Add `VirtualItemKey` and `KeyedVirtualItems::try_new`, which rejects duplicate keys with both indices. New interactive list, grid, and carousel constructors scope descendant IDs by the collection path and key; reuse ephemeral layouts for child-first hit testing, painting, and accessibility; register only visible-plus-overscan callbacks; and retire scoped maps when keys leave the current view. The collection remains the row/cell fallback target outside a child control.

**Files Changed:** Keyed source API, widget identity, layout/render/hit-test paths, runtime metadata and focus collection, AccessKit generation, public exports, focused tests, documentation, and package version metadata.

**Validation:** `cargo fmt --all -- --check`; `cargo check --offline --all-targets`; `cargo check --locked --all-targets`; focused keyed interaction, runtime-bounds, identity, duplicate-key, accessibility, and Tab-order tests; `cargo clippy --locked --all-targets -- -D warnings`; `cargo test --locked` (572 library tests, 33 binary tests, integration suites, and 298 doctests); and `git diff --check`.

## [2026-09-20T01:51:09-03:00] - Bounded Multi-Window Application Wakeups - 0.36.0

**Context:** Add an event-loop-owned deadline hook so multi-window applications can make low-frequency state transitions without worker threads or raw Winit access.

**Challenge:** Merge app deadlines with existing surface schedules, propagate shared model changes without forcing redraws on unaffected surfaces, avoid past-deadline busy loops, and retain the normal validated command-routing path.

**Alternatives:** A public `EventLoopProxy` would allow unrestricted cross-thread ingress and need separate capacity and ownership semantics. Continuous redraws waste CPU and GPU work. A callback that recursively consumes every overdue interval would replay work after suspension. A single due callback per event cycle keeps the boundary bounded and lets applications select the next future deadline.

**Decision:** Add default `MultiWindowAppLogic::next_wakeup` and `wakeup` methods using `Instant`. The multi-window runner polls the canonical model on its event-loop thread, waits with the earliest future deadline, invokes one due callback per event cycle, applies returned `SurfaceCommand`s through the existing router, and synchronizes model clones without an implicit redraw. Past replacement deadlines disable waiting until a subsequent event instead of spinning.

**Files Changed:** `src/multi_window/mod.rs`, multi-window runner scheduling, deterministic wakeup tests, README documentation, and package version metadata.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo check --locked --all-targets`; `cargo test --locked wakeup --lib` (7 passed); `cargo test --locked` (564 library tests, 33 binary tests, integration suites, and 288 doctests); and `cargo clippy --locked --all-targets -- -D warnings`.

## [2026-09-20T01:18:43-03:00] - Versioned Custom Widget Boundary - 0.35.0

**Context:** Add a supported v1 extension boundary for custom Rutter leaf widgets with constrained rendering, input, local runtime state, and accessibility semantics.

**Challenge:** Extend layout, rendering, hit testing, keyboard focus, pointer capture, state reconciliation, and AccessKit without leaking the native canvas, renderer, event loop, or application state across the public API.

**Alternatives:** Exposing the window canvas directly would simplify custom painting but let callback code escape Rutter's clip and rendering lifecycle. Allowing arbitrary nested custom subtrees would add reconciliation, focus, and accessibility ownership complexity. Recording into a private picture and keeping v1 as a styled leaf preserves controlled rendering and a small compatible contract.

**Decision:** Add `Widget::Custom` with mandatory manual IDs and a versioned `CustomWidgetV1` trait. Record painting into a private Skia picture before replaying it through node clipping; retain at most 64 KiB of private runtime bytes per live ID; route typed pointer, keyboard, and approved AccessKit actions through `AppLogic::update`; and reject visual-only widgets that advertise accessibility actions.

**Files Changed:** `src/widget/custom.rs`, widget identity/validation, Taffy layout, custom rendering, hit testing, engine state and runner routing, AccessKit emission/actions, public exports, README documentation, and package version metadata.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo check --locked --all-targets`; `cargo test --locked` (556 library tests, 33 binary tests, integration suites, and 286 doctests); and `cargo clippy --locked --all-targets -- -D warnings`.

## [2026-09-19T20:54:24-03:00] - Surface-Local Application Shortcuts - 0.34.0

**Context:** Add typed, layout-aware keyboard shortcut hooks for single- and multi-window applications without exposing Winit input types.

**Challenge:** Preserve IME and text composition, focus traversal, editing commands, widget navigation, and accessibility actions while allowing matched named keys and modifier chords to reach application state through the existing Elm update path.

**Alternatives:** Returning `Option<Message>` keeps the hook minimal but forces applications to invent no-op messages to suppress key repeats. A three-state `ShortcutOutcome` keeps unmatched input transparent, dispatches typed messages normally, and permits explicit repeat consumption without a fabricated state transition.

**Decision:** Normalize Winit logical keys into framework-owned character, named, dead, and unidentified variants plus modifier and repeat state. Route only pressed events for the focused surface; reserve printable unmodified input and IME commits for text composition, then offer named and modified events to the application before existing toolkit handling. Forward the multi-window hook through the surface adapter so `SurfaceId`, revision tracking, and `SurfaceCommand` collection remain intact.

**Files Changed:** `src/app/shortcut.rs`, `src/app.rs`, `src/engine/runner/shortcut.rs`, runner event routing, the multi-window trait and adapter, root exports, focused tests, README documentation, and package version metadata.

**Validation:** `cargo fmt --all -- --check`; `cargo check --locked --all-targets`; focused shortcut tests; `cargo test --locked` (544 library tests, 33 binary tests, integration suites, and 281 doctests); `cargo clippy --locked --all-targets -- -D warnings`; and `git diff --check`.

## [2026-09-09T02:23:28-03:00] - Semantic Text Table - 0.33.0

**Context:** Add a keyed textual table with sticky headers, controlled interaction, bidirectional scrolling, RTL layout, and assistive-technology semantics.

**Challenge:** Rendering only visible rows while preserving stable row and column identity required layout, hit testing, runtime focus, selection anchors, scrollbar geometry, and AccessKit nodes to share one keyed coordinate model.

**Alternatives:** Embedding arbitrary widgets in cells would enable rich content but turn the table into a nested layout tree and substantially expand virtualization and focus complexity. Editable cells were also considered, but require an IME-capable overlay editor, controlled commit/cancel behavior, and additional accessibility semantics, so both remain outside the 0.33.0 textual-table scope.

**Decision:** Keep `Table` as a focused leaf widget with validated textual cells, fixed or weighted-flex columns, controlled single/multiple selection and sorting, one composite keyboard focus target, and stable derived accessibility IDs. Use an axis-aware scrollbar path shared with existing vertical scrolling while mirroring horizontal geometry in RTL.

**Files Changed:** `src/widgets/table/`, `src/widget/`, `src/layout.rs`, `src/render/table.rs`, `src/render/hit_test.rs`, `src/engine/table_runtime.rs`, `src/engine/runner/table.rs`, `src/accessibility/table.rs`, public exports, tests, the standalone table demo, documentation, and package version metadata.

**Validation:** `cargo fmt --all -- --check`; `cargo check --locked --all-targets`; `cargo test --locked --quiet` (534 library tests, 33 binary tests, integration suites, and 275 doctests); `cargo clippy --locked --all-targets -- -D warnings`; and `git diff --check`.

## [2026-09-03T01:17:59-03:00] - Context Menu Pre-Open Selection - 0.29.0

**Context:** Let applications select a right-clicked context-menu target before its in-surface menu overlay opens.

**Challenge:** Context-menu routing claims a secondary press before the unclaimed pointer callback, while overlay priority and multi-window state synchronization must remain unchanged.

**Alternatives:** Mutating application state from a new lifecycle hook would bypass `AppLogic::update`. Returning an optional message preserves the existing Elm-style update boundary and allows the menu to open after selection state changes.

**Decision:** Add `ContextMenuTarget` plus `context_menu_opening` hooks on single- and multi-window application traits. The runner dispatches a returned selection message through `update` before opening the target menu.

**Files Changed:** `src/app.rs`, `src/engine/runner/secondary_pointer.rs`, `src/multi_window/mod.rs`, `src/engine/multi_runner/app_adapter.rs`, public exports, documentation, and package version metadata.

**Validation:** `cargo fmt --all -- --check`; `CARGO_TARGET_DIR=/home/p_daniel/Labs/Isolated_Environment/rustLang@Projects/rutter/target CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo check --locked --all-targets`; the same environment with `cargo test --locked` and `cargo clippy --locked --all-targets -- -D warnings`; and `git diff --check`.

## [2026-08-29T20:53:15-03:00] - Responsive Widget Examples - Unreleased

**Context:** Make every standalone widget example adapt to narrow surfaces while retaining useful desktop dimensions.

**Challenge:** Fixed control widths, horizontal action rows, and popup anchors could overflow narrow windows even when their root columns already filled the surface.

**Alternatives:** Keeping fixed widths would preserve the old desktop geometry but retain narrow-window overflow. Per-demo ad hoc percentage styles would work but duplicate the same width-cap policy across every example.

**Decision:** Add an internal responsive-width style helper, use it for bounded controls and collections, wrap multi-action rows, and make the multi-window example resizable with smaller minimum dimensions. Preserve fixed dimensions only for intrinsic affordances such as icons, switches, and spinners.

**Files Changed:** `examples/widgets/layout.rs`, the affected `examples/widgets/*.rs` demos, `tests/unit/controls_demo_layout_unit_tests.rs`, and `tests/unit/multi_window_demo_unit_tests.rs`.

**Validation:** `cargo fmt --all`; `CARGO_TARGET_DIR=/home/p_daniel/Labs/Isolated_Environment/rustLang@Projects/rutter/target CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo check --locked --all-targets`; the same environment with `cargo test --locked --bin rutter`, `cargo test --locked`, and `cargo clippy --locked --all-targets -- -D warnings`; and `git diff --check`.

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
