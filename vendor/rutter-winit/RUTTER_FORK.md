# rutter-winit (local fork)

This standalone workspace is a source fork of [winit v0.31.0-beta.3](https://github.com/rust-windowing/winit/releases/tag/v0.31.0-beta.3), from upstream commit `7d20408d33210c93bf036335b88ee083ca563a90`. Its published crate names and versions remain unchanged for compatibility; `rutter-winit` names the fork, not a new published crate. The upstream Apache-2.0 license and source files are preserved. No GitHub repository has been created for this fork.

## Backend-neutral panel role

The fork adds independent validated `PanelConfig` settings and `WindowType::Panel`.
Wayland translates them into its existing real layer-shell role; X11 creates a
checked managed EWMH DOCK with physical partial struts, pinned XRandR output identity,
nonactivating input policy and terminal child-first geometry/output retirement.
X11 keeps retired drawables allocated for renderer cleanup, but revokes new raw
handle access. The existing `x11-dl` library loads and retains RandR's Xlib event
converters; otherwise Xlib discards the events selected through x11rb. No new Cargo
dependency or fork version change is involved. Unsupported backends reject the role.
The explicit LayerShell API remains Wayland-only. Neutral output-name metrics do
not invent a layer-shell capability on X11.

Panel updates use fresh, generation-checked RandR snapshots and reconcile the latest
requested visibility. Transient failures retry; confirmed output loss revokes the
entire panel tree before callbacks, with terminal XID tombstones suppressing queued
input after owner drop. Compact perpendicular margins and neutral Wayland activation
tokens are rejected before native allocation.

See [the native panel contract](../../docs/NATIVE_PANELS.md) for APIs, backend
differences, internal-root-border reservation limits, lifecycle and isolated probes.

## X11 popup experiment

The X11 backend now accepts `WindowType::Popup` with a **live X11 parent in the same event loop**. It creates an override-redirect window under the X screen root (not an X11 child clipped to the parent), sets `_NET_WM_WINDOW_TYPE_POPUP_MENU` and `WM_TRANSIENT_FOR`, and places it relative to the parent content origin. `with_positioner` and `set_positioner` use winit's shared xdg-style placement algorithm with monitor bounds; `with_position` without a positioner gives the popup a parent-relative top-left origin. Active popups request X keyboard focus; inactive popups do not. Visible popups grab **all enabled XInput2 master pointers**, including new masters after a hierarchy change; a button press outside the popup emits `WindowEvent::CloseRequested` and releases its grabs. Hiding or dropping the popup restores the preceding visible popup's grabs, if any. Parent `ConfigureNotify` events reposition associated popups. If the parent is destroyed, the backend hides its descendant popups, releases their grabs, and sends them `CloseRequested` before reporting the parent's destruction. Applications must still drop their popup handles in response.

Example, from an `ApplicationHandler::can_create_surfaces` callback:

```rust,no_run
use raw_window_handle::HasWindowHandle;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowType};

fn open_menu(event_loop: &dyn ActiveEventLoop, parent: &dyn Window) {
    let handle = parent.window_handle().unwrap().as_raw();
    // SAFETY: the parent is alive for at least as long as the popup.
    let attributes = unsafe {
        WindowAttributes::default()
            .with_window_type(WindowType::Popup)
            .with_parent_window(Some(handle))
    }
    .with_surface_size(winit::dpi::PhysicalSize::new(180, 240))
    .with_position(winit::dpi::PhysicalPosition::new(10, 40));
    let popup = event_loop.create_window(attributes).unwrap();
    // Retain `popup` in application state; drop it before the parent.
}
```

Active X11 popups also reacquire keyboard focus when shown again. Before hiding or dropping a popup that still owns keyboard focus, the backend restores the preceding visible active popup or its live, viewable application parent. It leaves focus alone when another window has taken it and skips hidden or dropped restore targets. This does not depend on a window manager: X11's `RevertToParent` alone would focus the screen root because these popups are root-level windows, despite `WM_TRANSIENT_FOR`.

**Remaining limits and options:** X11 has no `xdg_popup` protocol. Outside presses are consumed rather than delivered to the underlying window. XI2 `ReplayDevice` cannot replay events from the **active asynchronous grab** used here; forwarding with XTEST or `SendEvent` would fabricate input. Global synchronous passive grabs could replay presses, but they may conflict with other clients' grabs and system shortcuts. Likewise, XI2 active pointer grabs explicitly do not capture XI2.2 touch sequences. A separate global passive-touch grab with accept/reject ownership could handle them but would interfere with other clients' touch processing. The user chose to preserve input isolation rather than add these global grabs. Floating slave pointers (not attached to a master pointer) are also outside the master-pointer grab. If a pointer grab fails, initial popup creation returns an error; showing a hidden popup logs an error and hides it again. A keyboard-focus request is not equivalent to Wayland's popup grab. Window-manager and compositor behavior may vary. Rutter now connects built-in dropdown and context-menu widgets to native parented popups, but other overlays and untested platforms remain outside that integration.

Run checks from the Rutter repository root (validation must be sequential):

```bash
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --test popup -- --ignored # requires DISPLAY to be an X11 display
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --test popup_movement -- --ignored # requires DISPLAY to be an X11 display
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --test popup_dismissal -- --ignored # only on a disposable X11 display with XTEST
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --test popup_parent_destroy -- --ignored # requires DISPLAY to be an X11 display
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --test popup_grab_stack -- --ignored # requires DISPLAY to be an X11 display
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --test popup_multi_pointer -- --ignored # only on a disposable X11 display; modifies device hierarchy
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --test popup_selection -- --ignored # only on a disposable X11 display with XTEST; selects a test menu option
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-x11 --test popup_keyboard_focus -- --ignored # only on a disposable X11 display with XTEST; verifies focus restoration and native key delivery
```

The `popup_selection` test represents two menu options as hit regions in its own application handler. It injects a left-click through XTEST into the second region and checks press, release, the selected option, and that the popup remains open with its pointer grab. This verifies X11 event delivery, **not** rendering or selecting an actual Rutter menu widget.

## Integration status

### Wayland layer-shell role

The fork now supports `WindowAttributes::with_layer_shell(LayerShellConfig)` on
Wayland, using the existing SCTK 0.21 layer-shell implementation and event queue.
The builder selects `WindowType::LayerShell`, which the created window reports;
the bare role without settings is rejected, and ordinary windows keep `Window`.
It creates a real layer role on an unassigned surface before the initial empty
commit; configure is acknowledged before buffer presentation. `WindowState`
shares existing scale/viewport, pointer, frame, child-popup and close handling.
Layer surfaces have no xdg_surface/toplevel and never issue xdg geometry requests.
Their settings are immutable; untyped size hints return None. Unsupported
platforms, missing protocol, unavailable/ambiguous output names and unsupported
keyboard versions fail instead of converting to ordinary windows.

Public connection-independent types live in `winit-core/src/window/layer_shell.rs`;
construction/configure dispatch is focused in `winit-wayland/src/layer_shell.rs`.
`ActiveEventLoop::layer_shell_version`, `Window::layer_shell_version`,
`Window::layer_output_name` and `Window::surface_logical_size` are defaulted methods.
The logical snapshot avoids fractional buffer-rounding error. Targeted wl_output
removal and compositor `closed` are non-vetoable terminal retirement without
reattachment; recreation requires a new request. Ordinary toplevel close requests
remain vetoable. Terminal flags revoke frontend/raw-handle access even when callers
retain strong window/state references. Popup roles are destroyed deepest-first,
then the layer role; native maps and pending events are retired before child-first
`CloseRequested`/`Destroyed` delivery. Retained SCTK owners keep only the backing
surface allocation for renderer destruction: cached pointers must not be used for
further presentation. Release graphics resources before the retained window/state.
The low-level `Window::surface()` is cleanup-only once terminal. Repeated SCTK role
destructors target dead ObjectIds and are rejected locally by wayland-client; final
owner release destroys the backing wl_surface. Retained-window and actual OpenGL
teardown probes are documented in the root guide.
Layer-owned menus create an xdg_popup with null xdg parent, associate through
get_popup before grab/commit, and retain the existing popup lifecycle and overflow.
Failed initial popup creation removes provisional child ownership.

See [the root layer-shell guide](../../docs/WAYLAND_LAYER_SHELL.md) for typed
settings, input/exclusive-region semantics and sequential test commands. Live
labwc tests verify real GL rendering, edge pixels, compact length, reservations and
cleanup, compositor-delivered popup input, resize/integer/fractional scale and
targeted output removal. The available FRAYM build stalls on its first layer
request; its suspected layer-map lock-lifetime defect and remaining acceptance
gaps are documented separately. No dependency or version changes were needed.

### Popup coordinate and size corrections

On X11, popup `outer_position()` reports coordinates relative to the parent's content origin instead of desktop coordinates. `set_outer_position()` updates the stored positioner so parent/configure notifications preserve the requested placement. The movement regression checks both this relative API and the actual root-level X11 position after repositioning.

On Wayland, `with_position` without an explicit positioner anchors the popup's top-left rather than centering it on the requested point. Popup coordinates are converted from the parent's xdg geometry to its content-area coordinates; `set_outer_position()` goes through the reposition protocol. Initial popup surface size is kept separate from anchor-rectangle size, fixing an accidental 1×1 configure for a 1×1 anchor. Explicit caller-supplied positioners remain unchanged.

Rutter supplies a parent-inside 1×1 logical anchor, an offset to the requested union origin, and all slide/flip/resize constraints. Native sizing is independent of the parent's drawable extent (1024×768 logical policy budget; at most 320 logical pixels per panel). Core Wayland does not supply an authoritative shell work area: this budget is finite client policy, while the compositor's configured rectangle controls the actual drawable area. Overflow and negative parent-relative coordinates are supported. Rutter retains one resolved panel list for drawing, input, scrolling, lower-layer hover and dropdown accessibility, uses the popup's actual pixel scale, and accepts resized panels with scrollable content.

The defaulted `Window::request_popup_layout(positioner, size)` method combines popup size and placement. The Wayland override updates one positioner under the popup state lock and issues one `xdg_popup.reposition`; it returns `None` until asynchronous configure delivery. Position-only popup configures also produce `SurfaceResized`, even when dimensions are unchanged, so callers can acknowledge the transaction and read the new `outer_position`. Protocol versions without reposition support time out through Rutter's existing two-second fallback deadline. Other backends retain default size-request/positioner behavior.

The defaulted `Window::popup_surface_geometry(parent_scale_factor)` returns a `PopupSurfaceGeometry` snapshot: parent-content logical position, popup physical buffer size, popup scale, and an optional configure serial. Its default converts parent-relative physical positions using the parent scale; the Wayland override reads the logical configure, buffer size, scale and serial together under the state lock. This avoids conflating parent-buffer pixels with popup-buffer pixels when their scales differ. Rutter uses this snapshot to reconcile configures before pointer/wheel input and presents changed geometry before selection. Configuration alone does not suppress the parent overlay: a successful frame for the current layout/drawable is required. Requested/configured popup buffers are capped at 4096 physical pixels per axis and 4,194,304 pixels per buffer, including backend allocation/resize checks and Vulkan's selected swapchain extent.

Native menu frames clear to transparent before painting only their panels: empty areas inside a submenu chain's rectangular union reveal the parent. RGB-only softbuffer parents retain a usable overlay. Rutter's defaulted single/multi-window `native_menu_unavailable` hooks receive a menu ID, reason and `overlay_usable` flag through the parent-owned application path. An overlay too small for a complete row is dismissed rather than treated as successful native presentation; subsequent unusable opening attempts remain observable. Raster tests verify transparent gaps, fractional-scale painting, configured scrolling and removal of collapsed submenu pixels.

Disposable Weston headless/pixman tests verify initial size/position, OpenGL dropdown/submenu expansion and selection, and the CPU overlay fallback using synthetic runner events. Run these checks sequentially, with each display-dependent test in its own Cargo process:

```bash
WAYLAND_DISPLAY=<test-socket> LIBGL_ALWAYS_SOFTWARE=1 cargo test --locked --offline --lib native_wayland_dropdown_stays_at_parent_anchor_during_submenu_expansion -- --ignored
WAYLAND_DISPLAY=<test-socket> cargo test --locked --offline --lib wayland_cpu_dropdown_falls_back_to_overlay_and_selects_submenu -- --ignored
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-wayland --offline
```

The native Wayland tests need an alpha-capable EGL driver; CPU fallback needs `wl_shm`. Rutter's overflow fixtures now cover both 800×30 and 30×600 parents, ContextMenu and DropdownMenu submenu chains, unchanged parent sizes, unusable overlays, smaller popup configures with scrolling/keyboard reveal, and two-parent `MultiWindowRunner` selection/failure routing. They passed sequentially on Weston headless/pixman and private rootful Xwayland `-shm`, using software GL; thin context, nested context and multi-window checks also passed at integer output scale 2. See the root README for exact filters. The existing Wayland placement and CPU submenu fixtures passed too.

`native_wayland_compositor_pointer_selects_outside_thin_parent` additionally passed with XTEST sent only to a disposable nested Weston's X11 host. Weston delivered genuine Wayland seat events to the parent and popup; the fixture requires one parent press, one popup press and a selection outside the unchanged thin parent, without synthetic runner events. The Weston scene graph confirmed a real ARGB `xdg_popup`. This does not establish compositor screenshot pixels, fractional-output input, native Vulkan or live accessibility. Spontaneous output-boundary constraints were not demonstrated: Weston did not shrink a 320-pixel menu on the short-output probe. Explicit smaller configure delivery and deterministic adjusted/negative geometry cover the client handling; other compositor policies and compositor-delivered nested/multi-window Wayland input need further live validation.

Rutter now depends on this fork, the local [AccessKit](../rutter-accesskit-winit/README.md) and [glutin-winit](../rutter-glutin-winit/README.md) adapters, and `accesskit 0.25.1`. Its render backends and application handlers use the Winit 0.31 trait-window, surface-lifecycle and pointer-event APIs; `cargo tree -i winit` confirms a single Winit instance. Disposable-X11 integration tests render a Rutter button and native dropdown/context menus, deliver XTEST selections, dismiss an outside click and exercise submenu expansion. Rutter keeps menu state in the parent engine, painting to a popup when available, retaining a usable overlay on failure, and notifying application logic when a thin-parent overlay must be dismissed. Runtime behavior on non-Linux platforms and screen-reader behavior on the popup are unverified.
