# rutter-accesskit-winit (local fork)

This standalone crate is a source fork of [accesskit_winit 0.34.1](https://crates.io/crates/accesskit_winit/0.34.1) (upstream source revision `ce8164ba92995cfa86005b6259115e08c8244253`, `adapters/winit`). It keeps the published crate name and version, upstream copyright notices and Apache-2.0 license. Its `winit` dependency points to the local [rutter-winit](../rutter-winit/RUTTER_FORK.md) fork at `0.31.0-beta.3`; Rutter now uses both forks.

The native AccessKit adapters remain in place on Unix (AT-SPI), Windows (UI Automation), macOS, iOS, and Android; unsupported platforms keep the upstream no-op adapter. `Adapter::with_direct_handlers`, `process_event`, and `update_if_active` retain their responsibilities. On Unix, `Moved` and `SurfaceResized` update desktop-space accessibility bounds using `Window::surface_position()` relative to `outer_position()`; `Focused` still updates focus state.

The exact-version path dependency is intended for local builds only. The published Winit beta does not contain the local X11 popup changes, so publishing this adapter as-is would not preserve the tested integration.

## API differences from upstream 0.34.1

- `ActiveEventLoop` and `Window` are now trait objects (`&dyn ActiveEventLoop`, `&dyn Window`). Construct adapters **before** showing windows, as with upstream.
- Winit 0.31's `EventLoopProxy` only wakes the event loop, so `with_event_loop_proxy` and `with_mixed_handlers` also take a `std::sync::mpsc::Sender<accesskit_winit::Event>`. Drain the associated receiver from `ApplicationHandler::proxy_wake_up`; process each event for its `window_id`. If the receiver has been dropped, no wakeup is sent. If activation is deferred, send a full accessibility tree via `update_if_active`.
- Winit 0.31 only supports raw-window-handle 0.6. The upstream `rwh_05`/`rwh_06` feature switch was removed; 0.6 is always enabled. The `accesskit` version is `0.25.1`, as required by upstream 0.34.1; Rutter uses the same version.

## Validation

Run from the repository root, one resource-intensive command at a time:

```bash
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --offline
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --test direct_handlers --offline -- --ignored # only on a disposable X11 display, e.g. DISPLAY=:98 on Xephyr
CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-accesskit-winit/Cargo.toml --all-targets --offline -- -D warnings
```

The X11 test exercises creation of a hidden real window, a direct adapter and delivery of resize, move and focus events. Rutter's own X11 test verifies adapter creation before first visibility and recreation with a new GL surface. Neither test verifies an AT-SPI client or UI Automation, AppKit, iOS, or Android behavior.
