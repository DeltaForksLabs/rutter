# Development Log

## [2026-10-02T23:07:38-03:00] - Restore X11 Keyboard Focus After Native Menus Close - Unreleased

**Context:** Continue the pending ContextMenu X11 interaction validation and correct the keyboard-focus loss exposed by the live demo.

**Challenge:** Override-redirect popups are children of the screen root, so X11's `RevertToParent` does not restore their `WM_TRANSIENT_FOR` application parent. Existing tests checked retained widget focus but not the server's input focus or subsequent native key delivery. Restoration must preserve a preceding active popup without stealing focus from another window or targeting hidden/dropped windows.

**Decision:** Keep a weak parent reference and the popup's active policy in the X11 backend. Before hiding or destroying a popup, restore keyboard focus only if that popup still owns it, choosing the preceding visible active popup or a live, server-viewable parent. Use the latest event timestamp so the server can reject restoration after a newer focus change. Reacquire focus when showing an active popup, retain inactive behavior, and restore focus on pointer-grab creation failure. Keep this in the platform backend instead of relying on WM activation requests from Rutter. Add a disposable-display regression covering popup stacks, hide/show/drop, lower-popup removal, inactive popups, hidden parents, external focus preservation, and an XTEST key delivered to the restored parent. Require actual parent input focus after selection in all three existing Rutter ContextMenu graphical fixtures.

**Files Changed:** `vendor/rutter-winit/winit-x11/src/window.rs`, `vendor/rutter-winit/winit-x11/tests/popup_keyboard_focus.rs`, `src/engine/runner/native_integration.rs`, `vendor/rutter-winit/RUTTER_FORK.md`, and `decisions/DEVLOG.md`.

**Validation:** Resource-intensive operations ran sequentially with `CARGO_BUILD_JOBS=2`.

- The new `popup_keyboard_focus` graphical regression failed before the fix because hiding the topmost popup left input focus on the root, then passed after the fix. All eight fork graphical binaries (`popup_keyboard_focus`, `popup`, `popup_movement`, `popup_dismissal`, `popup_parent_destroy`, `popup_grab_stack`, `popup_multi_pointer`, `popup_selection`) passed in separate sequential processes on disposable Xephyr `:98`, using `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --test <binary> --locked --offline -- --ignored --nocapture`.
- All seven Rutter X11 graphical fixtures passed separately with software GL and `cargo test --locked --offline --lib engine::runner::native_integration::<test> -- --ignored --exact --nocapture`, including the new server-focus assertions after ContextMenu selection. An initial invocation with unqualified exact filters matched no tests; the qualified invocations each ran one test.
- `cargo build --locked --offline --bin rutter` passed. A temporary Python/ctypes XTEST harness outside the repository exercised the rebuilt `context_menu` demo on disposable Xephyr. Screenshots and status-region comparisons confirmed Copy, nested pointer and native-keyboard Projects selection, Escape/Tab/outside-click dismissal without action changes, CTRL+O/CTRL+C/F2/Delete actions, disabled CTRL+V, and disabled Paste clicks. Parent input focus was checked after each dismissal; no manual refocusing was used between menu closure and shortcuts. The harness was corrected to select the viewable OpenGL window rather than a disappearing or unmapped Vulkan bootstrap window. Its final run passed and cleaned up the demo/display processes.
- The first full-suite invocation was interrupted before results were returned. After verifying that no validation processes remained, `cargo test --locked --offline --no-run` completed, and `cargo test --locked --offline --quiet -- --test-threads=2` passed 670 library tests, 50 binary tests, all 235 integration tests, and 307 doctests; 10 library tests were ignored by default (nine graphical fixtures and the diagnostic CPU profile).
- `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --locked --offline` passed six unit tests and two doctests; its eight graphical tests were also run explicitly above. Rutter `cargo check --locked --offline --all-targets --all-features`, Rutter `cargo clippy --locked --offline --all-targets --all-features -- -D warnings`, fork `CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --all-targets --locked --offline -- -D warnings`, affected-file `rustfmt --edition 2024`, both workspace `cargo fmt ... --all -- --check` commands, and `git diff --check` passed. Final changes and Git status were reviewed.

Bare Xephyr has no alpha compositor: screenshots show black transparent gaps between submenu panels and do not establish correct compositor presentation. No compositor-backed X11 pixel comparison, other WM policy, live multirunner focus behavior, non-Linux runtime, or assistive-technology client was tested in this continuation. Wayland display fixtures and the CPU timing diagnostic were not rerun because this correction is confined to X11 focus handling. No dependencies, public APIs, versions, lockfiles, configuration formats, or generated source files changed for this fix; preexisting changes, including the user's `.gitignore`, were preserved.

## [2026-10-01T23:10:53-03:00] - Add SVG Icons and Shortcut Labels to ContextMenu - Unreleased

**Context:** Support context-menu rows such as an SVG copy icon followed by Copy and an inline-end CTRL+C label, and demonstrate the feature in the existing widget example.

**Challenge:** Presentation metadata must preserve borrowed lifetimes, disabled behavior, recursive paths, typeahead and runtime topology. Icon and shortcut columns must fit both overlay and transparent native menus without overlapping labels, submenu arrows or scrollbars at fractional scales and in RTL.

**Alternatives:** Adding fields directly to Item/Submenu would simplify storage but break existing direct variant construction and field-exhaustive patterns. A presentation wrapper retains those field shapes and constructors, although exhaustive enum matches still need an additional branch.

**Decision:** Add `ContextMenuEntry::Decorated` with borrowed SVG bytes and shortcut text, merged by `with_svg_icon` and `with_shortcut_label`. Semantic accessors transparently unwrap presentation, and owned navigation caches omit it. Share measured per-panel command/shortcut columns through a focused `menu_row` renderer; shortcuts align to inline end, icons occupy a 16×16 logical gutter and disabled icons use reduced opacity. Reuse the existing bounded SVG validation/rasterization and persistent engine image cache in both overlay and native paths. Preserve SVG source colors and existing invalid-image fallback. Shortcuts remain display metadata; the example explicitly binds CTRL+C/CTRL+O/F2/Delete through the existing surface-local hook to status-only actions, leaves disabled Paste unbound, and performs no clipboard or filesystem operation. Existing Item/Submenu field shapes and constructor signatures are unchanged; downstream exhaustive ContextMenuEntry matches must handle Decorated or use semantic accessors.

**Files Changed:** `src/widget/mod.rs`, `src/widgets/dropdown_menu/runtime.rs`, `src/render/{menu_row.rs,dropdown_menu_overlay.rs,mod.rs,hit_test.rs}`, `src/engine/mod.rs`, `src/engine/runner/native_integration.rs`, `examples/widgets/context_menu_demo.rs`, ContextMenu entry/decoration raster/demo/engine/overlay tests, DropdownMenu mark regressions, native-menu raster fixtures, `README.md`, and `decisions/DEVLOG.md`.

**Validation:** All resource-intensive commands ran sequentially with `CARGO_BUILD_JOBS=2`. Targeted ContextMenu tests passed (51 passed, 3 graphical fixtures ignored), DropdownMenu/native-menu tests passed, and the demo passed all 7 tests covering equivalent click/shortcut actions, modifier/repeat behavior and disabled Paste. Added raster coverage checks both themes, LTR/RTL, scales 1/1.5/2, preserved SVG colors, disabled attenuation, repeated cache reuse, independent columns at each level, trailing alignment of short shortcuts, narrow-panel suppression, transparent gaps, cleared buffers and rejected SVG/raster limits. A shared-painter regression verifies checked/unchecked and selected/unselected DropdownMenu marks. The initial demo compile exposed a by-reference ShortcutEvent signature mismatch; it was corrected to the existing by-value hook before successful validation. The first full-suite invocation hit the 300-second tool timeout without reporting suite results; `cargo test --locked --offline --no-run` completed compilation and subsequent `cargo test --locked --offline --quiet -- --test-threads=2` runs passed 662 library tests, 50 binary tests, all integration suites and 307 doctests (9 graphical tests ignored by default). All 7 X11 graphical fixtures passed separately on disposable Xephyr :98 with software GL, including decorated context entries and nested selection. Both existing Wayland DropdownMenu fixtures passed separately on disposable Weston headless/pixman, covering OpenGL placement and CPU overlay fallback. `cargo check --locked --offline --all-targets --all-features`, `cargo clippy --locked --offline --all-targets --all-features -- -D warnings`, `cargo build --locked --offline --bin rutter`, `cargo fmt --all -- --check` and `git diff --check` passed. The rebuilt context_menu demo mapped an 800×600 window and remained running during a five-second isolated Xephyr startup smoke check, using the expected OpenGL fallback when Vulkan presentation was unavailable. Final diff and Git status were reviewed. Compositor screenshot comparison, dedicated decorated ContextMenu input/presentation on Wayland, graphical multirunner, Windows/macOS and live assistive-technology validation were not performed; the previously recorded native focus/configuration concerns remain outside this change. No dependencies, version/lockfile changes or generated-file edits were made; prior changes, including .gitignore, were preserved. Nothing was staged or committed.

## [2026-10-01T12:46:59-03:00] - Add Cascading ContextMenu Widget Example - Unreleased

**Context:** Add a ContextMenu demonstration alongside Dialog and Counter, with a right-click area, two submenu levels, disabled entries, separators and visible feedback for the last action.

**Challenge:** ContextMenu previously supported only flat items and separators. A genuine submenu example required framework support across retained state, geometry, painting, hit testing, keyboard/pointer input and the existing transparent native popup path. Opening callbacks can change menu topology, and retained menus must stop receiving input when their owners become hidden or disabled.

**Alternatives:** A flat-only example would omit the requested interaction. Simulating cascading menus with DropdownMenu triggers would demonstrate the wrong widget. The user explicitly approved extending ContextMenu after the missing capability and exhaustive-match compatibility concern were explained.

**Decision:** Add recursively owned `ContextMenuEntry::Submenu` entries with borrowed labels and enabled/disabled constructors while retaining the existing borrowed-slice widget constructor. Reuse DropdownMenu's entry-access contract, retained navigation, recursive geometry, painter and input transitions, with ContextMenu-specific pointer anchoring, row dimensions and theme-aware widths for every level. Native popup bounds now cover the union of all open context panels, preserving transparent clears and overlay fallback. Keep the invoking widget's focus while navigating. Reconcile runtime after the opening callback and track currently interactable owners separately to close stale menus. Forward native popup modifier changes into retained keyboard state. The example keeps its entry slice in state, starts Dark with the shared theme toggle, and only updates status text for actions; it performs no filesystem or clipboard operations.

**Files Changed:** `examples/widgets/context_menu_demo.rs`, `examples/widgets/mod.rs`, `src/main.rs`, `src/widget/mod.rs`, `src/widgets/dropdown_menu/{mod.rs,runtime.rs,geometry.rs}`, `src/engine/{mod.rs,widget_state.rs,dropdown_menu_runtime.rs,runner.rs}`, runner modules `dropdown_keyboard.rs`, `dropdown_pointer.rs`, `secondary_pointer.rs`, `native_menu.rs` and `native_integration.rs`, rendering modules `mod.rs`, `context_menu_overlay.rs`, `dropdown_menu_overlay.rs`, `hit_test.rs`, `overlay_hover.rs` and `select_overlay/collector.rs`, ContextMenu demo/engine/navigation/overlay unit tests, native-menu raster regressions, the shared example-theme test, and `README.md`. Authorized source-compatibility changes: exhaustive matches on `ContextMenuEntry` and `ContextMenuOverlayHit` must handle `Submenu`; explicit `ContextMenuState` literals must initialize the new `navigation` field or use its default. No configuration or stored-data migration is required.

**Validation:** Resource-intensive operations ran sequentially with `CARGO_BUILD_JOBS=2`. Targeted framework checks passed: `cargo test --locked --offline --lib context_menu` (41 passed, 3 graphical fixtures ignored), `--lib dropdown_menu` (67 passed), `--lib native_menu` (9 passed), and the ContextMenu doctest filters. `cargo test --locked --offline --bin rutter context_menu_demo -- --test-threads=2` passed all 4 demo tests, and the foundational theme-selector test passed. An initial full-suite invocation hit the 240-second tool timeout before producing output; `cargo test --locked --offline --no-run` then completed compilation, and `cargo test --locked --offline --quiet -- --test-threads=2` passed 651 library tests, 47 binary tests, all integration suites and 305 doctests (9 graphical tests ignored by default). All 7 X11 graphical fixtures, including nested context selection and opening-time topology changes, passed in separate Cargo processes on disposable Xephyr `:98` with `DISPLAY=:98 LIBGL_ALWAYS_SOFTWARE=1 cargo test --locked --offline --lib <test-name> -- --ignored --nocapture`. Both existing Wayland DropdownMenu fixtures passed separately on disposable Weston headless/pixman: OpenGL native placement/selection and CPU overlay fallback. `cargo check --locked --offline --all-targets --all-features`, `cargo clippy --locked --offline --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check` and `git diff --check` passed. `cargo build --locked --offline --bin rutter` passed; the built `context_menu` demo stayed running and mapped an 800×600 Rutter window on disposable Xephyr during a five-second startup smoke check, using OpenGL after the expected Vulkan presentation fallback. Final diff and Git status were reviewed. No compositor screenshot comparison, live ContextMenu submenu check on Wayland, live graphical multirunner check, Windows/macOS check or live assistive-technology check was performed. The preexisting native multirunner parent/popup focus-loss grouping concern and unverified asynchronous left-extending popup shrink behavior remain outside this feature's validation. No new dependencies, version/lockfile changes or generated-file edits were made for this task; prior changes, including `.gitignore`, were preserved. Nothing was staged or committed.

## [2026-10-01T01:44:56-03:00] - Finish Native Menu Artifact Correction and Validation - Unreleased

**Context:** Complete the correction and pending validation of the borderless region covering the main layout when dropdown submenus open.

**Challenge:** One native popup holds the rectangular union of all submenu panels. Its opaque window attributes and parent-themed clear filled the gaps between panels, obscuring underlying content even though the native renderer did not paint the parent widget tree. Validation also encountered a missing `target/debug/deps/librutter.rlib` during rustdoc execution and timed out before reporting complete doctest results.

**Decision:** Request transparent native menu surfaces and make `draw_native_menu` own the transparent clear for every popup frame. Remove the parent-themed clear from native-menu painting. Preserve existing alpha-capability checks and fall back to the in-window overlay for RGB-only CPU presentation instead of drawing an opaque popup. Add raster regressions for transparent gaps in both themes, LTR/RTL and fractional scale, and removal of collapsed-submenu pixels. Exercise the Wayland native path with OpenGL and the CPU fallback separately. Rebuild the library through Cargo and rerun doctests without changing examples or disabling tests; missing build-artifact errors no longer reproduce in the completed runs.

**Files Changed:** `src/engine/runner/{native_menu.rs,native_integration.rs}`, `src/engine/mod.rs`, `src/render/mod.rs`, `tests/unit/native_menu_render_unit_tests.rs`, `README.md`, `vendor/rutter-winit/RUTTER_FORK.md`, and `decisions/DEVLOG.md`. Focus-ring changes from the intervening task were preserved.

**Validation:** All resource-intensive commands ran sequentially. The two raster regressions failed before the correction and passed afterward. The final run of `CARGO_BUILD_JOBS=2 cargo test --locked --offline --quiet -- --test-threads=2` passed 631 library tests, 43 binary tests, all integration suites and all 302 doctests (7 display-dependent tests ignored by default). `CARGO_BUILD_JOBS=2 cargo test --locked --offline --doc --quiet -- --test-threads=1` separately passed all 302 doctests. The initial filtered doctest attempt reproduced missing-rlib compilation errors and hit a 180-second tool timeout; `cargo test --locked --offline --doc -v -- --list` rebuilt the library and listed all 302 tests before the successful complete reruns. Targeted `cargo test --locked --offline --lib native_menu_render_tests -- --test-threads=2` passed 2 tests, and `cargo test --locked --offline --lib engine::runner::native_menu::tests -- --test-threads=2` passed 4 tests. All five Rutter X11 graphical tests passed separately on disposable Xephyr `:98` with `DISPLAY=:98 LIBGL_ALWAYS_SOFTWARE=1 CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib <test-name> -- --ignored --nocapture`. The native Wayland OpenGL test and CPU fallback/selection test passed in separate Cargo processes on disposable Weston headless/pixman at 800×600. `CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets --all-features`, `CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check` passed. Final diff and Git status were reviewed. Compositor screenshot comparisons, real Wayland input/grabs, Wayland Vulkan, graphical multirunner popups, Windows/macOS and live assistive-technology clients remain outside this validation. No new dependencies, version changes, lockfile edits or generated-file edits were made for this correction; preexisting changes and the user's `.gitignore` were preserved.

## [2026-10-01T01:29:16-03:00] - Slim Shared Focus Rings - Unreleased

**Context:** Reduce the visual weight of focus rings without removing keyboard-focus feedback.

**Challenge:** The shared outline combined a 4-pixel contrast stroke with a 2-pixel accent stroke and extended 3.5 logical pixels beyond the component.

**Decision:** Halve the contrast and accent strokes to 2 and 1 logical pixels, and halve the outer offset to 0.75 pixels. Preserve existing theme-dependent colors, antialiasing, focus routing and layout dimensions. Add raster coverage for a compact but visible ring in both themes at scales 1, 1.5 and 2.

**Files Changed:** `src/render/mod.rs`, `tests/unit/focus_ring_render_unit_tests.rs`, and `decisions/DEVLOG.md`.

**Validation:** `cargo fmt --all`; `CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib focus_ring_render_tests -- --test-threads=2` (1 passed); `CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib --quiet -- --test-threads=2` (631 passed, 7 display-dependent tests ignored); `CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets -- -D warnings`; `cargo fmt --all -- --check`; `git diff --check`. No graphical screenshot comparison or full doctest rerun was performed during this styling change. The earlier doctest failure/timeout was subsequently investigated and the full suite passed, as recorded in the newer native-menu artifact validation entry. No dependencies, versions, lockfiles or generated files changed for this adjustment.

## [2026-09-30T23:36:01-03:00] - Correct Native Menu Placement on X11 and Wayland - Unreleased

**Context:** Fix native dropdowns appearing shifted left or clipped on both Linux window systems while preserving submenu selection and overlay fallback.

**Challenge:** X11 reported desktop-space popup coordinates where the runner needed parent-content coordinates, and configure notifications could restore an outdated positioner. Wayland's implicit center gravity shifted the popup; anchor-size variable shadowing also replaced the requested surface size with 1×1. Submenu resizing configures asynchronously and must not hide the menu or trigger premature fallback.

**Decision:** Report parent-content popup coordinates, synchronize X11 repositioning with its positioner, and give implicit Wayland positions top-left anchoring while preserving explicit positioners. Keep anchor and surface sizes separate. Rutter uses an explicit positioner and one requested origin for painting/input, validates configured bounds, and keeps the overlay until popup readiness with a two-second configuration deadline in both runners. Regression coverage includes an independently created Wayland popup's initial size/position, right-edge clamping, decorated/displaced X11 parents, and Wayland submenu expansion and selection.

**Files Changed:** `vendor/rutter-winit/winit-{core/src/window.rs,x11/src/window.rs,x11/tests/popup_movement.rs,wayland/src/popup.rs}`, `src/engine/runner/{native_menu.rs,native_integration.rs}`, `src/engine/{runner.rs,multi_runner.rs}`, `README.md`, `vendor/rutter-winit/RUTTER_FORK.md`, and `decisions/DEVLOG.md`.

**Validation:** Sequentially ran `cargo fmt --all`, affected-fork `rustfmt --edition 2024`, both workspace `cargo fmt ... -- --check` commands, and `git diff --check`; targeted native-menu tests (3 passed); `CARGO_TARGET_DIR=target CARGO_BUILD_JOBS=2 cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-wayland -p winit-x11 --offline` (2 Wayland unit tests, 6 X11 unit tests and 2 X11 doctests passed; graphical tests ignored by default). `CARGO_BUILD_JOBS=2 cargo test --locked --offline -- --test-threads=2` passed 627 library tests, 43 binary tests and all integration tests, then hit the tool's 240-second timeout at doctest startup; `CARGO_BUILD_JOBS=2 cargo test --locked --offline --doc -- --test-threads=2` separately passed all 302 doctests. `CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets --features image-rs-decoder` and Rutter `cargo clippy --locked --offline --all-targets -- -D warnings` passed. Fork `cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-wayland -p winit-x11 --all-targets --offline -- -D warnings` passed after moving the test module to the end of the file. On disposable Weston headless/pixman, `WAYLAND_DISPLAY=<test-socket> CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib native_wayland_dropdown_stays_at_parent_anchor_during_submenu_expansion -- --ignored --nocapture` passed. On disposable Xephyr `:98`, all seven fork popup integration binaries and all five Rutter X11 graphical tests passed in separate, sequential Cargo processes (software GL for Rutter). Wayland input is synthetic at the runner boundary; screenshot pixels, compositor-delivered Wayland input/grabs, Wayland GPU backends, graphical multirunner popups, Windows/macOS, and real assistive-technology clients remain unverified. No dependency, version, lockfile, generated-file, or user `.gitignore` changes were made for this correction.

## [2026-09-29T23:58:58-03:00] - Render Built-In Menus in Native Popups - Unreleased

**Context:** Connect Rutter's built-in dropdown and context-menu widgets to parented popup windows from the local Winit fork.

**Challenge:** Menus previously painted and hit-tested using the parent canvas and logical viewport. A popup has a separate graphics surface, physical coordinates, a different native window ID and an independent lifetime; dropdown submenus can expand the painted area while a pointer grab is active. Multisurface application focus-loss handling must not mistake a menu popup for a separate application surface.

**Alternatives:** Creating another `SurfaceRunner` for each menu would duplicate widget trees, focus, menu state and message routing. Keeping the existing state in the parent engine and rendering only its menu into a child popup preserves observable widget semantics and allows a fallback where native popups cannot be created or positioned.

**Decision:** Use a single `WindowType::Popup` per open dropdown/submenu chain or context menu, created with the parent's live raw window handle and the parent's graphics backend type. Reuse the existing menu geometry, paint it with a parent-to-popup canvas translation, route popup-local mouse, wheel and keyboard events back to the parent's widget state, and suppress that menu's in-window drawing. Reposition/resize the popup as menu geometry changes, release it before the parent on suspension or destruction, and fall back to the existing overlay if native creation or reported positioning fails. Route popup IDs through the multirunner without reporting a second application surface or closing the parent due solely to popup activation. Other overlay widgets remain in-window; X11 outside presses are consumed and are not replayed to the underlying window. Accessibility continues to publish menu nodes through the parent adapter; behavior with an assistive-technology client on the separate popup remains unverified.

**Files Changed:** `src/engine/runner/native_menu.rs`, `src/engine/runner/native_integration.rs`, `src/engine/{runner.rs,mod.rs,multi_runner.rs,multi_runner/surface_events.rs}`, `src/render/{mod.rs,dropdown_menu_overlay.rs}`, `README.md`, `vendor/rutter-winit/RUTTER_FORK.md`, and `decisions/DEVLOG.md`.

**Validation:** `cargo fmt --all` and `cargo fmt --all -- --check`; `CARGO_BUILD_JOBS=2 cargo test --locked --offline -- --test-threads=2` (625 library tests passed, 5 display tests ignored, 43 binary tests, integration tests and 302 doctests passed); `CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets --features image-rs-decoder`; `CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets -- -D warnings`; `git diff --check`. On disposable Xephyr `:98`, five separate `DISPLAY=:98 CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib <test name> -- --ignored` processes passed: the original GL/button/AccessKit test, native dropdown selection, native submenu selection, outside-click dismissal without selection and native context-menu selection. Running multiple ignored display tests in the same test process cannot recreate the Winit event loop; the tests were rerun separately. Windows/macOS/Wayland behavior, live assistive-technology clients, and a graphical multirunner popup were not tested.

## [2026-09-29T23:16:38-03:00] - Integrate Local Winit and Native Adapters in Rutter - Unreleased

**Context:** Connect the local Winit 0.31 beta, AccessKit adapter and Glutin bridge forks to Rutter and preserve its existing rendering and widget behavior.

**Challenge:** The beta replaces concrete window types, user events, surface lifecycle callbacks and mouse input with trait windows, wake-only proxies, render-surface callbacks and positioned pointer events. The new AccessKit adapter requires `accesskit 0.25.1`. The event loop now owns its `'static` application handler, so typed runtime failures cannot be read directly afterward.

**Alternatives:** Keep the registry bridges in parallel with the local Winit (would create incompatible Winit types and fail to integrate graphics/accessibility), or migrate all three dependencies together. For typed error reporting, passing a borrowed runtime to `run_app` no longer satisfies its `'static` bound; a shared completion slot populated when the owned handler drops retains the public `try_run` error contracts on blocking platforms.

**Decision:** Point Rutter at the three local path dependencies and align `accesskit` to 0.25.1 while leaving core `glutin 0.32.3` unchanged. Adapt render backends to `Rc<dyn Window>` and `surface_size()`, create/accessibility-adapt windows only in `can_create_surfaces` and release them in `destroy_surfaces`, and use positioned mouse-source pointer events without treating touch/tablet events as mouse gestures. Preserve `WindowConfig`'s public inner-size names and normalize Space shortcuts and Meta modifiers. Use the wake-only proxy for accessibility and bounded worker ingress, explicitly close ingress when its owner drops, and preserve typed runtime errors through a shared completion slot. Keep Rutter menus as in-window overlays; merely linking the X11 popup-capable fork does not turn them into native popups.

**Files Changed:** `Cargo.toml`, `Cargo.lock`, `src/app/shortcut.rs`, `src/accessibility/mod.rs`, `src/engine/{mod.rs,gpu.rs,gpu/,runner.rs,runner/,multi_runner.rs,multi_runner/}`, `src/multi_window/{ingress.rs,window_config.rs}`, `tests/unit/{multi_runner_unit_tests.rs,multi_window_unit_tests.rs}`, `README.md`, and the three fork READMEs. The user's preexisting `.gitignore` change was not modified.

**Validation:** `cargo fmt --all` and `cargo fmt --all -- --check`; `CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets`; `CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets -- -D warnings`; `CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib -- --test-threads=2` (622 passed before the final tests were added); `CARGO_BUILD_JOBS=2 cargo test --locked --offline -- --test-threads=2` (623 library tests, 43 binary tests, all integration tests and 302 doctests passed; native X11 test ignored by default); `DISPLAY=:98 LIBGL_ALWAYS_SOFTWARE=1 CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib native_mouse_selects_a_rendered_rutter_widget_with_accessibility_adapter -- --ignored --nocapture` passed on disposable Xephyr with an actual Rutter button, GL rendering, XTEST click, AccessKit adapter and native-surface recreation; `cargo tree --locked --offline -i winit` reported one Winit instance shared by Rutter and both adapters; `git diff --check`. Non-Linux runtimes, live AT-SPI clients, and native Rutter popup widgets were not tested.

## [2026-09-29T01:41:55-03:00] - Adapt Glutin Winit Bridge to Local Winit Beta - Unreleased

**Context:** Prepare Rutter's OpenGL bootstrap for `rutter-winit 0.31.0-beta.3` without integrating the Winit migration into Rutter.

**Challenge:** Published `glutin-winit 0.5.0` uses Winit 0.30 concrete windows and deprecated event-loop window creation. Winit 0.31 requires an active event loop, returns boxed trait windows and distinguishes surface size from outer window size; GLX still needs its matching X11 visual.

**Alternatives:** Vendor `glutin 0.32.3` itself (would duplicate the GL implementation even though it has no Winit dependency) or fork just `glutin-winit 0.5.0` while pinning the existing glutin release. The narrower bridge fork avoids unrelated GL changes and new registry dependencies.

**Decision:** Fork the MIT-licensed `glutin-winit` bridge under `vendor/rutter-glutin-winit`, targeting the local Winit fork and unmodified `glutin 0.32.3`. Accept `&dyn ActiveEventLoop`, return `Box<dyn Window>`, create/resize GL surfaces with `surface_size`, and preserve existing X11 platform attributes when installing the selected GLX visual. Reject incompatible platform attributes with a contextual Winit `RequestError`; retain EGL/GLX/WGL/CGL selection and nontransparent-config handling. Rutter continues using its published Winit 0.30 bridge until its GL backend and other adapters are migrated together.

**Files Changed:** `vendor/rutter-glutin-winit/` (manifest, source, MIT license, integration tests and README), `vendor/rutter-winit/RUTTER_FORK.md`, `README.md`, `decisions/DEVLOG.md`; corrected local-publication wording in `vendor/rutter-accesskit-winit/README.md`.

**Validation:** `cargo fmt --manifest-path vendor/rutter-glutin-winit/Cargo.toml --all -- --check`; `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-glutin-winit/Cargo.toml --offline` (4 unit tests, 2 doctests passed; display test ignored by default); `DISPLAY=:98 CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-glutin-winit/Cargo.toml --test display_builder --offline -- --ignored` passed on disposable Xephyr with a real GL config, window, surface and current context; `CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-glutin-winit/Cargo.toml --all-targets --offline -- -D warnings`; `CARGO_TARGET_DIR=target cargo check --manifest-path vendor/rutter-glutin-winit/Cargo.toml --lib --offline --no-default-features --features egl,x11`, `--features glx` and `--features egl,wayland`; `CARGO_TARGET_DIR=target cargo check --manifest-path vendor/rutter-glutin-winit/Cargo.toml --target x86_64-unknown-freebsd --lib --no-default-features --features egl,glx --offline`; `cargo check --locked --offline --all-targets` (Rutter); `diff -u` verified upstream MIT license; `git diff --check`. No Rutter GL frame or non-Linux graphics runtime was tested.

## [2026-09-29T01:08:26-03:00] - Fork AccessKit Winit Adapter for Local Winit Beta - Unreleased

**Context:** Prepare AccessKit's `accesskit_winit 0.34.1` adapter for the standalone `rutter-winit 0.31.0-beta.3` fork without migrating Rutter yet.

**Challenge:** Winit 0.31 replaces concrete windows/event loops with trait objects, renames inner geometry to surface geometry, removes raw-window-handle 0.5 and changes event-loop proxies from typed event delivery to wake-only signals. AccessKit 0.34.1 also requires `accesskit 0.25.1`, whereas Rutter still uses 0.24.0.

**Alternatives:** Keep only direct handlers and remove proxy constructors (smaller fork, but loses upstream event-dispatch mode); retain them using an application-owned channel plus a wake-only Winit proxy (explicit dispatch, supports mixed handlers). The latter preserves those modes without pretending Winit can transport payloads.

**Decision:** Vendor the 0.34.1 adapter with native platform modules and upstream Apache-2.0 license. Use `&dyn ActiveEventLoop`/`&dyn Window`, compute Unix surface bounds from the window origin plus `surface_position`, and send proxy-based events through `std::sync::mpsc::Sender<Event>` before calling `EventLoopProxy::wake_up`. Retain direct handler, event processing and conditional updates; use raw-window-handle 0.6 only. Do not change Rutter's current dependencies or glutin bridge.

**Files Changed:** `vendor/rutter-accesskit-winit/` (manifest, source, tests, license and documentation), `vendor/rutter-winit/RUTTER_FORK.md`, `README.md`, `decisions/DEVLOG.md`.

**Validation:** `cargo fmt --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --all -- --check`; `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --offline` (4 unit tests passed; display test ignored by default); `DISPLAY=:98 CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --test direct_handlers --offline -- --ignored` passed on disposable Xephyr; `CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --all-targets --offline -- -D warnings`; `CARGO_TARGET_DIR=target cargo check --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --lib --no-default-features --features winit/x11 --offline`; `CARGO_TARGET_DIR=target cargo check --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --target x86_64-unknown-freebsd --lib --no-default-features --features accesskit_unix,async-io,winit/x11 --offline`; `CARGO_TARGET_DIR=target cargo check --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --lib --no-default-features --features accesskit_unix,tokio,winit/x11`; `cargo check --locked --offline --all-targets` for Rutter; `diff -u LICENSE_APACHE_2.0 vendor/rutter-accesskit-winit/LICENSE-APACHE`; `git diff --check`. Full default-feature FreeBSD cross-check could not complete because `wayland-sys` needs a FreeBSD pkg-config sysroot. No AT-SPI client or other native platform behavior was exercised.

## [2026-09-29T00:34:31-03:00] - Verify Selecting an X11 Popup Option - Unreleased

**Context:** Confirm that pointer capture does not prevent selecting an option inside an X11 popup before integrating the fork with Rutter.

**Challenge:** Previous X11 integration tests covered popup creation and outside-click dismissal but did not observe an inside press/release reaching an application callback.

**Decision:** Add a standalone integration test with two menu-option hit regions in its application handler. On an isolated X11 display, inject a left click through XTEST at the second region and require its press, release, selection, no `CloseRequested`, and a still-active pointer grab. This tests the X11 event path without misrepresenting it as a Rutter widget test.

**Files Changed:** `vendor/rutter-winit/winit-x11/tests/popup_selection.rs`, `vendor/rutter-winit/RUTTER_FORK.md`, `decisions/DEVLOG.md`.

**Validation:** `rustfmt --edition 2024 vendor/rutter-winit/winit-x11/tests/popup_selection.rs`; `cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check`; `DISPLAY=:98 CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --test popup_selection --offline -- --ignored` passed on disposable Xephyr `:98`; `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --offline` (6 unit tests and 2 doctests passed; integration tests compiled and ignored by default); `CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --all-targets --offline -- -D warnings`; `git diff --check`. No actual Rutter popup widget or physical mouse was tested.

## [2026-09-29T00:16:47-03:00] - Track X11 Popup Parents and Master Pointers - Unreleased

**Context:** Address the remaining safe X11 popup lifecycle and multipointer limitations without integrating the fork into Rutter.

**Challenge:** XInput2 active asynchronous grabs cannot replay an outside press and do not affect XI2.2 touch sequences; managing multiple master pointers and nested popup grabs requires per-event-loop ownership, rollback, and device-hierarchy handling.

**Alternatives:** Global passive synchronous pointer and touch grabs could replay presses and manage touch ownership but risk conflicting with other clients' grabs, shortcuts, and touch processing. Synthetic XTEST or `SendEvent` input cannot faithfully replay the original click. The user selected retaining isolated input behavior instead of global interception.

**Decision:** Grab every enabled master pointer for the visible popup, refresh grabs when the XI2 hierarchy changes, and restore the previous visible popup when the topmost popup releases its grabs. When a parent is destroyed, hide its descendant popups, release captures, and deliver `CloseRequested` before the parent's `Destroyed` event. Explicitly document the remaining protocol and policy limits on outside-click delivery and touch capture.

**Files Changed:** `vendor/rutter-winit/winit-x11/src/{window,event_loop,event_processor}.rs`, `vendor/rutter-winit/winit-x11/tests/{popup_parent_destroy,popup_grab_stack,popup_multi_pointer}.rs`, `vendor/rutter-winit/RUTTER_FORK.md`, `decisions/DEVLOG.md`.

**Validation:** `rustfmt --edition 2024` on changed Rust files; `cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check`; `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --offline` (6 unit tests, 2 doctests, integration tests compiled); six `--offline -- --ignored` X11 integration test binaries (`popup`, `popup_movement`, `popup_dismissal`, `popup_parent_destroy`, `popup_grab_stack`, `popup_multi_pointer`) passed sequentially on disposable Xephyr `:98`; `CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --all-targets --offline -- -D warnings`; `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-common -p winit-x11 -p winit --lib --offline`; `cargo check --locked --offline --all-targets` (Rutter); `git diff --check`. No real touch hardware, other window managers, or cross-process click replay was tested.

## [2026-09-28T23:31:57-03:00] - Complete X11 Popup Pointer and Parent Tracking - Unreleased

**Context:** Add pointer capture, outside-click dismissal, and automatic parent tracking to the local X11 popup prototype before adapting the glutin bridge.

**Challenge:** Root-level override-redirect windows do not inherit parent movement or receive outside clicks; XInput2 input and X11 pointer grabs must be coordinated across popup creation, hiding, and destruction.

**Alternatives:** A core X11 pointer grab would not feed the existing XInput2 button path. Using an XInput2 master-pointer grab with events redirected to the popup keeps input handling in one place, at the cost of consuming outside presses rather than replaying them.

**Decision:** Track the current popup grab per X11 event loop, grab the XInput2 virtual core pointer for visible popups, release it on outside press, hiding, and dropping, and emit `CloseRequested` so the application controls popup lifetime. Reposition children on parent `ConfigureNotify`, including real notifications on root-parented windows. Keep independent pointers, nested-popup grab restoration, and parent-destruction cleanup outside this prototype's scope.

**Files Changed:** `vendor/rutter-winit/winit-x11/src/{event_loop,event_processor,window,popup}.rs`, `vendor/rutter-winit/winit-x11/tests/{popup,popup_movement,popup_dismissal}.rs`, `vendor/rutter-winit/winit-x11/Cargo.toml`, `vendor/rutter-winit/RUTTER_FORK.md`, `decisions/DEVLOG.md`.

**Validation:** `rustfmt --edition 2024` on affected Rust files; `cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check`; `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --offline` (6 unit tests, 2 doctests, integration tests compiled); three isolated X11 integration tests using Xephyr `:98` (`--test popup`, `--test popup_movement`, `--test popup_dismissal`, each with `--offline -- --ignored`) passed, including reruns after the final X11 code edit; `CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --all-targets --offline -- -D warnings`; `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-common -p winit-x11 -p winit --lib --offline`; `cargo check --locked --offline --all-targets` (Rutter); `git diff --check`. No end-to-end Rutter migration or glutin fork was performed.

## [2026-09-25T13:48:05-03:00] - Prototype X11 Popups in Local rutter-winit Fork - Unreleased

**Context:** Fork winit `0.31.0-beta.3` locally to experiment with native X11 popups for menus outside a parent window.

**Challenge:** X11 has no `xdg_popup`, and the existing Rutter bridges `glutin-winit 0.5.0` and `accesskit_winit 0.32.2` depend on winit `0.30`, so switching Rutter to the beta would introduce incompatible Winit instances.

**Alternatives:** Keep using the X11-specific window attributes in winit `0.30` (no cross-backend popup API), or migrate Rutter and both bridges in the same change (substantially broader compatibility work). Keep the beta fork standalone while exercising its popup API first.

**Decision:** Vendor the upstream beta as `vendor/rutter-winit`, retaining the crate names, and create X11 popups as root-level override-redirect transient windows. Reuse winit's shared positioner for initial placement and explicit repositioning; leave outside-click dismissal, automatic parent tracking, and the Rutter bridge migration to follow-up work rather than claiming Wayland-equivalent grabs.

**Files Changed:** `vendor/rutter-winit/winit-x11/`, `vendor/rutter-winit/winit-core/src/window.rs`, `vendor/rutter-winit/RUTTER_FORK.md`, `README.md`, `decisions/DEVLOG.md`.

**Validation:** `rustfmt --edition 2024` and `rustfmt --check --edition 2024` on changed Rust files; `cargo fmt --all -- --check`; `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11` (5 unit tests and 2 doctests passed); `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --test popup -- --ignored` (1 integration test passed on X11); `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-common -p winit-x11 -p winit --lib` (all library tests passed); `CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --all-targets -- -D warnings`; `cargo check --locked --all-targets` (Rutter); `git diff --check -- README.md`. The X11 experiment does not implement an outside-click grab or an end-to-end Rutter menu.

## [2026-09-24T11:33:10-03:00] - Release Selected-Text Drag and Gesture Fixes - 0.40.0

**Context:** Release the opt-in selected-text drag feature together with its word-selection and click-to-caret fixes.

**Challenge:** Synchronize the package version and generated lockfile without changing dependency resolution or including unrelated local Git changes.

**Decision:** Bump the minor version from 0.39.0 to 0.40.0 because the release adds a backward-compatible public API; keep the existing default behavior for applications that do not opt in. Regenerate the root package's lockfile version through Cargo and leave `.gitignore` outside the commit.

**Files Changed:** `Cargo.toml`, `Cargo.lock`, `decisions/DEVLOG.md`.

**Validation:** `cargo check --offline --all-targets` (updated only the root package version in `Cargo.lock`); `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo test --locked --quiet -- --test-threads=2` (621 library tests, 43 binary tests, integration suites and 302 doctests passed); `cargo clippy --locked --all-targets -- -D warnings`; `cargo check --locked --all-targets --features image-rs-decoder`; `git diff --check`. No interactive native-window gesture test was run.

## [2026-09-24T11:20:44-03:00] - Correct Word Selection and Click-to-Caret Drag Gesture - Unreleased

**Context:** Fix partial double-click selection of mixed-case words and allow a single click inside selected text to position the caret rather than start drag/drop.

**Challenge:** The editor's Unicode word selection splits `chatGPT` at its case boundary, while the selected-text drag previously published application messages on mouse press before a click could be distinguished from movement.

**Alternatives:** Starting drag on press and cancelling it on release would still expose spurious drag events and a badge. Waiting until movement to read the selection could lose the original text if application state changed. Capture the selection at press but defer side effects until movement exceeds the click tolerance instead.

**Decision:** Expand the editor's double-click word selection over contiguous Unicode letters, numbers and underscores without crossing punctuation. Arm a selected-text snapshot on press, initiate the existing drag/drop lifecycle after five logical pixels of movement, and otherwise collapse the selection at the original press position on release. Treat the click immediately after a double-click as a single click; discard an armed gesture on focus or cursor loss.

**Files Changed:** `src/input/state/mod.rs`, `src/engine/runner.rs`, `src/engine/runner/pointer_region.rs`, `src/app.rs`, `README.md`, `decisions/DEVLOG.md`.

**Validation:** `cargo test --locked --lib double_click_in_middle_selects_entire_word -- --test-threads=2` (first reproduced failure, then passed); `cargo test --locked --lib selected_text -- --test-threads=2` (4 passed); `cargo test --locked --lib -- --test-threads=2` (620 passed); `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo clippy --locked --all-targets -- -D warnings`; `cargo test --locked --quiet -- --test-threads=2` (621 library tests, 43 binary tests, integration suites and 302 doctests passed); `cargo check --locked --all-targets --features image-rs-decoder`; `git diff --check`. No interactive native-window gesture test was run.

## [2026-09-24T01:58:32-03:00] - Drag Highlighted Text Directly From Input - Unreleased

**Context:** Replace the drag/drop demo's Select text button and separate handle with a drag that begins from a highlighted word in the editable text field.

**Challenge:** Pointer regions take hits before their children, so wrapping the input would break cursor placement and double-click selection; the highlighted substring lives in engine-owned editor state rather than in application messages.

**Alternatives:** Keeping a dedicated drag handle would retain the extra step the user wants removed. Wrapping the input in a normal pointer region would intercept editing. An opt-in application hook leaves ordinary editor hits untouched and reuses existing drag capture and target routing only when a press lands inside a valid selection.

**Decision:** Add defaulted `selected_text_drag` hooks for single- and multi-window apps. On a press inside a selected range, check the editor's shaped hit position, UTF-8 boundaries and the 64-byte badge limit; never expose password selections. Send the owned substring to the application before starting drag capture, paint a text badge, and retain existing drop, cancellation and accessibility behavior. In the demo, drag the substring from the input itself and use the current validated draft for keyboard Place Text.

**Files Changed:** `src/pointer.rs`, `src/app.rs`, `src/multi_window/mod.rs`, `src/engine/multi_runner/app_adapter.rs`, `src/input/state/mod.rs`, `src/engine/runner.rs`, `src/engine/runner/pointer_region.rs`, `src/lib.rs`, `examples/widgets/drag_drop_demo.rs`, `README.md`, `decisions/DEVLOG.md`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo test --locked --lib selected_text -- --test-threads=2` (4 passed); `cargo test --locked --lib selected_drag -- --test-threads=2` (1 passed); `cargo test --locked --bin rutter drag_drop_demo -- --test-threads=2` (10 passed); `cargo test --locked -- --test-threads=2` (616 library tests, 43 binary tests, integration suites and 302 doctests passed); `cargo clippy --locked --all-targets -- -D warnings`; `cargo check --locked --all-targets --features image-rs-decoder`; `git diff --check`. No interactive visual drag was run.

## [2026-09-24T00:48:40-03:00] - Bounded Multi-Window Worker Ingress - 0.39.0

**Context:** Let application-owned platform watchers deliver typed, surface-targeted updates without polling or exposing Winit/graphics resources to workers.

**Challenge:** Enforce a bounded nonblocking queue and coalesced native wakeups while preserving FIFO delivery, event-loop-only application callbacks, deterministic shutdown, and existing non-`Send` multi-window callers.

**Alternatives:** A periodic application deadline would delay platform event delivery and cannot wake the loop when a worker finishes. A public Winit proxy or unbounded channel would leak event-loop details or permit uncontrolled queue growth. An opt-in, preallocated, count-bounded queue keeps these concerns inside Rutter, while applications remain responsible for bounding payload sizes.

**Decision:** Create an opt-in `try_run_with_state_and_message_ingress` startup path after constructing the private Winit proxy. Sender clones use a nonblocking queue-lock attempt, return `Busy`/`Full`/`Closed`, and emit one native event per queued burst. The runner drains at most a configured budget per event, invokes the existing surface adapter update/command route after releasing the queue lock, discards retired-surface messages, and closes/discards ingress before destroying UI state. The ordinary startup path has no queue or additional `Send` bound.

**Files Changed:** `src/multi_window/ingress.rs`, `src/multi_window/mod.rs`, `src/engine/multi_runner.rs`, `src/engine/multi_runner/startup.rs`, `src/engine/runner.rs`, `src/lib.rs`, `tests/unit/multi_runner_unit_tests.rs`, `README.md`, `Cargo.toml`, `Cargo.lock`, `decisions/DEVLOG.md`.

**Validation:** `cargo fmt --all`; `cargo fmt --all -- --check`; `cargo check --offline --all-targets` (synchronized `Cargo.lock` to 0.39.0); `cargo test --locked --lib ingress -- --test-threads=2` (9 passed); `cargo test --locked --lib external_delivery -- --test-threads=2` (1 passed); `cargo test --locked --lib ordinary_multi_window_startup -- --test-threads=2` (1 passed); `cargo test --locked --lib invalid_ingress_configuration -- --test-threads=2` (1 passed); `cargo test --locked -- --test-threads=2` (609 library tests, 43 binary tests, integration suites, 302 doctests passed; repeated at 0.39.0); `cargo clippy --locked --all-targets -- -D warnings`; `cargo check --locked --all-targets --features image-rs-decoder`; `git diff --check`. No interactive native-window/worker integration run was performed.

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
