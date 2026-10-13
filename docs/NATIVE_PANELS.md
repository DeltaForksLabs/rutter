# Native desktop panels

`WindowConfig::with_panel(PanelConfig)` selects an opt-in, immutable panel role.
Rutter exports `PanelConfig`, `PanelConfigError`, `PanelEdge`, `PanelMargins`,
`PanelOutput` and `PanelFocus`; direct Winit users use `WindowAttributes::with_panel`.
Both native backends report `WindowType::Panel`. Ordinary windows keep their role.

```rust
use rutter::{PanelConfig, PanelEdge, PanelOutput, WindowConfig};

let panel = PanelConfig::new("example-panel", PanelEdge::Top, 0, 32)?
    .with_output(PanelOutput::Name("DP-1".into()))?
    .with_reservation(Some(32))?;
let window = WindowConfig::default().with_panel(panel);
# Ok::<(), rutter::PanelConfigError>(())
```

Names are exact, connection-local, case-sensitive, NUL-free, and bounded to
1–1024 bytes. One edge is mandatory. Length zero stretches along that edge;
positive lengths are centered compact layouts. Thickness is positive. Dimensions,
margins and reservations are **logical pixels** and fit signed coordinates.
Margins are nonnegative; the default is zero. Reservation defaults to `None`;
`Some(n)` requires a positive distance, with the anchored margin added to that
distance. Set it to the panel thickness to exclude the panel and its edge gap.
Defaults accept no keyboard focus and do not reserve space.
Compact panels reject nonzero perpendicular margins: left/right for Top/Bottom,
top/bottom for Left/Right. Wayland does not apply those margins without the
corresponding opposite anchors, so accepting them would produce different native
geometry. Stretched panels accept them on both backends.

Panels reject parents, alternate roles, missing settings, position/positioner,
ordinary size/min/max/increment hints, fullscreen, maximization and stacking
overrides. Decoration/resizability/activation defaults are replaced by panel
policy. Runtime geometry/role changes are unsupported: close and reopen with new
settings. Surface-size requests return `None`. Transparency does not make gaps
or rounded corners click-through; native input covers the full panel rectangle.

## X11 / EWMH

- Creates a **managed**, undecorated root-level `_NET_WM_WINDOW_TYPE_DOCK`, never
  an override-redirect popup or an ordinary toplevel pretending to be a panel.
- Requires a fresh valid root `_NET_SUPPORTING_WM_CHECK` whose supporting window
  identifies itself, and advertised `_NET_WM_WINDOW_TYPE` / DOCK atoms. Reserving
  also requires advertised `_NET_WM_STRUT_PARTIAL`. Failure is a creation error.
  This query does not use the fork's global WM-name cache.
  The supporting window must be distinct from, and an immediate child of, that
  root; supporting references and advertised capabilities require WINDOW32 and
  ATOM32 properties respectively.
- `PanelOutput::Name` resolves connected XRandR output names, including cloned
  outputs sharing a CRTC. Missing or ambiguous selection fails. `Default` selects
  the **unique output containing the pointer at creation** and pins its name for
  that native lifetime. A pointer outside outputs or overlapping cloned outputs
  requires an explicit name. There is no first-monitor fallback. Panels use the
  default X screen; nondefault screen/embedding/override-redirect/platform role,
  base-size and activation-token overrides are rejected.
  Pointer selection uses half-open output rectangles, so an adjacent boundary
  belongs to exactly one output. Output/CRTC identities, status, geometry and root
  size come from a fresh snapshot, with timestamps and root dimensions rechecked
  before acceptance. Up to three attempts handle topology changes during reads;
  transient read/write failures log context and retry on a later event-loop
  iteration rather than permanently retiring a valid panel.
- Layout uses full output geometry and physical root coordinates, never the
  work area reduced by the panel's own strut. Logical distances round to nearest
  physical pixel at output scale. Geometry is checked against root bounds and
  X11 wire coordinate/size limits before native allocation.
  The base reservation and anchored margin round independently, then add in
  physical pixels, matching the rounded panel thickness and gap when the base is
  its thickness. This can differ by one pixel from rounding their logical sum at
  fractional scale (base 1 plus margin 1 at scale 1.5 gives 2 + 2 = 4 pixels).
- `_NET_WM_STRUT_PARTIAL` is twelve CARDINAL32 values: physical root-edge
  distances and **inclusive** physical length ranges. Internal monitor borders
  cannot be represented without reserving unrelated root space, so **reserving
  panels must anchor at an outer root border**. Nonreserving internal-edge
  panels are supported. The WM controls actual work-area policy; compact struts
  may still reduce its rectangular work area across the entire screen.
- `PanelFocus::NonActivating` sets `WM_HINTS.input = false` and suppresses explicit
  focus requests. `Normal` sets input true for user-driven focus. Neither activates
  on creation/remapping. Active panel-owned menus capture the previous focused
  application and restore it only while still owning focus and while that target
  is viewable. Nonactivating menus leave application focus alone. Existing normal
  window/popup behavior is preserved.
  Immediate popup close also restores external application focus: the ownership
  and viewability checks plus CurrentTime restoration run under a short server
  grab, avoiding both stale cached event timestamps and intervening client focus
  changes.
- Hide/unmap removes the reservation; show/map recomputes layout and reinstalls
  it. Drop explicitly releases it before destroying the window. Creation failures
  destroy the provisional native window, including failures after mapping.
  Mapping notifications observe fresh server state and reconcile the latest
  requested visibility; queued old notifications cannot undo a rapid hide/show.
- RandR screen/output changes and Xft/XSettings scale updates recompute geometry
  and reservation. Xlib's RandR converters are initialized through the existing
  `x11-dl` dependency; **libXrandr is required at runtime** for panels. A missing
  pinned output or impossible new geometry retires that panel instead of migrating
  it. Children retire deepest-first, then the panel, with `CloseRequested` followed
  by `Destroyed`. New raw window/display handles return `Unavailable`; ignoring
  close or restoring geometry cannot remap a retained retired panel.
  Output-change notifications are selected independently of CRTC changes, so
  unplugging a named clone is detected even when its shared CRTC survives. Every
  tree handle is revoked before any terminal callback, and new children under
  retained terminal parents are rejected. Terminal XIDs remain input-routing
  tombstones after owner drop; queued core/XI2/IME/redraw events are suppressed.
  Retirement also clears active keyboard focus before callbacks. Global XKB state
  continues updating without dispatch to terminal menus. Late DestroyNotify for a
  tombstoned ancestor only cleans native routes/IME, without closing its already
  retired retained descendants again. Successful topology/scale retries publish
  ScaleFactorChanged once, using the last notified scale and committed geometry.
- Retirement unmaps but retains the native drawable until its owning window is
  dropped, so retained renderers can release EGL/Skia resources safely. Rutter's
  frame/present/resize guards reject retired handles. Release child renderers before
  parent renderers/windows. A third party destroying the backing X window is a
  different X11 failure: handles are revoked on DestroyNotify, but cached external
  EGL drawables cannot be preserved against server-side destruction.

`SurfaceMetrics.output_name` uses the neutral `Window::panel_output_name()`
accessor. The legacy `layer_output_name()` remains available. X11 panels report
**no layer-shell version**: no Wayland capability is fabricated.

## Wayland and other platforms

The neutral request translates before role construction to a Top-layer
`LayerShellConfig`: edge anchor plus both perpendicular anchors for stretch,
positive compact length otherwise, logical thickness/margins/reservation, named
output or a null compositor-selected output.
The translated exclusive distance is the base reservation **plus its anchored
margin**, independent of setter order; `reservation()` still returns the base.
For example, `Some(32)` with an 8-pixel anchored gap sends Reserve(40). With no
reservation, margins alone leave the exclusive policy None. Wayland receives
these integer logical values; physical scaling/rounding is compositor-controlled.
NonActivating maps to keyboard None;
Normal maps to OnDemand and **requires negotiated layer-shell v4**. Existing
layer creation, configure acknowledgment, output retirement, rendering and
layer-owned popup ownership are reused. Default output policy can migrate under
compositor policy, unlike the pinned X11 default. Nonreserving Wayland panels
respect compositor reservations; X11 layout uses full output bounds. Wayland
visibility hints retain the existing layer-shell behavior: close/reopen to release
reservations; X11 hide/show is not a portable auto-hide promise.
Neutral Wayland panels reject activation-token overrides before allocating a
surface or native role. The explicit LayerShell activation contract is unchanged.

`with_layer_shell()` remains strictly Wayland-only, with its existing signed
margins, stacking and exclusive policies. See [the layer-shell contract](WAYLAND_LAYER_SHELL.md).
Android, AppKit, Orbital, UIKit, Web and Win32 explicitly reject neutral panels.

## Validation

Run compilation, tests and native probes **sequentially**:

```sh
CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib --no-run
python3 tests/native/x11_panel_smoke.py --openbox /absolute/path/to/openbox
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target cargo test --manifest-path vendor/rutter-winit/Cargo.toml -p winit-core -p winit-x11 --locked --offline
```

The native driver starts only owned Weston, rootful Xwayland and Openbox instances,
with private configuration/runtime directories, non-desktop display numbers and
cleared inherited display/session/DBus addresses. It validates missing-WM rejection,
four managed DOCK rectangles, nonactivating WM hints, inclusive strut values,
work-area/maximized Settings bounds, inactive and active popup focus restoration,
separate-process and immediate-close focus restoration, rapid hide/show/close
release, malformed EWMH root/self references, surviving-panel reposition on root
resize, child-first
terminal impossible-geometry retirement, sticky raw-handle revocation and retained
OpenGL renderer cleanup. A first terminal callback checks all tree handles and
attempts rejected child creation; an active nested child drops its renderer during
dispatch, with queued input checked for absence after Destroyed. The driver also
verifies retained descendants across ancestor destruction and controlled XKB/core-key
dispatch through terminal focus. Scale-failure/recovery notification is covered by
a pure controlled seam, not a live DPI experiment. The driver does
not require or modify a production session.

Pure tests cover all edges, stretch/compact geometry, scales 1/1.5/2, root origins,
internal-edge restrictions, margins/ranges and API defaults/clones/conflicts.
They also simulate named clone reassignment/removal, topology generation/status
mismatches and adjacent pointer boundaries. Physical clone unplug and multi-monitor
WM behavior remain live-test gaps; root resize exercises the same
recompute/retirement path without unplugging hardware.
Non-Linux native targets are source-updated but not cross-compiled in this validation.
The neutral Wayland probe renders a Top panel with an 8-pixel anchored gap and
checks the client trace for margin 8 and exclusive distance 40 from base 32.
This verifies protocol transmission, not pixel-measured gap/work-area behavior on
every edge or fractional-scale compositor rounding.
