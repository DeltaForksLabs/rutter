# rutter-winit (local fork)

This standalone workspace is a source fork of [winit v0.31.0-beta.3](https://github.com/rust-windowing/winit/releases/tag/v0.31.0-beta.3), from upstream commit `7d20408d33210c93bf036335b88ee083ca563a90`. Its published crate names and versions remain unchanged for compatibility; `rutter-winit` names the fork, not a new published crate. The upstream Apache-2.0 license and source files are preserved. No GitHub repository has been created for this fork.

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

### Popup coordinate and size corrections

On X11, popup `outer_position()` reports coordinates relative to the parent's content origin instead of desktop coordinates. `set_outer_position()` updates the stored positioner so parent/configure notifications preserve the requested placement. The movement regression checks both this relative API and the actual root-level X11 position after repositioning.

On Wayland, `with_position` without an explicit positioner anchors the popup's top-left rather than centering it on the requested point. Popup coordinates are converted from the parent's xdg geometry to its content-area coordinates; `set_outer_position()` goes through the reposition protocol. Initial popup surface size is kept separate from anchor-rectangle size, fixing an accidental 1×1 configure for a 1×1 anchor. Explicit caller-supplied positioners remain unchanged.

Rutter supplies a top-left positioner and uses the same requested origin for drawing and hit testing. It retains the in-window overlay while an asynchronous resize is pending and falls back after two seconds or when the configured geometry cannot fit the parent viewport. Native menus now request transparent surfaces and clear each frame to transparent before painting only the menu panels: the empty areas inside a submenu chain's rectangular union must reveal the parent instead of covering its layout. RGB-only softbuffer parents keep the existing overlay fallback. Raster tests verify transparent gaps and removal of collapsed submenu pixels.

Disposable Weston headless/pixman tests verify initial size/position, OpenGL dropdown/submenu expansion and selection, and the CPU overlay fallback using synthetic runner events. Run these checks sequentially, with each display-dependent test in its own Cargo process:

```bash
WAYLAND_DISPLAY=<test-socket> LIBGL_ALWAYS_SOFTWARE=1 cargo test --locked --offline --lib native_wayland_dropdown_stays_at_parent_anchor_during_submenu_expansion -- --ignored
WAYLAND_DISPLAY=<test-socket> cargo test --locked --offline --lib wayland_cpu_dropdown_falls_back_to_overlay_and_selects_submenu -- --ignored
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-wayland --offline
```

The native Wayland test needs an alpha-capable EGL driver; the CPU fallback test needs `wl_shm`. These tests do not compare compositor screenshot pixels, exercise Wayland Vulkan, or verify compositor-delivered Wayland pointer/grab behavior. X11 alpha compositing also requires a compositor; the Xephyr input tests do not establish its pixel composition.

Rutter now depends on this fork, the local [AccessKit](../rutter-accesskit-winit/README.md) and [glutin-winit](../rutter-glutin-winit/README.md) adapters, and `accesskit 0.25.1`. Its render backends and application handlers use the Winit 0.31 trait-window, surface-lifecycle and pointer-event APIs; `cargo tree -i winit` confirms a single Winit instance. Disposable-X11 integration tests render a Rutter button and native dropdown/context menus, deliver XTEST selections, dismiss an outside click and exercise submenu expansion. Rutter keeps menu state in the parent engine, painting to a popup when available and reverting to an in-window overlay otherwise. Runtime behavior on non-Linux platforms and screen-reader behavior on the popup are unverified.
