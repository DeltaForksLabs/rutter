# rutter-glutin-winit (local fork)

This standalone crate is a source fork of [glutin-winit 0.5.0](https://crates.io/crates/glutin-winit/0.5.0) (upstream revision `0811b6c753334f1681c7db8a48e407596cfa9366`, `glutin-winit`). It retains the published crate name and version, the upstream MIT license, and depends on unmodified [glutin 0.32.3](https://crates.io/crates/glutin/0.32.3) plus the local [rutter-winit 0.31.0-beta.3](../rutter-winit/RUTTER_FORK.md). **Glutin itself is not forked**: only `glutin-winit` directly uses Winit and needs an adaptation. Rutter now depends on both local forks.

## API and behavior changes

- `DisplayBuilder::build` and `finalize_window` now take `&dyn ActiveEventLoop` and return a `Box<dyn Window>` (optional for the builder). Creating a window before the event loop becomes active is no longer supported in Winit 0.31. `finalize_window` returns Winit's `RequestError` rather than the former `OsError`.
- `GlWindow` is implemented for `dyn Window`. Its surface creation/resizing methods use `Window::surface_size()`, not the former `inner_size()`. Window handles remain raw-window-handle 0.6.
- When GLX requires a specific X11 visual, the bridge updates existing `WindowAttributesX11` instead of overwriting other X11 attributes. An incompatible platform-attribute object returns `RequestError::NotSupported`; an unadorned window receives X11 attributes for the visual. Transparency is still disabled for GL configs that do not support it.
- The existing EGL/GLX/WGL/CGL display-selection and feature logic is retained. `glutin` is pinned to `0.32.3` to match Rutter's current dependency; no core GL code is modified.

Run from the Rutter repository root, one resource-intensive command at a time:

```bash
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-glutin-winit/Cargo.toml --offline
CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-glutin-winit/Cargo.toml --test display_builder --offline -- --ignored # only on a disposable X11 display with an EGL or GLX driver (e.g. DISPLAY=:98 on Xephyr)
CARGO_TARGET_DIR=target cargo clippy --manifest-path vendor/rutter-glutin-winit/Cargo.toml --all-targets --offline -- -D warnings
```

The bridge's X11 integration test selects a real GL config, creates a compatible Winit window and surface, and makes a GL context current. Rutter's separate X11 integration test also renders a widget through its GL backend, clicks it, and recreates its surface. Neither test validates other native platforms. This bridge targets the local Winit source fork, not the published beta with the same version; it is not ready for registry publication as-is.
