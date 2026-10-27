# Native arbitrary-content popups

`MultiWindowRunner` accepts a native popup through
`WindowConfig::with_popup(PopupConfig)`. Content comes from the ordinary
`MultiWindowAppLogic::view(state, popup_surface_id)`: calendars, live clocks,
buttons, custom widgets, and normal source-aware messages all use the popup's
own engine, layout, input route and accessibility tree.

```rust
use rutter::{PopupAnchor, PopupConfig, PopupConfigError, PopupConstraints, PopupDirection,
             SurfaceCommand, SurfaceId, SurfaceRequest, WindowConfig};

fn open_popup() -> Result<SurfaceCommand, PopupConfigError> {
    let popup = PopupConfig::new(
        SurfaceId::PRIMARY,
        PopupAnchor::new(8, 0, 96, 32)?, // logical parent-content coordinates
        384, 448,                       // logical popup size
        PopupDirection::Below,
    )?
    .with_offset(0, 4)?
    .with_constraints(PopupConstraints::SLIDE_X | PopupConstraints::SLIDE_Y)?;
    Ok(SurfaceCommand::Open(SurfaceRequest::new(
        SurfaceId::new(7), WindowConfig::default().with_popup(popup),
    )))
}
```

## Native roles and support

- **Wayland:** a real `xdg_popup` parented to a registered ordinary window,
  backend-neutral panel, or layer-shell surface. The existing vendored backend
  associates layer parents with `layer.get_popup` before the initial commit.
  The child has no second layer-shell role. Layer-parent keyboard-interactivity
  settings are preserved.
- **X11:** the vendored native override-redirect popup, with transient-parent
  hints, parent-relative placement, output constraints and native pointer-grab
  dismissal. Existing X11 support is used directly.
- **Other backends:** this Rutter API currently returns an observable unsupported
  popup error. There is no top-level or clipped-overlay fallback.

Only the runtime resolves raw parent handles. The parent must already have a
registered, live native window. Missing, closed, suspended, self, and popup
parents are rejected. Startup requests are opened in the supplied order; put
parents first. Nested arbitrary-content popups are currently unsupported.

Popup geometry conflicts with `with_panel`, `with_layer_shell`, desktop
`with_position`, ordinary inner/min/max sizes, non-normal stacking levels,
initially hidden state and `ExitApplication` close behavior. Builder order does
not bypass validation. Popup native decorations and interactive resizing are
disabled; use popup constraint flags for compositor resizing.

## Geometry and budgets

Anchors are positive integral logical rectangles entirely inside the parent's
**configured content viewport**. Their coordinates are not desktop positions.
Above/left placement and signed offsets can put the popup origin outside that
viewport, including negative coordinates; the popup's content extent is not
limited by the thin parent's size.

| Parent panel edge | Direction | Gap offset |
| --- | --- | --- |
| Top | `Below` | `(0, +gap)` |
| Bottom | `Above` | `(0, -gap)` |
| Left | `Right` | `(+gap, 0)` |
| Right | `Left` | `(-gap, 0)` |

Placement is centered on the selected anchor edge. `PopupConstraints` allows
slide, flip and resize independently per axis. The runtime passes **exactly**
the requested flags; the default is empty. When a request cannot fit, the
compositor/backend decides the configured placement under that policy. Rutter
does not silently add resize/flip or promise that an impossible request fits.
Choose slide flags for corner anchors when flipping would point out of a panel.

Anchor endpoints are bounded by 1,000,000 logical pixels; offsets are bounded
by +/-1,000,000. Requested dimensions are positive, at most 4096 per axis and
4,194,304 logical pixels in area. Before creating a native window, the runtime
also checks the request at the live parent's actual scale against the physical
budget: **4096 per axis and 4,194,304 physical pixels per buffer**. Configured
buffers and later graphics resizes use the existing native-popup allocation
checks. This is a per-buffer pixel budget, not a total GPU-memory guarantee.

Logical request values remain logical through Winit, including at fractional
scales. Rendering, hit testing and accessibility use the popup's configured
buffer and actual scale, not its parent's viewport or the requested dimensions.
`surface_metrics_changed` publishes those actual metrics. Constraint resizing
can therefore produce a different content viewport. Design content to adapt
or scroll when allowing resize constraints.

Wayland logical metrics report the unrounded `xdg_popup.configure` dimensions.
Physical buffers are rounded at the actual popup scale. Rendering and hit testing
share that drawable/scale coordinate space; applications should use configured
metrics rather than assuming the request was granted unchanged.

Parent size, scale, movement, output/metrics changes and hiding close the child.
The application receives `surface_closed` and may **close/recreate** with a fresh
anchor after the parent configure. There is no public reposition command and
no dependency on newer `xdg_popup.reposition` protocol versions.

## Input and dismissal

By default a popup requires a native input grab (`PopupGrab::Required` in Winit).
On Wayland, return the `Open` command **synchronously from the invoking parent's
physical pointer-button or non-repeat keyboard press**. Ordinary Rutter buttons
dispatch their `on_press` message on the pointer press, so this works without raw
handles or serials. A one-use token carries the exact parent, seat and press serial
through application dispatch and synchronous popup construction. The backend
rejects missing, released, consumed, wrong-parent or unavailable-seat authorization
before allocating the popup role. It never fabricates a serial, chooses another
seat by comparing serial numbers, or silently downgrades to a non-grabbing popup.

Release-triggered, scheduled/deferred, synthetic, touch and tablet opens cannot
authorize this required-grab path. A press whose release was already dispatched
by Wayland before the application callback is also rejected. The compositor has
the final say and may dismiss a grab immediately. These failures use the normal
surface-scoped error path, not a fake successful presentation.

For a deliberately programmatic popup, opt into `popup.with_grab(false)`. This
selects `PopupGrab::None` and `active = false`. On Wayland this needs no input
serial, but **outside-client dismissal and keyboard focus are not guaranteed**.
The X11 backend retains its existing native pointer-grab policy independently of
the Wayland serial policy.

Pointer input delivered to the popup uses its own ordinary engine route. The
runtime forwards keyboard, modifier and IME events received on its invoking
parent to that popup. Other panel parents keep their own input routes. A grabbed
Wayland popup can receive keyboard focus without making the layer parent
keyboard-exclusive, but a grab does not override the compositor's keyboard-focus
policy. A layer parent configured with no keyboard interactivity may leave even
a grabbed popup without keyboard events (observed on labwc 0.7.1). Select
`PanelFocus::Normal` (Wayland on-demand), or explicit layer on-demand interactivity,
when Escape is required; the runtime never silently changes the parent's policy.
Text-input/IME support remains backend-dependent.

The runtime closes the popup on:

- native `CloseRequested` (including compositor/backend dismissal) or destruction;
- Escape received on the popup or its invoking parent, not an unrelated panel;
- a pointer press delivered to another application-owned surface, outside the
  invoking anchor (anchor clicks remain available to a toggle control);
- explicit `Close`, hiding the popup, parent closure/configure/hiding,
  replacement, suspension, or shutdown.

**Outside clicks in other clients are not guaranteed to dismiss a non-grabbing
Wayland popup.** Those events may never reach this application, and a compositor
need not send `popup_done` for a non-grabbing popup. Required grabs use native
compositor dismissal instead. Keep an explicit close/toggle control when opting
out; a non-grabbing popup on a layer panel with no keyboard input cannot rely on
Escape either.

## Lifetime, menus, and errors

There is one native transient per multi-window application. Opening an arbitrary
popup closes the previous one and dismisses native context/dropdown menus on
all parents. Opening a context/dropdown menu on another ordinary/panel surface
closes the arbitrary popup first. Nested context/dropdown menus inside an
arbitrary popup are dismissed by this policy. Simultaneous menu requests are
resolved deterministically, preferring a newly requested menu over an existing
native menu, then registry order.

Opening a popup using the already-open popup's logical ID also closes/recreates
it, including when changing parents; it never merely swaps content under an
unrelated native parent. Ordinary duplicate window IDs remain errors.

Children release graphics/native resources and retire routes before their
parent. `surface_closed` updates canonical application state exactly once for
each removed surface. A dismissed popup ID can be reused. On suspension,
transients are discarded and notified; ordinary parents retain their usual
resume behavior. Popups are not automatically restored before or after resume.

Validation/live-parent failures return `MultiWindowRunError::Popup` with popup
and parent identities. Native creation, allocation and presentation failures
follow the existing surface-scoped `MultiWindowRunError::Surface` path.
`try_run*` returns these errors; `run*` reports them to stderr. The runtime's
existing policy is to exit on a failed surface operation. No successful
native presentation is reported for a fallback top-level or overlay.
Creation inherits the vendored Wayland backend's synchronous initial-configure
wait; this runtime extension does not add a timeout around that wait.

### Compatibility

Native popup presentation is opt-in. Existing `SurfaceRequest` fields and
`SurfaceCommand` variants are unchanged; generic in-surface popovers remain
in-surface. `MultiWindowRunError` gains a `Popup` variant, so downstream exhaustive
matches over that error enum need a corresponding arm. The Winit grab default
remains `Auto` for existing callers; only the new Rutter popup API selects
`Required` by default. This API ships with Rutter 0.42.0 and adds no dependencies.

## Wayland DropdownMenu ghost/timeout repair — new report 63.8

This report concerns built-in `DropdownMenu`/`ContextMenu` native surfaces, which
share the parent runner's model. The number 63.8 was also used for an earlier
ScrollView request; this is the **Wayland popup** report.

The reported symptom was an `xdg_popup` menu appearing at the wrong position,
then moving, followed by native menus being disabled with:

```text
rutter: native menu unavailable; overlay_usable=true: popup configure/presentation timed out
```

Two client-side defects were identified:

- A successful frame left its old deadline attached to the popup. Later parent
  invalidation revoked readiness without renewing that deadline, so an otherwise
  healthy menu open for over two seconds could fail immediately.
- Parent reconciliation refreshed desired panels/origin before requesting their
  placement. Popup redraw or input could therefore present the new content using
  the previous configured rectangle before `about_to_wait` sent the request.

The runner now tracks an optional pending deadline, retiring it after a successful
frame with matching configured geometry. Content invalidation starts a budget
only when no transaction is pending. Configure waiting and presentation waiting
are distinct states, reported as `popup configure timed out` and
`popup presentation timed out`. Polling, unchanged requests and repeated model
invalidation do not extend a pending budget. An actual new placement request
starts a fresh two-second budget after the preceding request is acknowledged.
Geometry-read errors take the observable failure path with operation context.

Every popup frame/input path reconciles placement first. Only one asynchronous
request is in flight; newer desired placements coalesce until its configure,
then the latest placement is requested before any intermediate frame or hit layout
is authorized. Synchronous responses are reread too. Configured position, size
and scale remain authoritative, including compositor adjustments and negative
origins. A geometry change during presentation requires another matching frame.
The existing configure-serial-change/serial-less geometry contract is retained;
this does not add protocol token correlation to the vendored backend.

The parent overlay remains available until the first matching native presentation
succeeds. That live native surface then retains **paint ownership** across later
pending updates, independently of current-frame readiness and resolved layout.
Otherwise, the parent repaints its viewport-constrained fallback while the native
surface still displays its previous buffer. Submenus extending outside the parent
make the different placements particularly visible. Pending native paint ownership
does not authorize stale pointer or accessibility geometry.

Failure, timeout, retirement or replacement releases that surface's ownership.
Timeout/failure keeps a usable overlay and reports once for that failed native
transaction; an unusable overlay is dismissed, and later unusable opening attempts
remain observable. Hiding the fallback at initial allocation would remove visible
content before any usable native frame exists. A brief overlap at the **initial**
handoff remains possible because compositor presentation is asynchronous; window
visibility flags are not a Wayland mapping handshake.

Deterministic regressions cover invalidation after completed presentation,
non-renewing pending budgets, timeout phases, callback counts, unusable-overlay
reopen, placement before redraw/input, coalesced requests, adjusted geometry and
configure-during-frame. Additional ownership regressions cover initial pending,
later pending without resolved layout, stale input rejection and release on
failure/timeout/retirement/replacement. A parent raster regression verifies that
an out-of-parent submenu does not reappear clamped in the parent during established
native ownership, while unowned fallback still paints: 320/800px widths, a short
viewport, both themes, LTR/RTL and 1x/1.5x/2x. Placement/input coverage also includes
1x/1.5x/2x. The ignored
`native_wayland63_8_dropdown_revalidates_after_old_timeout_then_selects_submenu`
test holds a real native dropdown open for 2.1 seconds, invalidates its parent,
then expands/selects its submenu. Run display-dependent tests **one per Cargo
process**, sequentially, on an owned disposable compositor/display. For example,
with its private `XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY` and clipboard `DISPLAY`:

```bash
CARGO_BUILD_JOBS=2 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --offline --lib --all-features native_wayland63_8_dropdown_revalidates_after_old_timeout_then_selects_submenu -- --ignored --test-threads=1
```

This passed on headless Weston/pixman with software OpenGL and an isolated Xvfb
clipboard display, alongside the existing Wayland placement, CPU submenu fallback,
first-frame-failure and input-before-redraw probes. X11 dropdown, submenu and
outside-dismissal probes passed on isolated Xvfb. Native tests use synthetic runner
events on Wayland and XTEST on X11; they do not establish compositor screenshot
pixels or manual visual acceptance. The full software suite retains existing
narrow/desktop, theme, clipping, keyboard and menu-raster coverage; this repair
does not add a complete native theme/RTL/localization/responsive-state matrix.
No guarantee that all compositor-visible ghosts are gone is made without that
pixel evidence. Public APIs, dependencies, versions and fork sources are unchanged.

## Clock/calendar example and validation boundary

Run `cargo run --example native_popup_clock -- top` (or `bottom`, `left`,
`right`) in a disposable graphical session. Click the clock icon on the thin
32-logical-pixel panel. The child shows a UTC `HH:MM:SS` clock and an immediately
visible, interactive Portuguese calendar, plus a Close button. The clock uses
the ordinary second-boundary runner scheduling. No system time or settings are
modified. The example explicitly selects on-demand, not exclusive, panel keyboard
input for Escape. Actual popup presence is tracked through lifecycle callbacks.

Deterministic tests cover request validation, all four direction mappings,
exact flags, 1x/1.25x/2x logical conversion, physical budgets, parent containment,
live-parent failure boundaries, configured metrics, source-aware messages,
replacement, parent configure, child-first release and suspend/reopen. Runtime
tests reuse in-memory windows and raster fixtures; engine construction requires
the same isolated clipboard service as the existing runner tests.

The owned-display probe below passed on labwc 0.7.1 at all four panel edges and
1x/1.25x/2x output scales (12 scenarios, three opens each). It runs the real
example, checks protocol role/grab and layer-parent association, reads configured
bounds after slide constraints, and samples the nested compositor's actual pixels
for ticking, month navigation and day selection. XTEST is sent only to its owned
Xvfb host; labwc delivers genuine Wayland seat input. It checks Escape,
outside-client dismissal, explicit Close and reopening. The example's `(0, 0)`
anchor exercises a panel corner; this is not every possible corner anchor.

```bash
CARGO_BUILD_JOBS=2 cargo build --locked --offline --example native_popup_clock
python3 tests/native/native_popup_smoke.py \
  --labwc /absolute/path/to/labwc \
  --xvfb /absolute/path/to/Xvfb \
  --wlr-randr /absolute/path/to/wlr-randr
```

The script creates private displays/configuration without inherited session DBus
or desktop access, runs scenarios sequentially, and retains logs on failure
(`--keep` retains successful logs too). Build before running the probe, never
concurrently. No new Python packages are required.

Remaining live coverage includes ordinary toplevel and X11 parents for this new
API, non-grabbing behavior, every corner/constraint policy, multiple parents,
menu arbitration, parent closure, suspend/shutdown and FRAYM. Deterministic
coverage is not a substitute for those compositor scenarios. Non-grabbing
outside-client dismissal is not guaranteed by that opt-out contract.
