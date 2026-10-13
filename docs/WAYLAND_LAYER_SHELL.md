# Wayland layer-shell surfaces

For the independent `PanelConfig` / `WindowConfig::with_panel` API, including the
native X11/EWMH backend and its geometry/focus limitations, see
[Native desktop panels](NATIVE_PANELS.md). `with_layer_shell` retains the strictly
Wayland-only contract documented here.

## Public request and ownership

Rutter exports `LayerShellConfig`, `LayerAnchors`, `LayerOutput`, `Layer`,
`LayerMargins`, `LayerExclusiveZone`, `LayerKeyboardInteractivity` and
`LayerShellConfigError`. Select the role with `WindowConfig::with_layer_shell` in a
normal `SurfaceRequest`. The existing multi-window runtime owns window handles,
graphics, application state and native-menu routes. Worker threads can send
application messages through the existing ingress; they do not own Wayland objects.

The fork uses its existing Smithay Client Toolkit dependency. It binds
`zwlr_layer_shell_v1` at versions 1–4, creates the role on an unassigned `wl_surface`,
sets pending role state and makes an initial **bufferless** commit. SCTK sends
`ack_configure` before the configure callback; window creation waits for that
callback before a graphics backend can attach a buffer. An initial close or
connection error retires provisional window state/requests. There is no conversion
of a mapped `xdg_toplevel` into a panel and no silently selected preview fallback.
The fork builder selects `WindowType::LayerShell` and the native window reports
that role. A bare LayerShell role without settings, a simultaneous Popup role or
a native window parent is rejected rather than creating a toplevel.

Ordinary geometry, stacking, decoration and maximize hints do not replace layer
settings. Logical panel size comes from `LayerShellConfig`, not physical
`WindowConfig::with_inner_size`. Role settings are immutable for one native surface:
close and explicitly reopen to change them. Untyped Winit size requests on layer
surfaces return `None`; they cannot invalidate the validated anchor/size combination.
Wayland visibility hints do not implement auto-hide. Closing a surface releases its
reservation; automatic hide/show and editable island input regions are follow-ups.

## Geometry, work area and input

- Sizes, margins and exclusive zones use **logical pixels**. Dimensions must fit
  signed logical coordinates. Zero width requires LEFT and RIGHT; zero height
  requires TOP and BOTTOM. Configured zero axes use the client's positive fallback.
- Full horizontal: TOP | LEFT | RIGHT, width 0, positive height. Full vertical:
  LEFT | TOP | BOTTOM, positive width, height 0. Replace TOP/LEFT with BOTTOM/RIGHT
  for the opposite edge.
- Compact horizontal: TOP or BOTTOM with a positive width and height. Compact
  vertical: LEFT or RIGHT with both positive dimensions. One edge centers the
  non-anchored axis. Fixed lengths are not changed into output-spanning requests.
  Two perpendicular edges place a fixed surface at a corner.
- Signed margins apply to anchored edges. Zero margins are the default. Negative
  margins intentionally permit overlap. The protocol's exclusive distance includes
  the margin: for a 32-pixel panel with an 8-pixel edge gap, reserve 40 if both should
  be excluded from the ordinary work area.
- `LayerExclusiveZone::None` is protocol 0: respect other reservations, reserve
  nothing. `Ignore` is protocol -1: ignore other reservations. `Reserve(n)` requires
  a positive signed-range distance and exactly one edge, or that edge plus both
  perpendicular edges. Ambiguous corner/all-edge reservations are rejected.
  A compact panel can reserve a whole edge strip, **not just its own length**;
  compositor policy controls the resulting work area. Rutter does not recursively
  compute panel size from the work area reduced by its own reservation.
- `Layer::Top`, zero margins, no reservation and keyboard `None` are defaults.
  `OnDemand` requests ordinary user-driven focus and requires protocol version 4;
  older versions fail without downgrade. `Exclusive` is explicit and can steal
  ordinary application keyboard focus.
- Transparency changes opaque-region/presentation behavior, **not the input
  region**. The native input region remains the full surface rectangle, including
  rounded corners, transparent gaps and spaces between islands. Visual islands
  can occupy one compact panel, but are not automatically click-through. Transparent
  presentation needs an alpha-capable backend; RGB-only softbuffer cannot provide it.

## Output identity, negotiation and lifecycle

`LayerOutput::Name(name)` matches the exact, case-sensitive wl_output/xdg-output name
advertised on Rutter's connection. It does not match make/model descriptions,
enumeration order, primary-output guesses or objects from another connection.
Unavailable or ambiguous names fail. Namespace/output names are nonempty,
NUL-free and bounded to 1024 bytes. FRAYM IPC can supply inventory names, but remains
a distinct discovery boundary. `LayerOutput::Compositor` passes a null output:
the compositor chooses and may migrate that surface according to its own policy.

The fork exposes `ActiveEventLoop::layer_shell_version()` for negotiated support;
`Window::layer_shell_version()` reports an actual layer role, and returns None for
ordinary windows. Missing layer-shell and every non-Wayland backend return a clear
creation error. Multi-window creation errors follow the existing typed
`MultiWindowRunError` path; failed opens terminate that run. Applications must
explicitly choose an ordinary labeled preview rather than assuming native support.

`MultiWindowAppLogic::surface_metrics_changed(state, surface, SurfaceMetrics)` is a
defaulted callback on the current runner thread. It reports the initial configured
logical viewport, physical buffer size, scale and subsequent size/scale/output
changes, coalesced after native dispatch. Fractional buffer rounding does not change
the logical viewport in this snapshot. Output name may initially be None until an
output is entered, if the compositor does not advertise one, or if the surface
enters multiple outputs and no unique name can be reported. Duplicate snapshots
are suppressed. Native menu geometry is not reported as a panel resize.
Releasing native surfaces clears the snapshot cache, so resume reports a new
initial configuration even when its dimensions match the previous native lifetime.

Compositor closure and removal of a **targeted** wl_output are terminal native
events, not vetoable application close requests. The fork revokes the layer and
its popup descendants immediately, destroys popup roles deepest-first before the
layer role, removes native routes and pending events, and delivers `CloseRequested`
then `Destroyed` for each retired child and parent. Ignoring `CloseRequested` or
retaining a `Window`/`Arc<Mutex<WindowState>>` cannot preserve the roles or reservation.
Ordinary xdg-toplevel `CloseRequested` remains vetoable.

After revocation, new raw window/display handle access returns `Unavailable`,
redraw/presentation notifications and native mutations do not issue requests, and
geometry reports an unavailable/default snapshot. Rutter's CPU, OpenGL and Vulkan
backends reject frame begin, presentation and resize before using retired native
resources. Direct fork renderers must stop rendering on terminal closure/Destroyed
and release their graphics resources **before** releasing the retained window.
The role-owning SCTK allocation retains its backing `wl_surface` for this cleanup
only, so pointers previously cached by EGL remain valid during destruction. This
does not preserve a live layer/popup role or allow remapping. The low-level Wayland
`surface()` accessor is cleanup-only after retirement; callers must not commit or
attach buffers through cached raw handles/proxies. SCTK destroys the backing
surface when the last window/state owner is released.

The multi-window runner retires its menu, renderer and route; `surface_closed`
observes removal. The panel is never reattached to an
unrelated output. A later output with the same name does not automatically reopen
the panel: the application must submit a new explicit request. Runner shutdown
releases surfaces too. Role destruction precedes wl_surface destruction through
SCTK ownership, releasing exclusive reservations. Ordinary Settings windows remain
independent toplevels and may follow the compositor's normal output policy.

## Layer-owned menus

Built-in native ContextMenu and DropdownMenu use the current menu/render owner.
For a layer parent the fork creates `xdg_popup` with a **null xdg parent**, calls
`zwlr_layer_surface_v1.get_popup` before grab/initial commit, and routes popup
configure/dismissal/destruction through existing child ownership. Popup creation
failure removes the provisional child route. Menus retain the first issue's
independent sizing, overflow, scale, scrolling, transparent painting, focus and
observable fallback behavior. Select and generic popovers remain in-surface.

## Reproducible validation

Build tests first; graphical checks must run sequentially:

```sh
CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib --no-run
python3 tests/native/layer_shell_smoke.py --fraym /absolute/path/to/fraym
# Alternate layer-supporting compositor (requires Weston, Xwayland, labwc, xprop,
# libX11/libXtst, software EGL, and wlr-randr for scale/removal probes):
python3 tests/native/layer_shell_smoke.py --labwc /absolute/path/to/labwc --wlr-randr /absolute/path/to/wlr-randr
python3 tests/native/layer_shell_smoke.py --labwc /absolute/path/to/labwc --wlr-randr /absolute/path/to/wlr-randr --remove-output
# Direct fork consumer retains layer, nested popups and metadata; ignores close:
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-wayland --locked --offline --lib --no-run
python3 tests/native/layer_shell_smoke.py --labwc /absolute/path/to/labwc --retained-layer
python3 tests/native/layer_shell_smoke.py --labwc /absolute/path/to/labwc --wlr-randr /absolute/path/to/wlr-randr --remove-output --retained-layer
# Real retained OpenGL parent/popup renderers reject further use and clean up:
python3 tests/native/layer_shell_smoke.py --labwc /absolute/path/to/labwc --wlr-randr /absolute/path/to/wlr-randr --remove-output --retained-renderer
```

The driver creates private Weston/Xwayland sockets for the host and clipboard,
clears inherited display/session addresses, and sends XTEST only to its owned host.
`--keep` preserves logs under `/tmp/opencode`. Each ignored Rust fixture runs in its
own process. The unsupported test uses Weston without layer-shell and also creates
an ordinary window. The layer test renders four colored panels plus Settings and
uses actual compositor-delivered Wayland presses for menu selection.

**Established on labwc 0.7.1 / wlroots 0.17.1:** compositor pixel probes at all four
edges and outside compact lengths; real OpenGL frames; work-area height 760 with a
40-pixel reservation on an 800-pixel output, then 800 after close; null-parent popup
association and selection below the unchanged 40-pixel parent; retained Settings
focus; initial empty commit/ack/buffer ordering; output resize; scale 2 and 1.5
notifications; two-output targeting; wl_output removal on disable closes all four
targeted panels, removes routes and preserves Settings without migrating panels.
The removal probe leaves a second output enabled; it is not physical hotplug.

Retained-consumer probes also establish terminal child-first role destruction,
new handle revocation, no post-retirement redraw and final backing-surface release
while ordinary toplevel close requests remain ignorable. The `closed` probe invokes
the production SCTK callback explicitly on a real mapped layer; the targeted-output
probe uses actual compositor output removal. A separate probe retains actual OpenGL
layer/popup renderers, rejects subsequent begin/present/resize, and destroys EGL/Skia
resources after retirement. CPU/Vulkan retirement guards are compiled and linted;
their live terminal teardown is not established by the OpenGL probe.

**FRAYM gap:** the available `fraym_compositor/target/debug/fraym` starts nested and
advertises layer-shell v4, but ceases responding after the first get_layer_surface;
the subsequent display sync receives no reply. Its inspected
`src/handlers/layer_shell.rs::new_layer_surface` matches directly on
`layer_map_for_output(...).map_layer(...)`, retaining the map guard across the
success arm, where it accesses the same layer map again. This points to a
compositor-side lock-lifetime deadlock, not a protocol error reported by FRAYM.
That checkout was not modified. FRAYM acceptance remains pending a corrected
compositor build. Labwc results do not establish FRAYM success.

Physical hotplug/output recreation, live versions 1–3 negotiation, fractional-scale
menu input, layer-parent nested dropdown chains, native Vulkan, non-Linux runtime
and live assistive technology remain unvalidated. Deterministic tests cover
geometry/identity/range/keyboard validation; the existing first-issue tests cover
ordinary-parent nested menus and overflow.
