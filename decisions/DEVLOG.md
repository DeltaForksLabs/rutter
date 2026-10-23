# Development Log

## [2026-10-22T22:00:14-03:00] - Release Native Widget Popups with the Requested Commit Amend - 0.42.0

**Context:** Include the project version increment in the last native-popup/menu-reconciliation commit using the user's explicitly requested amend.

**Challenge:** Synchronize generated package metadata without changing dependency resolution, preserve the previous commit's complete file descriptions and breaking-change notice, and avoid including the preexisting `.gitignore` edit.

**Decision:** Advance Rutter from 0.41.0 to 0.42.0 as the next pre-1.0 minor release for the new popup API. This is not a claim of full source compatibility: downstream exhaustive matches need the new `MultiWindowRunError::Popup` arm, and shutdown now notifies `surface_closed` for remaining registered surfaces. Change the authoritative root manifest and let Cargo regenerate only the root package version in the lockfile. Update current popup documentation, retain historical DEVLOG versions and independent fork versions, and prepare the authorized amend with an English per-file message and its existing `BREAKING CHANGE` footer. No registry publication, tag or push is requested.

**Files Changed:** `Cargo.toml`, Cargo-regenerated `Cargo.lock`, `docs/NATIVE_POPUPS.md`, `vendor/rutter-winit/RUTTER_FORK.md`, and this log. The earlier implementation, tests and example remain part of the amended commit without further source edits.

**Validation:** All resource-intensive commands ran sequentially:

```bash
CARGO_BUILD_JOBS=2 cargo check --offline --all-targets --all-features
cargo fmt --all -- --check
cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check
CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets --all-features -- -D warnings
CARGO_PROFILE_TEST_DEBUG=0 python3 /tmp/opencode/with_xvfb.py cargo test --locked --offline --all-features -- --test-threads=2
CARGO_BUILD_JOBS=2 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --offline --example native_popup_clock
git diff --check
git diff --exit-code -- ':(glob)vendor/**/Cargo.toml' ':(glob)vendor/**/Cargo.lock'
```

Check and Clippy passed at 0.42.0; both formatting checks passed. The full root suite passed 712 library tests (35 ignored), 50 binary tests, 235 integration tests and 308 doctests; the example's two tests passed separately. The owned Xvfb wrapper sets two Cargo build jobs, clears inherited display/session/DBus variables and supplies the isolated clipboard service needed by engine tests. Complete suite results were inspected. The manifest/lockfile diff contains only the root version increment; dependency resolution and fork manifests/lockfiles are unchanged. Source/docs diffs, Git status and staging scope were reviewed for the requested amend.

Fork suites, live compositor smoke, non-Linux builds and registry packaging were not rerun for this metadata/documentation-only increment; the previous entry records the actual implementation validation and remaining graphical/FRAYM gaps. There are no new dependencies, runtime changes or generated source edits in this release follow-up. The generated root lockfile changed only its Rutter version. The preexisting `.gitignore` edit remains outside the commit, and local ignored issue references must be synchronized to the replacement commit hash after amend. No push is performed.

## [2026-10-22T20:36:11-03:00] - Reconcile Published Menu Models and Expose Native Widget Popups - Unreleased

**Context:** Implement the two remaining local issues for stale native-menu widget-ID snapshots and arbitrary-content parented popups without replacing native menus or layer-shell panels.

**Challenge:** A popup can receive input before its parent's queued redraw reconciles a published model. Arbitrary widget surfaces also need truthful native parenting, configured geometry, input-grab authorization and child-first lifetime without leaking raw handles to applications. The live clock probe exposed a second bug: multi-window post-event scheduling replaced a live clock's wakeup with idle `Wait`.

**Decision:** Invalidate every runner at shared-model publication, reconcile widget ownership/state/layout before menu consumers, retain strict same-state reconstruction checks, and track layout revisions even when another path has consumed the dirty flag. Revoke stale hit/paint state and retire removed/changed openers instead of retargeting actions. Add validated `PopupConfig`/`WindowConfig::with_popup` requests using registered parent IDs and ordinary per-surface widget engines. Preserve `SurfaceRequest` fields and `SurfaceCommand` variants, enforce logical/physical buffer budgets and native-role checks, arbitrate one application transient, close/recreate on parent geometry changes, and discard children before parents on closure/suspension/shutdown. Extend the Winit fork with default-compatible `PopupGrab::Auto`, strict `Required` and explicit `None`: required grabs consume the exact live parent/seat press token before any native allocation; release, replacement input, callback return and focus/capability/seat loss revoke it. Legacy serial choice follows receipt order, never numeric magnitude. Aggregate live-clock and smooth-scroll redraw deadlines so post-event scheduling preserves both. Keep panel focus policy unchanged in the runtime; the example explicitly selects on-demand input because labwc 0.7.1 otherwise withheld Escape from its grabbed layer-owned popup.

**Files Changed:** `src/engine/{mod.rs,runner.rs,multi_runner.rs,test_surface.rs}`, native-menu reconciliation/presentation and tests, multi-runner popup/publication/lifecycle/metrics modules and tests, `src/multi_window/{mod.rs,window_config.rs,popup.rs}`, public exports, popup configuration tests, `examples/native_popup_clock.rs`, `tests/native/native_popup_smoke.py`, the Winit core/Wayland popup/event-loop/seat modules, `vendor/rutter-winit/RUTTER_FORK.md`, `README.md`, `docs/NATIVE_POPUPS.md`, the two local issue status reports and this log. The shared in-memory/raster test fixture lives in `engine/test_surface.rs` rather than being compiled as duplicate modules.

**Validation:** Resource-intensive operations ran sequentially, with two Cargo build jobs and reduced test debug information where shown. Graphical input targeted only owned disposable displays, not the inherited desktop. Final successful commands included:

```bash
CARGO_PROFILE_TEST_DEBUG=0 python3 /tmp/opencode/with_xvfb.py cargo test --locked --offline --all-features -- --test-threads=2
CARGO_BUILD_JOBS=2 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --offline --example native_popup_clock
CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets --all-features
CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets --all-features -- -D warnings
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --offline --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-wayland -p winit-x11
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo clippy --locked --offline --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-wayland -p winit-x11 --all-targets -- -D warnings
cargo fmt --all
cargo fmt --all -- --check
cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check
PYTHONPYCACHEPREFIX=/tmp/opencode/pycache python3 -m py_compile tests/native/native_popup_smoke.py
```

- The temporary Xvfb wrapper sets `CARGO_BUILD_JOBS=2`, clears inherited display/session/DBus variables and provides an isolated clipboard display for existing engine initialization. The complete final root output was inspected: 712 library tests passed (35 ignored), 50 binary tests, 235 integration tests and 308 doctests passed. Eight menu reconciliation regressions cover both menu types and withheld parent redraws; three publication/scheduling regressions cover cross-surface/external/wakeup paths and preservation of the live-clock deadline. Two example tests passed separately.
- The selected Linux fork tests passed 43 unit tests (16 core, 13 Wayland, 14 X11) and 18 doctests. Eleven graphical fixtures and four doctests remained default-ignored. Root all-target/all-feature check/Clippy, selected fork all-target Clippy, root/fork format checks and Python syntax compilation passed. Literal Winit workspace all-features and non-Linux builds were not rerun: the previous entry records the Apple-only dependency incompatibility on this Linux host. No equivalence to that feature/platform matrix is claimed.

```bash
CARGO_BUILD_JOBS=2 CARGO_PROFILE_TEST_DEBUG=0 cargo build --locked --offline --profile test --example native_popup_clock
LD_LIBRARY_PATH=/tmp/opencode/graphics-tools/usr/lib/x86_64-linux-gnu python3 tests/native/native_popup_smoke.py --labwc /tmp/opencode/graphics-tools/usr/bin/labwc --xvfb /tmp/opencode/graphics-tools/usr/bin/Xvfb --wlr-randr /tmp/opencode/graphics-tools/usr/bin/wlr-randr --keep
git diff --check
git diff --exit-code -- ':(glob)**/Cargo.toml' ':(glob)**/Cargo.lock' Cargo.toml Cargo.lock
```

- The final smoke rerun passed 12 scenarios (top/bottom/left/right at 1x/1.25x/2x), each with three opens. Real Wayland protocol traces establish `xdg_popup`/grab/layer-parent association, configured 384x448 bounds and no additional layer roles for children. Pixel sampling verifies clock ticking, month navigation and day selection; Escape, outside-client dismissal, explicit Close and reopening passed. XTEST was sent only to the owned Xvfb host; labwc delivered Wayland seat input. Final logs: `/tmp/opencode/rutter-popup-3iwp7qkm`; the preceding successful matrix is in `/tmp/opencode/rutter-popup-4_2irr0u`. Runtime tools/libraries were extracted under `/tmp/opencode`, not added as project dependencies or installed globally.
- Intermediate failures were not counted as successes: zero-area/not-invalidated test menu fixtures caused two publication regressions to fail before fixture correction; initial smoke attempts lacked the clipboard display, exposed the clock deadline bug and the noninteractive-parent Escape limitation, and assumed only one layer role despite backend probing. An example test initially referred to a nonexistent calendar enum variant. Clippy identified duplicate fixture modules, large enum variants, a collapsible conditional and a test arithmetic lint; shared fixture placement and boxed private settings/metadata fixed them without suppressions. The last formatting check found only the example import wrap; formatting and final checks then passed.

Remaining live gaps are ordinary toplevel/X11 parents through the new API, non-grabbing mode, every corner/constraint combination, multiple-parent/menu arbitration, parent closure, suspend/shutdown and FRAYM. The example's `(0, 0)` anchor is not all corner coverage. In-memory menu ordering/lifecycle tests are not a live Quasar/FRAYM reproduction. FRAYM's previously recorded layer-shell configure stall remains uncorrected; passing labwc is not FRAYM acceptance. Non-Linux execution, physical output hotplug and real multi-monitor DPI transitions were not validated. The inherited synchronous Wayland initial-configure wait still has no added timeout.

Compatibility: popup presentation is opt-in, existing generic popovers remain in-surface, and legacy Winit grab defaults remain `Auto`. The new `MultiWindowRunError::Popup` variant requires downstream exhaustive error matches to add an arm; shutdown now notifies `surface_closed` for remaining registered surfaces. Required grabs intentionally reject deferred/release-triggered opens; callers can explicitly request non-grabbing behavior with its documented focus/dismissal limits. No Cargo dependencies, versions, lockfiles or generated source files changed; root remains 0.41.0. DEVLOG and local issue statuses were updated. The ignored `issues/` reports remain local; new fix hashes and `[DONE]` renames are deferred until actual commits exist. The preexisting `.gitignore` change was preserved. Nothing was staged, committed or pushed.

## [2026-10-12T23:11:30-03:00] - Validate Locked Linux Feature Sets Across All Three Forks - Unreleased

**Context:** Verify the current uncommitted PanelConfig/X11/EWMH worktree with online dependency access and strict fork Clippy/test checks on the Linux host.

**Challenge:** Winit's literal selected-package all-features set enables Apple-only Foundation dependencies through winit-common; the supported Linux test build also exhausted the filesystem before tests could run.

**Decision:** Preserve manifests and locked resolution. Attempt the requested all-features commands, then explicitly enable the supported Linux features without winit-common's core-foundation/foundation or Winit's foreign-platform features. Keep all Clippy warnings denied. Test AccessKit's async-io and Tokio runtime choices separately; their combined all-features set is intentionally incompatible and was not invoked. After the disk failure, inspect processes (no Cargo/compiler/test process remained), remove only the rebuildable target/debug/incremental cache (11 GiB), and rerun the same supported Winit test command successfully. All resource-intensive operations ran sequentially with CARGO_BUILD_JOBS=2 and CARGO_TARGET_DIR=target; no subagents. Test builds used CARGO_PROFILE_TEST_DEBUG=0 and two test threads.

**Files Changed:** Only this DEVLOG entry during this verification task. No production/test source fix was needed; all earlier source, documentation and .gitignore changes were preserved.

**Validation:** Executed from the repository root with rustc 1.97.0-nightly (20de910db 2026-05-02) and cargo 1.97.0-nightly (4f9b52075 2026-05-01). These exact commands were run, in the order below:

```bash
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-common -p winit-wayland -p winit-x11 -p winit -p dpi --all-targets --all-features --locked -- -D warnings
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-common -p winit-wayland -p winit-x11 -p winit -p dpi --all-features --locked -- --test-threads=2
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-common -p winit-wayland -p winit-x11 -p winit -p dpi --all-targets --features winit-core/serde,winit-common/event-handler,winit-common/positioner,winit-common/wayland,winit-common/x11,winit-common/xkb,winit-wayland/serde,winit-x11/serde,dpi/std,dpi/serde,dpi/mint,winit/serde,winit/mint,winit/x11,winit/wayland,winit/wayland-dlopen,winit/wayland-csd-adwaita,winit/wayland-csd-adwaita-cosmic-text,winit/wayland-csd-adwaita-crossfont,winit/wayland-csd-adwaita-notitle,winit/wayland-csd-adwaita-notitlebar,winit/wayland-csd-adwaita-skrifa --locked -- -D warnings
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-common -p winit-wayland -p winit-x11 -p winit -p dpi --features winit-core/serde,winit-common/event-handler,winit-common/positioner,winit-common/wayland,winit-common/x11,winit-common/xkb,winit-wayland/serde,winit-x11/serde,dpi/std,dpi/serde,dpi/mint,winit/serde,winit/mint,winit/x11,winit/wayland,winit/wayland-dlopen,winit/wayland-csd-adwaita,winit/wayland-csd-adwaita-cosmic-text,winit/wayland-csd-adwaita-crossfont,winit/wayland-csd-adwaita-notitle,winit/wayland-csd-adwaita-notitlebar,winit/wayland-csd-adwaita-skrifa --locked -- --test-threads=2
```

- The first two commands failed in objc2 0.6.4 with its explicit Apple-platform compile error, not a panel regression. The Linux feature command excludes the unsupported common Foundation features while retaining event-handler/positioner/XKB, X11/Wayland, serde/mint and every listed Wayland CSD option. Clippy passed. The supported test command initially failed building skrifa with No space left on device; after the cache cleanup it was repeated verbatim and passed.
- Winit results: 58 unit tests passed (dpi 19, common 7, core 15, Wayland 3, X11 14), 14 integration tests passed (Send 6, serde 3, Sync 5), and 20 doctests passed (Winit 2, core 16, X11 2). Eleven graphical tests and eleven doctests were default-ignored. No ignored test was enabled.
- The initial online command successfully downloaded cosmic-text 0.19.0, mint 0.5.9, crossfont 0.9.0, freetype-rs 0.36.0, freetype-sys 0.20.1 and yeslogic-fontconfig-sys 6.0.1. These are existing locked dependencies now cached, not newly introduced dependencies or resolution changes.

```bash
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --all-targets --locked -- -D warnings
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --locked -- --test-threads=2
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --all-targets --no-default-features --features accesskit_unix,tokio,winit/x11,winit/wayland --locked -- -D warnings
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --no-default-features --features accesskit_unix,tokio,winit/x11,winit/wayland --locked -- --test-threads=2
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-glutin-winit/Cargo.toml --all-targets --all-features --locked -- -D warnings
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path vendor/rutter-glutin-winit/Cargo.toml --all-features --locked -- --test-threads=2
```

- Both AccessKit Clippy variants passed. Each test variant passed four unit tests with zero doctests; each default-ignored its one graphical direct_handlers integration fixture. That fixture creates a real hidden X11 window and is not a pure fake-handle test; the passing proxy/bounds unit tests are independent of a production display.
- Glutin all-target/all-feature Clippy passed. Tests passed four unit tests and two doctests; one graphical display_builder integration fixture remained default-ignored. Linux all-features compilation does not validate Windows WGL or other foreign-platform execution.

```bash
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo fmt --all -- --check
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo fmt --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --all -- --check
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo fmt --manifest-path vendor/rutter-glutin-winit/Cargo.toml --all -- --check
git diff --exit-code -- ':(glob)**/Cargo.toml' ':(glob)**/Cargo.lock' Cargo.toml Cargo.lock vendor/rutter-winit/.cargo/config.toml vendor/rutter-winit/rustfmt.toml
git diff --check
```

All four formatting checks, the manifest/lockfile/config unchanged diff check and whitespace check passed. SHA-256 hashes before and after validation matched for .gitignore, the root manifest/lockfile and all three fork manifests/lockfiles, including the Git-ignored fork lockfiles. Final Git status and diff were reviewed. No root test/check/Clippy rerun was needed because this task changed no production/test source; the previous entry records the root suite's 683 library, 50 binary, 235 integration and 308 doctest passes and is not a new execution here.

No native smoke test, production desktop session, non-Linux native workspace package, cross-compilation, target installation or optional FreeBSD branch was run. The runtime-less AccessKit optional branch was not checked. This verifies the listed combined Linux compile/test feature sets, not every independent CSD/backend permutation, live compositor/AT-SPI/GL behavior, physical hotplug, fractional-DPI transitions or all platforms. Existing live-validation gaps remain. No dependencies, versions, migrations, lockfiles, configs or generated source files changed; only incremental build cache was removed. Root remains 0.41.0. DEVLOG was updated, .gitignore was preserved, and nothing was staged or committed.

## [2026-10-12T17:03:18-03:00] - Verify Final Native Panel Worktree - Unreleased

**Context:** Run the full root suite after the terminal-dispatch and neutral Wayland reservation review corrections.

**Challenge:** Earlier full-suite results preceded the final localized fixes; the first final quiet invocation exceeded its 600-second tool timeout without producing output.

**Decision:** Inspect running processes before retrying; no Cargo, compiler or test process from that invocation remained. Rerun without quiet output to obtain an explicit final test result, without changing source or claiming a cause for the timeout.

**Files Changed:** This log; the reviewed panel implementation and contracts are detailed in the entries below.

**Validation:** Sequential `CARGO_BUILD_JOBS=2 cargo test --locked --offline --all-features -- --test-threads=2` passed 683 library tests (35 ignored), 50 binary tests, 235 integration tests and 308 doctests. The preceding identical invocation with `--quiet` timed out and is not counted as success. `git diff --check` and the manifest/lockfile unchanged check passed; final Git status was inspected. Existing graphical and cross-platform gaps remain unchanged. No versions, dependencies, lockfiles, generated files, staging or commits changed; the preexisting `.gitignore` edit remains untouched.

## [2026-10-12T16:44:04-03:00] - Include Anchored Margin in Neutral Wayland Reservations - Unreleased

**Context:** Correct neutral Some(n) reservation translation so Wayland receives the promised base distance plus anchored margin.

**Challenge:** The cached layer config held only the base zone, so Some(32) with margin 8 transmitted Reserve(32) instead of Reserve(40), despite either setter order passing range validation.

**Decision:** Translate a validated clone at layer_shell_config() using the base plus the current anchored margin and LayerShellConfig's validated exclusive-zone builder. Reuse one anchored-margin selector for translation and range validation. Private validated fields guarantee a positive sum fitting i32; the justified expect asserts that invariant, without a new signed cast or unsafe code. Keep reservation() returning the base, None remaining nonreserving with margins, and explicit LayerShellConfig distances unchanged. Review and preserve X11 rounding: base and anchored gap round independently before physical addition, matching rounded thickness/gap when the base equals thickness; document and test the possible one-pixel difference from rounding their logical sum at fractional scale. Wayland receives the integer logical sum, with physical rounding controlled by its compositor.

**Files Changed:** `vendor/rutter-winit/winit-core/src/window/panel.rs`, the rounding regression in `vendor/rutter-winit/winit-x11/src/panel.rs`, `src/engine/multi_runner/layer_shell_integration.rs`, `tests/native/layer_shell_smoke.py`, `docs/NATIVE_PANELS.md`, and this log.

**Validation:** Resource-intensive operations ran sequentially with CARGO_BUILD_JOBS=2; no agents/subagents.

- `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-x11 -p winit-wayland --locked --offline --quiet` passed 32 unit tests (15 core, 14 X11, 3 Wayland) and 18 doctests; eleven graphical fixtures and four doctests were default-ignored. New regressions cover all four edges, compact/stretch, both setter orders, base accessor preservation, only the anchored margin contributing, margins with None, resetting Some to None, the exact signed maximum, over-bound/zero/u32::MAX rejection, explicit layer-shell distance preservation, and independent X11 rounding at scale 1.5.
- `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-x11 -p winit-wayland --all-targets --locked --offline -- -D warnings` passed.
- `CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets --all-features` and `CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets --all-features -- -D warnings` passed.
- `CARGO_BUILD_JOBS=2 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --offline --lib neutral_panel_clone_defaults_and_conflicting_hints -- --test-threads=2` passed one root configuration regression.
- `CARGO_BUILD_JOBS=2 CARGO_PROFILE_TEST_DEBUG=0 LD_LIBRARY_PATH=/tmp/opencode/rutter-layer-compositor/usr/lib/x86_64-linux-gnu python3 tests/native/layer_shell_smoke.py --labwc /tmp/opencode/rutter-layer-compositor/usr/bin/labwc --wlr-randr /tmp/opencode/rutter-layer-compositor/usr/bin/wlr-randr --remove-output --retained-renderer` passed. The changed neutral probe renders a real compact Top panel with base 32 and gap 8; its captured client trace confirms set_margin(8, 0, 0, 0), set_exclusive_zone(40), and only one allocated layer role after activation-token rejection. Weston unsupported-role/ordinary-window creation and retained OpenGL targeted-output-removal cleanup also passed. Logs: `/tmp/opencode/rutter-layer-ygnqpv2f`.
- Targeted Rust formatting ran; root/fork cargo fmt checks, final source/docs review, git diff --check, Git status and unchanged manifest/lockfile checks completed.

The new live probe verifies transmitted protocol values and rendering, not pixel-measured gap/work-area behavior on all edges or fractional-scale Wayland compositor rounding. Those remain gaps; all-edge/setter-order/boundary behavior and X11 rounding are pure regressions. Full root tests and X11 graphical smoke were not rerun for this localized translation fix; selected fork tests, targeted root configuration, root check/Clippy and the expanded Wayland smoke were run. No new dependencies, versions, lockfiles, generated files or migrations; public base reservation and explicit layer-shell semantics remain compatible. DEVLOG and contract documentation changed. Root remains 0.41.0, preexisting `.gitignore` edits were untouched, and nothing was staged or committed.

## [2026-10-12T16:33:47-03:00] - Close Remaining X11 Panel Retirement and Scale Retry Gaps - Unreleased

**Context:** Fix the three concrete P2 blockers from the second panel source review.

**Challenge:** Suppressed FocusOut leaves stale active keyboard routing; later ancestor destruction can repeat close callbacks for retained terminal descendants; successful retries can overwrite monitor scale before publishing its transition.

**Decision:** Clear active_window, repeat/composition and filtered-modifier state when terminal tree focus is revoked, before withdrawal or callbacks. Guard active-based core-key and modifier dispatch against terminal targets while still advancing global XKB/modifier state. Treat tombstoned DestroyNotify as route/IME cleanup only, independently of weak-owner upgrade, and exclude terminal descendants from ordinary dismissal both during collection and before callbacks. Route normal, mapping and retry outcomes through one scale notifier with a per-panel last-published scale distinct from last_monitor; publish only successful committed snapshots, once per transition, with the SurfaceSizeWriter alive during synchronous dispatch. Store the validated scale/size pair after checked native writes and flush, avoiding a fresh geometry read racing redirected WM configures. Immutable validated panel geometry still wins over writer resize requests.

**Files Changed:** `vendor/rutter-winit/winit-x11/src/{event_processor.rs,event_loop.rs,panel.rs,window.rs}`, the root X11 graphical fixture, `tests/native/x11_panel_smoke.py`, `docs/NATIVE_PANELS.md`, and this log. Root production code did not change.

**Validation:** All resource-intensive commands ran sequentially with two Cargo build jobs; no agents/subagents. The owned-display driver cleared inherited display/session/DBus addresses.

- `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-x11 -p winit-wayland --locked --offline --quiet` passed 29 unit tests and 18 doctests, with eleven graphical fixtures and four doctests default-ignored. The new pure regression injects an initial refresh failure, commits scale/geometry on retry, verifies the callback snapshot and live writer, and verifies exactly one transition despite subsequent successful refreshes.
- `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-x11 -p winit-wayland --all-targets --locked --offline -- -D warnings` passed.
- `CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets --all-features` and `CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets --all-features -- -D warnings` passed, including the expanded root graphical fixture.
- `CARGO_BUILD_JOBS=2 CARGO_PROFILE_TEST_DEBUG=0 LD_LIBRARY_PATH=/tmp/opencode/x11-panel-tools/usr/lib/x86_64-linux-gnu python3 tests/native/x11_panel_smoke.py --openbox /tmp/opencode/x11-panel-tools/usr/bin/openbox` passed two root graphical fixtures, the ordinary fork popup keyboard-focus fixture, and the new fork terminal-routing fixture. The latter uses controlled native event dispatch on an owned live X connection: it verifies active/repeat/composition clearing, forces a stale terminal active route, processes XKB state/map and a core key addressed to a live nonterminal window, verifies global Shift state still updates without terminal callbacks, then drops an ancestor while retaining its descendant and verifies late DestroyNotify emits no events. The OpenGL probe additionally retains a retired sibling descendant while physically dropping its popup ancestor and draining server DestroyNotify, with no callbacks after Destroyed and successful retained-renderer cleanup. Existing managed geometry/reservation, visibility, focus and retirement smoke assertions still passed.
- Targeted Rust formatting ran. `cargo fmt --all -- --check` and `cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check` passed. Final source/docs review, `git diff --check`, Git status and unchanged manifest/lockfile checks completed.

An initial test compile lacked the required can_create_surfaces callback. The first new native-routing fixture run crashed because its fabricated XEvent omitted the live Display pointer required by XFilterEvent; initializing that test input fixed it. A subsequent count assertion included an unrelated ordinary-window IME event; restricting the terminal event count to the retired tree fixed the fixture. The complete driver then passed. These were fixture construction failures, not claimed production failures or successful validations.

Live DPI/native-read failure-and-recovery was not exercised: the scale regression is a pure controlled production-notifier seam, not equivalent live coverage. Other physical hotplug/multi-monitor/platform/feature-matrix gaps from the previous entries remain. The full root suite and Wayland smoke were not rerun for this follow-up: root production and Wayland code did not change; root all-target/all-feature check/Clippy and the changed graphical fixture were validated. No new dependencies, API/migration/version changes, lockfile changes or generated files. DEVLOG and native-panel documentation changed. Root remains 0.41.0; `.gitignore` retains its preexisting 7 additions/1 deletion untouched. Nothing was staged or committed.

## [2026-10-12T16:19:35-03:00] - Correct Nine Native Panel Review Defects - Unreleased

**Context:** Address the independent nine-defect review of the backend-neutral panel implementation, including focus-restoration coverage.

**Challenge:** RandR clone identity can change without a CRTC event, topology reads can race, delayed WM map notifications can reverse newer requests, and terminal callbacks can reenter creation or drop renderer/window owners while queued input still exists.

**Decision:** Select OUTPUT_CHANGE independently. Read output/CRTC replies and root geometry from fresh topology, checking status, resource/config timestamps, output/CRTC lists and root dimensions; retry changing snapshots up to three times. Distinguish confirmed terminal loss from transient native errors, log context and retry on later event-loop iterations with a 50ms maximum wait. Reconcile live mapping against the latest requested visibility. Collect and revoke the complete panel tree before withdrawal or callbacks, reject children under retained revoked parents, clear retry state, and retain terminal XID tombstones across owner drop/DestroyNotify. Gate core keys before IME filtering, XI2 target events, queued IME requests/events, redraw and activation callbacks. Use half-open output rectangles for pointer selection while preserving inclusive struts. Reject neutral Wayland activation tokens before allocation; reject compact perpendicular margins on every edge. Require an EWMH supporting window distinct from and immediately under the requested root, with correctly typed/formatted properties. Panel-owned popups restore actual prior external focus under a short server grab using CurrentTime, including immediate close before FocusIn drains; ordinary popup restoration remains unchanged.

**Files Changed:** `vendor/rutter-winit/winit-core/src/window/panel.rs`, X11 `panel.rs`, `monitor.rs`, `util/randr.rs`, `window.rs`, `event_processor.rs`, `event_loop.rs`, Wayland `event_loop/mod.rs`, root X11/Wayland native fixtures, `tests/native/x11_panel_smoke.py`, `docs/NATIVE_PANELS.md`, `vendor/rutter-winit/RUTTER_FORK.md`, and this log.

**Validation:** Resource-intensive commands ran sequentially; native drivers used owned disposable displays, with inherited display/session/DBus addresses cleared. No subagents were used.

- `CARGO_BUILD_JOBS=2 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --offline --all-features -- --test-threads=2` passed 683 library tests (35 ignored), 50 binary tests, 235 integration tests and 308 doctests.
- `CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets --all-features` and `CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets --all-features -- -D warnings` passed.
- `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-x11 -p winit-wayland --locked --offline --quiet` passed 28 unit tests and 18 doctests; ten graphical fixtures and four doctests were default-ignored. Added pure regressions cover half-open adjacent boundaries, named clone reassignment/removal, topology generation/status/root consistency, bounded changing-snapshot retries, and compact perpendicular margins on all edges.
- `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-x11 -p winit-wayland --all-targets --locked --offline -- -D warnings` passed.
- `CARGO_PROFILE_TEST_DEBUG=0 LD_LIBRARY_PATH=/tmp/opencode/x11-panel-tools/usr/lib/x86_64-linux-gnu python3 tests/native/x11_panel_smoke.py --openbox /tmp/opencode/x11-panel-tools/usr/bin/openbox` passed. It ran two root graphical fixtures and the existing fork ordinary-popup keyboard-focus fixture. Coverage includes managed DOCK geometry/struts/workarea, both rapid visibility-toggle orders, malformed root/self/nested/type EWMH properties, separate-process external focus and immediate-close restoration, first-callback whole-tree revocation and reentrant child rejection, active nested popup renderer drop, queued terminal core-key suppression, impossible-geometry retirement and retained OpenGL cleanup.
- `CARGO_PROFILE_TEST_DEBUG=0 LD_LIBRARY_PATH=/tmp/opencode/rutter-layer-compositor/usr/lib/x86_64-linux-gnu python3 tests/native/layer_shell_smoke.py --labwc /tmp/opencode/rutter-layer-compositor/usr/bin/labwc --wlr-randr /tmp/opencode/rutter-layer-compositor/usr/bin/wlr-randr --remove-output --retained-renderer` passed neutral token rejection followed by real neutral panel rendering, targeted output-removal retained-renderer cleanup, and Weston unsupported-role rejection with ordinary creation preserved.
- Root/fork `cargo fmt --all -- --check` (fork with `--manifest-path vendor/rutter-winit/Cargo.toml`) passed. Final source/docs review, `git diff --check`, unchanged manifest/lockfile verification and Git status inspection completed.

Intermediate X11 smoke failures exposed delayed WM unmaps and a retained retry flag that could remap a retired panel; live-state reconciliation and clearing/guarding terminal retries fixed them. A fixture initially rejected ordinary popup Focused(false) after ordinary Destroyed; its assertion was narrowed to the terminal panel tree to preserve ordinary semantics. One transient xwininfo failure prompted captured command diagnostics and explicit retirement-stage synchronization. Final repeated smoke runs passed; temporary debugging output was removed.

Remaining gaps: physical named-clone unplug, live multi-monitor CRTC reassignment/WM behavior, live topology/native-read fault injection, real fractional-DPI changes in this correction run, forced live layer-shell version combinations, and non-Linux cross-compilation were not exercised. Pure topology simulations and existing negotiated-version tests are not equivalent live coverage. Fork all-features offline coverage remains blocked by the previously recorded uncached `cosmic-text 0.19.0`; the successful selected-package checks do not claim that feature matrix. External server-side X drawable destruction remains distinct from safe retained-drawable retirement. No new Cargo dependencies, version/lockfile changes, generated files or migration were introduced; X11 panels require runtime libXrandr. Root remains 0.41.0. DEVLOG and contract documentation changed. The preexisting `.gitignore` edit remains untouched; nothing was staged or committed.

## [2026-10-12T15:42:18-03:00] - Add Backend-Neutral Native Panels and Managed X11 Docks - Unreleased

**Context:** Add opt-in `PanelConfig` / `WindowConfig::with_panel()` with real X11/EWMH and Wayland panel roles while preserving the explicit Wayland-only layer API and native menus.

**Challenge:** EWMH reservations use inclusive physical root-border coordinates; XRandR output identity, native focus restoration and retained EGL drawable lifetime must remain truthful through geometry changes and terminal retirement.

**Decision:** Validate immutable neutral edge/length/thickness/margin/output/reservation/focus requests before allocation. Translate Wayland requests into the existing Top layer-shell construction, preserving negotiated versions and retirement. Create managed X11 DOCK windows only after fresh EWMH root/self support checks and exact XRandR selection; pin the selected output, use full output geometry rather than workarea, and reject reservations on internal root borders. Initialize and retain libXrandr through existing x11-dl so Xlib decodes events selected through x11rb. Release struts on hide/drop, recompute on RandR/scale changes, and retire impossible layouts child-first with sticky handle revocation while retaining drawable storage for renderer cleanup. Panel-owned active menus restore the previous viewable application only when still owning focus. Box the private optional panel settings in root WindowConfig to avoid enlarging SurfaceCommand past Clippy's threshold without changing its public variants or suppressing the lint.

**Files Changed:** `src/multi_window/{mod.rs,window_config.rs}`, `src/lib.rs`, multi-runner metrics and native integration fixtures, `vendor/rutter-winit/winit-core/src/window{.rs,/panel.rs}`, X11 panel/window/event modules, Wayland role translation/reporting, unsupported-backend creation boundaries, `tests/native/{x11_panel_smoke.py,layer_shell_smoke.py}`, `docs/NATIVE_PANELS.md`, `docs/WAYLAND_LAYER_SHELL.md`, `README.md`, `vendor/rutter-winit/RUTTER_FORK.md`, and this log.

**Validation:** Resource-intensive commands ran sequentially with `CARGO_BUILD_JOBS=2`; no subagents. Native drivers used owned disposable displays and cleared inherited display/session/DBus addresses.

- Final `CARGO_BUILD_JOBS=2 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --offline --all-features -- --test-threads=2` passed 683 library tests (35 ignored), 50 binary tests, 235 integration tests and 308 doctests, including the final private-storage adjustment.
- Final `CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets --all-features` and `CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets --all-features -- -D warnings` passed. The first final Clippy attempt flagged large_enum_variant; boxing the private optional configuration resolved it without changing SurfaceCommand's API.
- Final `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-x11 -p winit-wayland --locked --offline` passed 23 unit tests and 18 doctests; ten graphical fixtures and four doctests were default-ignored. Geometry tests cover all four edges, stretch/compact layouts, scales 1/1.5/2, margins, inclusive ranges, root bounds and internal-edge restrictions.
- Final `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-x11 -p winit-wayland -p winit --locked --offline --all-targets -- -D warnings` passed.
- `LD_LIBRARY_PATH=/tmp/opencode/x11-panel-tools/usr/lib/x86_64-linux-gnu python3 tests/native/x11_panel_smoke.py --openbox /tmp/opencode/x11-panel-tools/usr/bin/openbox` passed missing-WM rejection, managed DOCK geometry/hints, partial struts, workarea/maximized Settings, active/inactive popup focus, hide/show/close reservation release, root-resize recompute and child-first impossible-geometry retirement with retained OpenGL frame/present/resize rejection and cleanup.
- `LD_LIBRARY_PATH=/tmp/opencode/rutter-layer-compositor/usr/lib/x86_64-linux-gnu python3 tests/native/layer_shell_smoke.py --labwc /tmp/opencode/rutter-layer-compositor/usr/bin/labwc --wlr-randr /tmp/opencode/rutter-layer-compositor/usr/bin/wlr-randr --remove-output --retained-renderer` passed the neutral Wayland OpenGL role probe, existing retained-renderer targeted-output retirement and Weston missing-layer-shell rejection.
- Affected Rust formatting ran; final `cargo fmt --all -- --check`, `cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check`, and `git diff --check` passed. Source/docs diffs and Git status were reviewed; `git diff --exit-code -- Cargo.toml Cargo.lock vendor/rutter-winit/Cargo.toml vendor/rutter-winit/Cargo.lock` confirmed unchanged versions/lockfiles.

Earlier full-suite linking failed with no disk space. `cargo clean --locked --offline -p rutter` exceeded its tool timeout but freed rebuildable artifacts; process inspection confirmed no compiler remained. Reduced test debug info allowed the successful full-suite runs; an initial reduced-debug attempt also exceeded its build timeout before the successful retry. An external XDestroyWindow renderer probe failed with BadDrawable; the successful retained-renderer probe instead exercises safe impossible-geometry retirement, and the contract explicitly distinguishes server-side drawable destruction.

Remaining gaps: physical X11 output unplug, live multi-monitor WM behavior, real fractional DPI changes, non-Linux cross-compilation and fork all-features combinations are not claimed validated. Fork all-features offline coverage remains blocked by the previously recorded uncached cosmic-text 0.19.0 dependency. Unsupported platforms are source-updated only. Panels require runtime libXrandr on X11; no new Cargo dependencies, generated files, migration or version/lockfile changes were introduced. Root remains 0.41.0. The preexisting `.gitignore` edit was left untouched; nothing was staged or committed.

## [2026-10-12T12:07:37-03:00] - Release Native Menu Overflow and Wayland Panels - 0.41.0

**Context:** Prepare one combined commit of both issue corrections, terminal layer retirement and fork lint fixes, with the requested Rutter version increment.

**Challenge:** Synchronize release metadata without changing dependency resolution, preserve the existing worktree, and distinguish exercised Linux behavior from unvalidated compositor/platform scenarios.

**Decision:** Release 0.41.0 as a minor increment for opt-in layer-shell settings, configured surface metrics and defaulted failure hooks. Preserve historical validation entries. Fix core-test Clippy diagnostics without suppressions: keep nested-reference downcast coverage via an explicitly typed binding and use a byte literal for IME test input. Leave the preexisting `.gitignore` edit outside staging under the standing repository restriction. Layer retirement remains terminal, while ordinary toplevel close requests remain vetoable; direct fork consumers must release renderer resources before retained windows. Local path dependencies still prevent registry publication as-is.

**Files Changed:** `Cargo.toml`, Cargo-regenerated `Cargo.lock`, native menu geometry/render/input/accessibility and GPU modules, multi-window configuration/metrics/lifecycle, their regression tests, the local Winit fork, `README.md`, `docs/WAYLAND_LAYER_SHELL.md`, `vendor/rutter-winit/RUTTER_FORK.md`, and this log. Independent fork versions are unchanged.

**Validation:** All resource-intensive commands ran sequentially with `CARGO_BUILD_JOBS=2`.

- `cargo check --offline --all-targets` passed and regenerated only the root package version in `Cargo.lock`; the manifest/lockfile diff was inspected.
- `cargo test --locked --offline --quiet -- --test-threads=2` passed again at 0.41.0: 682 library tests (32 ignored), 50 binary tests, 235 integration tests and 308 doctests.
- `cargo clippy --locked --offline --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check` and `cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check` passed at 0.41.0.
- The preceding lint correction run passed `CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-common -p winit-wayland -p winit-x11 -p winit -p dpi --all-targets --locked --offline -- -D warnings`. Fork tests with `-p winit-core -p winit-wayland -p winit-x11` passed 20 unit tests and 18 doctests; ten graphical fixtures and four doctests were ignored by default. All three forks' formatting checks passed.
- AccessKit fork all-target Clippy passed separately for defaults and `--no-default-features --features accesskit_unix,tokio,winit/x11,winit/wayland`; its all-features attempt combines incompatible async-io/tokio runtimes and fails by design. Glutin fork all-target/all-feature Clippy passed. Both used their respective `--manifest-path`, `CARGO_TARGET_DIR=target`, locked/offline resolution and `-- -D warnings`.
- Winit fork all-features Clippy could not run offline because cosmic-text 0.19.0 was not cached; an additional serde/mint attempt was blocked by uncached mint 0.5.9. Default-feature lint success does not imply these feature combinations or non-Linux targets passed.

Earlier isolated graphical results and their limitations remain recorded below; no graphical checks were rerun solely for this version increment. FRAYM acceptance remains blocked by the documented compositor stall. No new dependencies or generated source files were added; dependency resolution and independent fork lockfiles/versions are unchanged.

## [2026-10-12T10:13:22-03:00] - Make Layer-Shell Native Retirement Non-Vetoable - Unreleased

**Context:** Complete the remaining lifetime defect in the existing two-issue worktree: a direct fork consumer could ignore layer/output closure and retain its strong WindowState owner.

**Challenge:** Removing native maps alone cannot retire a strongly retained role; destroying SCTK's owner immediately also destroys wl_surface while EGL may still cache its pointer. Popup descendants must retire first, and ordinary toplevel close requests must remain vetoable.

**Alternatives:** Map removal/CloseRequested alone leaves live roles behind. Dropping the entire SCTK role owner at terminal dispatch invalidates renderer cleanup pointers. Keeping a cleanup-only SCTK owner while explicitly destroying its protocol roles releases reservations promptly without that pointer-lifetime hazard.

**Decision:** Complete the prior session's terminal flag, child-first retirement and frontend guards. Compositor layer `closed`, invalid configure and targeted-output removal revoke the layer/popup tree immediately; event-loop retirement destroys popup/xdg roles before the layer role, removes routes and pending events, and delivers child-first CloseRequested/Destroyed regardless of retained frontends/metadata. Keep a retired-state tombstone owning the backing wl_surface until renderer/window/state release; SCTK's repeated role destructors are rejected locally for dead ObjectIds. Retain popup state strongly for the same raw-handle cleanup guarantee and explicitly retire its role on existing popup removal paths. Ordinary xdg-toplevel requests remain vetoable. Reject Rutter CPU/OpenGL/Vulkan begin/present/resize when the native handle is unavailable. Document that direct fork consumers must stop rendering on terminal closure and destroy renderer resources before their retained windows; cached proxies and the low-level surface accessor are cleanup-only after retirement.

**Files Changed:** `vendor/rutter-winit/winit-wayland/src/{layer_shell.rs,state.rs,popup.rs,event_loop/mod.rs,event_loop/sink.rs,window/mod.rs,window/state.rs,window/handles.rs,window/lifetime.rs,window/retirement.rs,window/retirement_tests.rs}`, `vendor/rutter-winit/winit-core/src/window.rs`, `src/engine/gpu{.rs,/cpu.rs,/gl.rs,/vk.rs}`, `src/engine/multi_runner/layer_shell_integration.rs`, `tests/native/layer_shell_smoke.py`, `docs/WAYLAND_LAYER_SHELL.md`, `vendor/rutter-winit/RUTTER_FORK.md`, and this log. Earlier native-menu/layer-shell changes and the preexisting `.gitignore` edit were preserved.

**Validation:** Builds, tests and analysis ran sequentially with `CARGO_BUILD_JOBS=2`, locked/offline Cargo resolution, and no subagents. Graphical commands used private Weston/Xwayland/labwc sessions and disposable sockets, never `:0` or `wayland-0`; the driver terminated owned processes after each run.

- Affected root/fork `rustfmt --edition 2024` ran (fork uses `--config-path vendor/rutter-winit/rustfmt.toml`). `cargo fmt --all -- --check`, `cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check`, and `git diff --check` passed. Source/docs diffs and final Git status were reviewed. `git diff --exit-code -- Cargo.toml Cargo.lock vendor/rutter-winit/Cargo.toml vendor/rutter-winit/Cargo.lock` passed.
- `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-wayland --locked --offline` passed three unit tests; two new compositor fixtures were default-ignored. The broader fork command with `-p winit-core -p winit-wayland -p winit-x11` passed 20 unit tests and 18 doctests, with ten graphical fixtures and four core doctests default-ignored.
- `CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib --no-run` passed after correcting the new fixture's event-loop ownership and trait import. Final `CARGO_BUILD_JOBS=2 cargo test --locked --offline --quiet -- --test-threads=2` passed 682 library tests (32 ignored), 50 binary tests, 235 integration tests and 308 doctests. The first full-suite attempt exceeded the tool's 240-second timeout during build; process inspection confirmed no build/test remained before rerunning with a 600-second limit.
- `CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets --all-features` and `CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets --all-features -- -D warnings` passed. `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-wayland --all-targets --locked --offline -- -D warnings` passed after correcting fixture-only collapsible-match and disallowed println diagnostics. The previously documented unrelated core all-target test lint failures were not rerun or claimed resolved.
- Final retained closure command: `LD_LIBRARY_PATH=/tmp/opencode/rutter-layer-compositor/usr/lib/x86_64-linux-gnu python3 tests/native/layer_shell_smoke.py --labwc /tmp/opencode/rutter-layer-compositor/usr/bin/labwc --retained-layer --keep` passed. It creates a mapped layer, two nested popups and an independent ordinary toplevel, retains all Window/state/surface owners, deliberately ignores CloseRequested, and invokes the production SCTK closed callback. It verifies immediate new-handle/mutation revocation, child-first Destroyed, dead native roles with cleanup backing surfaces retained, final backing destruction, and the still-live ordinary toplevel. Logs: `/tmp/opencode/rutter-layer-oqctl9gy`. An initial fixture attempt deadlocked because it held the WindowState mutex while invoking the callback; releasing the lock before callback fixed the fixture.
- The same command with `--wlr-randr /tmp/opencode/rutter-layer-compositor/usr/bin/wlr-randr --remove-output --retained-layer --keep` passed actual targeted wl_output removal while a second output remained enabled. The retained layer/nested children retire without veto or migration. Logs: `/tmp/opencode/rutter-layer-k769csq4`.
- The same two-output command with `--retained-renderer` instead of `--retained-layer` passed actual retained OpenGL layer/popup frame presentation before removal, rejection of subsequent begin-frame/present/resize, and EGL/Skia destruction after retirement. Logs: `/tmp/opencode/rutter-layer-sjegyi17`. The initial new root fixture needed the existing with_any_thread Wayland builder pattern; that test-only failure was corrected before successful runs.
- The existing runner two-output smoke command (same labwc/wlr-randr paths, `--remove-output --keep`, no retained flag) passed four rendered panel pixel probes, targeted route retirement and preserved Settings. Logs: `/tmp/opencode/rutter-layer-15y72f9q`. Each layer-driver run also passed the unsupported Weston/ordinary-window fixture.
- `env -u DISPLAY -u WAYLAND_DISPLAY -u WAYLAND_SOCKET -u DBUS_SESSION_BUS_ADDRESS -u DBUS_STARTER_ADDRESS python3 /tmp/opencode/rutter_overflow_displays.py wayland 1` passed all ten first-issue Wayland overflow/nested/dropdown/CPU-fallback/multi-window routing fixtures. Weston emitted its existing teardown diagnostics; all fixtures completed successfully.

Remaining gaps: the closed test explicitly invokes the production callback rather than eliciting an unsolicited compositor closed event; live terminal CPU/Vulkan renderer teardown, physical hotplug/recreation, FRAYM's previously recorded compositor blocker and other earlier live acceptance gaps remain unvalidated. OpenGL validation does not claim those cases. The native layer lifetime contract is intentionally terminal; ordinary toplevel request semantics remain compatible. DEVLOG and contract documentation changed; no dependencies, versions, lockfiles or generated files changed. Nothing was staged or committed.

## [2026-10-12T00:54:12-03:00] - Integrate Native Wayland Layer-Shell Panels - Unreleased

**Context:** Implement the second issue, `issues/RUTTER_WAYLAND_LAYER_SHELL_PANEL_SURFACES.md`, while preserving the completed native-menu overflow tree and preexisting `.gitignore` edits.

**Challenge:** A layer panel must acquire a real role before mapping, share Winit/Rutter's current queue and SurfaceId owner, negotiate support honestly, retain configured logical geometry through fractional scaling, and associate xdg popups without an xdg parent. Publishing new metrics must also preserve queued source-local native-menu application updates.

**Decision:** Reuse the existing SCTK 0.21 layer-shell API and its configure acknowledgement/destruction ownership, without new Cargo dependencies. Add immutable, validated connection-independent role settings and a truthful `WindowType::LayerShell` tag in the fork; create the layer role on an unassigned surface before its initial empty commit. Resolve exact output names on the same connection, reject absent/ambiguous targets and unsupported requests, and keep compositor output selection explicit. Target removal closes logical panels without migration or automatic recreation. Keep ordinary Settings windows independent. Add a defaulted configured-metrics hook, reset its cache for new native lifetimes, and synchronize native-menu messages/commands before it publishes shared state. Create null-parent xdg popups and associate them with the layer before grab/commit; remove provisional state on initial configure/creation failure. Document immutable size hints, compact lengths, exclusive-distance/margin policy, full rectangular input regions and transparent presentation requirements.

**Files Changed:** `src/multi_window/{mod.rs,window_config.rs}`, `src/lib.rs`, `src/engine/{multi_runner.rs,runner.rs}`, `src/engine/multi_runner/{startup.rs,surface_metrics.rs,layer_shell_integration.rs}`, `vendor/rutter-winit/winit-core/src/{window.rs,window/layer_shell.rs,event_loop/mod.rs}`, `vendor/rutter-winit/winit-wayland/src/{layer_shell.rs,lib.rs,state.rs,popup.rs,event_loop/mod.rs,window/mod.rs,window/state.rs}`, non-Wayland fork event-loop creation boundaries, `tests/native/layer_shell_smoke.py`, `README.md`, `docs/WAYLAND_LAYER_SHELL.md`, `vendor/rutter-winit/RUTTER_FORK.md`, and this DEVLOG. Existing first-issue geometry/render/accessibility implementation and `.gitignore` edits were retained.

**Validation:** Compilation, tests and analysis were sequential with `CARGO_BUILD_JOBS=2`; owned graphical sessions were stopped after each driver invocation.

- Affected root/fork `rustfmt --edition 2024` ran. `cargo fmt --all -- --check`, `cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check` and `git diff --check` passed during validation; final checks/status/diffs were inspected. Lock/version manifests were checked with `git diff --exit-code -- Cargo.toml Cargo.lock vendor/rutter-winit/Cargo.toml vendor/rutter-winit/Cargo.lock`.
- `CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib layer_shell -- --test-threads=2` passed two root configuration/default-role tests, with two graphical fixtures ignored by default. Fork tests add four geometry/identity/coordinate/keyboard-version cases, including every anchor combination and non-reserving policies.
- `CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib native_ -- --test-threads=2` passed 32 tests, with 30 graphical fixtures ignored by default. The existing two-surface CPU failure-routing fixture initially exposed metrics publication overwriting a queued native-menu update. The runner ordering was corrected, without weakening that first-issue fixture.
- Final `CARGO_BUILD_JOBS=2 cargo test --locked --offline --quiet -- --test-threads=2` passed 682 library tests, 50 binary tests, 235 integration tests and 308 doctests. The 31 default-ignored library tests include graphical fixtures and the existing CPU diagnostic. An earlier attempt hit the tool's 120-second timeout during build; no build process remained before rerunning with a longer timeout. Full tests passed again after the ordering and role-tag changes.
- Final `CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets --all-features` and `CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets --all-features -- -D warnings` passed.
- Final `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-wayland -p winit-x11 --locked --offline` passed 20 unit tests and 18 doctests; eight fork graphical fixtures and four core doctests remained ignored by default. The new root live fixtures exercise the affected native boundary.
- Fork all-target Clippy with the same manifest/jobs/target/locked/offline settings and `-p winit-core -p winit-wayland -p winit-x11 --all-targets -- -D warnings` reproduced the existing unrelated `needless_borrow` core casting test and `char_lit_as_u8` core window test failures. New exhaustive-enum warnings were corrected with forward-compatible non-exhaustive settings enums. Final scoped Clippy with `-p winit-core --lib` and separately `-p winit-wayland -p winit-x11 --all-targets` passed. This does not claim all-target core-test lint success.
- `python3 /tmp/opencode/rutter_overflow_displays.py wayland 1` passed all ten existing first-issue Wayland fixtures after the publication-order correction: both thin orientations, context/dropdown/nested menus, usable/unusable CPU fallback and two-surface selection/failure routing. The initial run stopped at the failure-routing regression; the complete rerun passed. Weston teardown diagnostics did not fail the completed checks.
- `python3 tests/native/layer_shell_smoke.py --fraym /home/p_daniel/Labs/Isolated_Environment/rustLang@Projects/fraym_compositor/target/debug/fraym --keep` successfully exercised the unsupported Weston fixture, then established the FRAYM blocker: its advertised v4 layer global receives get_layer_surface, but no configure or subsequent display-sync reply arrives. Inspection of FRAYM's `new_layer_surface` shows a layer-map guard retained across a match success arm that locks the map again. The suspected compositor deadlock was documented; that checkout was not modified and FRAYM acceptance is not claimed.
- Labwc 0.7.1, wlroots 0.17.1 and wlr-randr were downloaded/extracted only under `/tmp/opencode/rutter-layer-compositor` for the alternate live checks, along with missing shared libraries; they are test tools, not repository/Cargo dependencies. Final command `LD_LIBRARY_PATH=/tmp/opencode/rutter-layer-compositor/usr/lib/x86_64-linux-gnu python3 tests/native/layer_shell_smoke.py --labwc /tmp/opencode/rutter-layer-compositor/usr/bin/labwc --wlr-randr /tmp/opencode/rutter-layer-compositor/usr/bin/wlr-randr --keep` passed. Pixel probes establish four edge anchors and compact lengths; OpenGL renders all panels and Settings. Settings is configured at 1100x760 with a 40-pixel reservation, then 1100x800 with restored pixels after closure. Real XTEST on the owned X11 host becomes compositor-delivered Wayland presses selecting a layer-owned popup below the unchanged 40-pixel parent; Settings focus survives. Client trace checks initial empty commit, configure acknowledgement before buffers, null xdg parent, get_popup association and role destruction. Output resize and scale 2/1.5 changes reach source-local configured-metrics hooks. Missing output and conflicting popup-role requests also fail explicitly. Final logs: `/tmp/opencode/rutter-layer-hb0j_o4h`.
- The same final labwc command with `--remove-output --keep` passed a separate two-output run. Disabling X11-1 actually removes its wl_output global; all four targeted panels close, their routes are retired, Settings remains, and no panels are created on X11-2. This is a live protocol-removal test, not physical hotplug or output recreation. A preliminary XDestroyWindow probe did not remove the output, and attempts to re-enable the only disabled X11 output were rejected by this compositor. The final probe keeps a second output enabled. Final logs: `/tmp/opencode/rutter-layer-o4bql0we`.

**Harness isolation correction:** Early diagnostic setup paths used inherited display/Wayland addresses for read-only queries. An early labwc launch also retained the inherited D-Bus address, which labwc uses to update DISPLAY/WAYLAND_DISPLAY/XDG_CURRENT_DESKTOP activation variables. Both `dbus-update-activation-environment DISPLAY WAYLAND_DISPLAY XDG_CURRENT_DESKTOP` and `systemctl --user import-environment DISPLAY WAYLAND_DISPLAY XDG_CURRENT_DESKTOP` were executed successfully to restore these keys from the original process environment. No test input or output-control requests were sent to inherited desktop displays. The final driver removes inherited display/session addresses, explicitly addresses owned X servers, uses a private runtime/config directory and empty session autostart/environment files, and avoids automatic wayland-0 naming. The successful final graphical runs used that corrected isolation.

Remaining acceptance gaps: FRAYM's stalled layer callback, physical hotplug/same-name output recreation, live protocol versions 1–3, fractional-scale menu input, layer-parent nested dropdown chains, native Vulkan, non-Linux runtime and live assistive technology. Deterministic version/geometry coverage and ordinary-parent first-issue regressions do not imply those live cases passed. Existing application defaults are backward compatible; no SurfaceEvent variant was added. No dependencies, versions, lockfiles or generated files changed. No files were staged or committed.

## [2026-10-11T22:23:03-03:00] - Complete Interrupted Native Overflow Review Validation - Unreleased

**Context:** Resume the first issue's interrupted review against the existing working tree, including its newer allocation, readiness, configure/input and submenu-chain corrections.

**Challenge:** The previous successful full-suite entry predates the review edits; their safeguards and graphical regressions needed fresh validation without touching the regular desktop or starting the separate layer-shell issue.

**Decision:** Retain the existing review implementation after inspecting its code, tests and diffs and validating its observable boundaries. No additional runtime defect was found in this continuation. Confirm the physical allocation ceiling before requested/configured drawable allocation and resize (including Vulkan's selected extent), successful-frame readiness, configure reconciliation/presentation before pointer input, parent-logical versus popup-buffer coordinates, and constrained chain reflow from actual revealed parent rows. Keep the focused native-menu geometry/presentation/test split. Complete README test-filter documentation and the fork's snapshot/readiness/allocation contract.

**Files Changed:** This continuation updates `README.md`, `vendor/rutter-winit/RUTTER_FORK.md` and this DEVLOG. Reviewed/validated preexisting implementation includes `src/engine/gpu/{popup_limits.rs,cpu.rs,gl.rs,vk.rs}`, `src/engine/runner/native_menu.rs`, `src/engine/runner/native_menu/{geometry.rs,presentation.rs,tests.rs}`, `src/engine/runner/native_menu_review_integration.rs`, `src/widgets/dropdown_menu/geometry.rs`, native integration/raster/geometry/accessibility/hover tests, and the Winit core/Wayland popup boundary.

**Validation:** All compilation, test and analysis operations ran sequentially with `CARGO_BUILD_JOBS=2`. No validation operation from the interrupted run remained active before starting. All graphical runs used owned disposable Weston/private Xwayland sessions; no input was sent to `:0` or `wayland-0`.

- `cargo fmt --all -- --check`, `cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check` and `git diff --check` passed. Affected review-source `rustfmt --edition 2024` ran and both formatting checks passed again. Final diffs and Git status were inspected.
- `CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib native_ -- --test-threads=2` passed 31 tests; 28 graphical fixtures were ignored by default. This includes allocation extremes, distinct parent/popup scale conversion, configure serial acknowledgement, constrained unequal-width LTR/RTL chains, actual parent-scroll child alignment, unusable-chain rejection, fractional raster/input conversion, transparency, hover and accessibility.
- `CARGO_BUILD_JOBS=2 cargo test --locked --offline --quiet -- --test-threads=2` passed 680 library tests, 50 binary tests, 235 integration tests and 307 doctests. The 29 default-ignored library tests comprise 28 graphical fixtures and the unrelated CPU diagnostic.
- `CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets --all-features` and `CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets --all-features -- -D warnings` passed.
- `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-wayland -p winit-x11 --locked --offline` passed 16 unit tests and 18 doctests, including the two new core/Wayland geometry-snapshot tests. Eight fork graphical fixtures and four core doctests were ignored by default; the root graphical checks exercise the affected popup boundary.
- Fork all-target Clippy with `-p winit-core -p winit-wayland -p winit-x11 --all-targets --locked --offline -- -D warnings` reproduced the existing unrelated core-test failures: `needless_borrow` at `winit-core/src/casting.rs:56` and `char_lit_as_u8` at `winit-core/src/window.rs:2459`. Scoped fork Clippy with `-p winit-core --lib` and separately `-p winit-wayland -p winit-x11 --all-targets` passed, using the same jobs/target/manifest/locked/offline settings. This does not claim all-target core-test lint success.
- `python3 /tmp/opencode/rutter_overflow_displays.py review 1` passed all four review fixtures: first presentation failure leaves readiness false and reports/dismisses the unusable fallback; oversized configure is rejected before backend resize; a pointer delivered before queued resize/redraw reconciles and presents the new scrolled layout before selection; oversized initial GL popup configuration is rejected before graphics-surface allocation.
- The same temporary driver ran `wayland 1` (10 passed), `x11 1` (11 passed), `wayland 2` (six passed) and `constrained 1` (one passed) sequentially. These cover ContextMenu/DropdownMenu outside both thin orientations, nested menus, XTEST selection/dismissal/focus, usable/unusable CPU fallback, opening reconciliation, multi-window selection/failure routing, and explicitly requested smaller configure with wheel scrolling and End-key reveal. Scale-2 coverage is context/fallback/multi-window/nested context, not dropdown fixtures.
- `python3 /tmp/opencode/rutter_overflow_displays.py live 1` passed the separate compositor-delivered Wayland input fixture. The owned nested Weston's scene graph showed an ARGB `xdg_popup` at (303,662)–(471,706) beyond the unchanged parent at (283,642)–(1083,672); external XTEST on its owned X11 host became actual Wayland parent/popup presses and selected Settings. In total the driver ran 33 successful separate-process graphical invocations covering 27 distinct fixtures. Owned processes were terminated and temporary runtime directories cleaned up. Weston emitted its layer/view teardown diagnostics; no Rutter fixture failed.

Core first-issue behavior is validated for the exercised scenarios; exhaustive live acceptance is not claimed. Spontaneous output-boundary flip/slide/resize, fractional-output pointer input, compositor pixel comparisons of submenu alpha, compositor-delivered nested/multi-window Wayland input, native Vulkan presentation, live assistive technology and non-Linux runtime remain unvalidated. The available checks establish deterministic adjusted/negative/fractional geometry and raster behavior, explicit smaller configure delivery, and real Wayland single-row overflow input. No additional environment for those remaining live scenarios was established in this continuation. The layer-shell issue was not started. No dependencies, versions, lockfiles or generated files changed; nothing was staged or committed, and the preexisting `.gitignore` changes were preserved.

## [2026-10-05T01:20:43-03:00] - Allow Native Menus Outside Thin Parent Viewports - Unreleased

**Context:** Implement only `issues/RUTTER_NATIVE_MENU_OUTSIDE_PARENT_VIEWPORT.md`, including independent native sizing, configured geometry, input and observable fallback.

**Challenge:** Removing parent containment alone leaves menus measured, painted and selected against a thin parent viewport. Wayland also requires a parent-inside anchor, can configure a different position/size/scale, and does not expose an authoritative shell work area.

**Decision:** Measure native ContextMenu/DropdownMenu chains within a finite local 1024×768 logical policy area, with a 320-pixel per-panel height cap and existing scrolling/reveal metrics; preserve the overlay geometry functions. Request a valid 1×1 logical parent anchor plus the union's offset and permit every slide/flip/resize constraint. Resolve the requested panels into the actual popup rectangle and retain that list in the parent engine for painting, hit-testing, hover, wheel targets, lower-layer coverage and dropdown accessibility. Paint/convert input at the actual popup scale and reproject the shared pointer for parent frames. Accept overflow, negative origins, displacement and usable resized surfaces. Add a defaulted Winit `request_popup_layout` transaction; Wayland applies size and position together and emits a configure acknowledgement even for unchanged-size repositioning. Preserve parent-owned state/messages, alpha union gaps, opening callbacks, focus and dismissal. Graphics/configuration failures retain only usable overlays; otherwise dismiss the menu and notify defaulted single/multi-window `native_menu_unavailable` hooks via an ordinary parent-owned message. Capability failure lasts until parent release/resume; repeated unusable opening attempts remain observable. No exhaustive SurfaceEvent variant was added.

**Files Changed:** `src/engine/runner/{native_menu.rs,dropdown_pointer.rs,native_integration.rs,native_overflow_integration.rs}`, `src/engine/{mod.rs,runner.rs,multi_runner.rs}`, `src/engine/multi_runner/{app_adapter.rs,native_menu_integration.rs}`, `src/widgets/dropdown_menu/{geometry.rs,mod.rs}`, `src/render/{mod.rs,dropdown_menu_overlay.rs,overlay_hover.rs}`, `src/accessibility/{mod.rs,dropdown_menu.rs}`, `src/{app.rs,lib.rs,multi_window/mod.rs}`, `tests/unit/{native_menu_render_unit_tests.rs,dropdown_menu_accessibility_unit_tests.rs,overlay_hover_unit_tests.rs}`, `vendor/rutter-winit/winit-core/src/window.rs`, `vendor/rutter-winit/winit-wayland/src/{popup.rs,window/state.rs}`, `README.md`, `vendor/rutter-winit/RUTTER_FORK.md`, and this DEVLOG.

**Validation:** All builds, tests and analysis ran sequentially with `CARGO_BUILD_JOBS=2`; disposable display processes were owned and cleaned up. No input was sent to the user's `:0` or `wayland-0`.

- Affected-file root `rustfmt --edition 2024` and fork `rustfmt --edition 2024 --config-path vendor/rutter-winit/rustfmt.toml` ran. `cargo fmt --all -- --check`, `cargo fmt --manifest-path vendor/rutter-winit/Cargo.toml --all -- --check`, and `git diff --check` passed. Final source/docs diffs and Git status were inspected.
- `CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib native_ -- --test-threads=2` passed 25 tests, with 24 graphical fixtures ignored by default. Deterministic/raster coverage includes thin parents, valid outside/negative placement anchors, adjusted/resized scrolling and row selection, differing parent/popup scale conversion, fractional scale 1.5, native union transparency and collapse cleanup, configured accessibility bounds, hover gaps and surface-local failure routing. The new hover assertion was corrected to the existing parent-layer self-masking semantics before this passing run.
- `CARGO_BUILD_JOBS=2 cargo test --locked --offline --quiet -- --test-threads=2` passed 674 library tests, 50 binary tests, all 235 integration tests and 307 doctests; 25 library tests were ignored by default (24 graphical fixtures and the unrelated CPU diagnostic).
- `CARGO_BUILD_JOBS=2 cargo check --locked --offline --all-targets --all-features` and `CARGO_BUILD_JOBS=2 cargo clippy --locked --offline --all-targets --all-features -- -D warnings` passed. Initial localized Clippy warnings were corrected.
- `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-wayland -p winit-x11 --locked --offline` passed 14 unit tests and 18 doctests; eight fork graphical fixtures and four core doctests remained ignored. Root menu graphical fixtures below exercise the changed backend boundary.
- Fork all-target Clippy including core (`CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-wayland -p winit-x11 --all-targets --locked --offline -- -D warnings`) failed on unchanged core tests: intentional repeated references at `winit-core/src/casting.rs:56` trigger `needless_borrow`, and the existing character-to-byte test literal at `winit-core/src/window.rs:2403` triggers `char_lit_as_u8`. Those unrelated tests were preserved. The scoped commands with `-p winit-core --lib` and with `-p winit-wayland -p winit-x11 --all-targets` both passed; these do not claim core-test lint coverage.
- A temporary driver `/tmp/opencode/rutter_overflow_displays.py` ran each graphical filter in a separate sequential `cargo test --locked --offline --lib <filter> -- --ignored --nocapture` process, with software GL, a private runtime directory, owned Weston headless/pixman and rootful `Xwayland :98 -shm`. `python3 /tmp/opencode/rutter_overflow_displays.py x11 1` passed 11 fixtures (both menu kinds outside both thin parents, nested menus, selections, outside dismissal, opening reconciliation and keyboard-focus restoration). `python3 /tmp/opencode/rutter_overflow_displays.py wayland 1` passed 10 fixtures (both menu kinds/thin orientations, nested menus, usable/unusable CPU overlays and two-parent selection/failure routing). `python3 /tmp/opencode/rutter_overflow_displays.py wayland 2` passed six context/fallback/multi-window/nested fixtures at integer output scale 2. Old parent-containment assertions were replaced with actual popup/resolved-panel agreement, and fixtures now wait for configured submenu lists before using them.
- `python3 /tmp/opencode/rutter_overflow_displays.py constrained 1` passed `native_wayland_configured_resize_scrolls_and_selects_outside_parent` on a 1100×180 output: an explicitly requested 120×72 popup configure preserves wheel scrolling, End-key reveal and selection. An earlier short-output probe did **not** cause Weston to shrink the 320-pixel request; spontaneous output-boundary resize is not claimed.
- `python3 /tmp/opencode/rutter_overflow_displays.py live 1` passed `native_wayland_compositor_pointer_selects_outside_thin_parent`. It ran a nested Weston's X11 backend on the owned Xwayland server; an external XTEST driver right-clicked the parent and left-clicked Settings via the nested host. Weston delivered actual Wayland seat events, with one parent and one popup press and no synthetic runner input. Its scene graph showed an ARGB `xdg_popup` at (303,662)–(471,706), extending beyond the unchanged 800×30 parent at (283,642)–(1083,672). All owned processes were terminated afterwards.

Initial automatic Weston Xwayland startup crashed in Mesa EGL and could not initialize Arboard's default X11 clipboard; explicitly launching the owned rootful Xwayland with `-shm` provided the isolated clipboard/input display. The successful setup required no application dependency or real desktop connection. Native creation/presentation and usable/unusable fallback are validated with OpenGL/softbuffer; no live Vulkan claim is made.

Remaining live acceptance gaps are explicit: spontaneous compositor output-boundary flip/slide/resize, fractional-output pointer input, screenshot comparisons of submenu-union alpha, compositor-delivered nested/multi-window Wayland input, live assistive technology and non-Linux runtime were not established. Adjusted/negative geometry, fractional raster/input conversion and alpha gaps have deterministic coverage; smaller configure delivery has live synthetic-input coverage. The separate layer-shell issue was neither edited nor begun. No new dependencies, version changes, lockfile changes, generated-file changes, staging or commits occurred. The preexisting unstaged `.gitignore` modification was preserved.

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
