# Dynamic Island

> **Current scope:** The example slides summary text inside the capsule; the
> capsule itself only expands/contracts. Interactive mode expands on hover and
> uses text entry only, never a slide-out. Text offsets follow the actual laid-out
> content box; interrupted transitions retain their sampled displacement and
> passive accessibility follows the current summary. The reusable `rutter::animation`
> primitives require the host to apply sampled values and schedule updates.

`DynamicIsland` is a composed widget, exported at the crate root and through
`rutter::dynamic_island`. It uses existing Container, Column, RichText and
ButtonContent widgets, not a new `Widget` enum variant or custom leaf. Widget
state, native hosting and future Quasar content/IPC remain separate concerns.

## Fixed mode, controlled expansion

- `DynamicIsland::passive()` creates a display-only, always-collapsed instance.
  Its view accepts summary text, has no button or tab stop, and may be rebuilt
  when application-owned content changes. Expansion operations return an error.
- `DynamicIsland::interactive()` creates a collapsed interactive instance.
  `set_expanded(bool)` and `toggle()` update application-owned state; the header
  emits the message supplied to `interactive_view`, and details can contain real
  controls. Details are absent from layout, hits and accessibility when collapsed.
- `mode()` has no setter. Call the matching `passive_view` or `interactive_view`;
  mismatches return `DynamicIslandError`. Replacing the entire object creates a
  different instance; this is not a global identity registry.

```rust
use rutter::{DynamicIsland, Theme};

let island = DynamicIsland::passive();
let view = island.passive_view::<()>("Sync complete", &Theme::dark())?;
# Ok::<(), rutter::DynamicIslandError>(())
```

Views wrap text with natural height, a bounded 800px maximum width, 16px internal
padding and 12px header/details separation using `rutter::spacing`. The 24px
radius is a shape dimension, not spacing. Keep a stable details structure to
retain automatic child IDs when reopening. Existing controls provide keyboard
actions and accessibility; no live screen-reader acceptance is claimed.

The core expansion setters remain immediate semantic updates. Hosts can opt into
the separate `DynamicIslandMotion` geometry controller; the native example does
so by default. Quasar IPC/content sources and automatic publication of
layout-derived native input are not included.

## Widget gallery example

```sh
CARGO_BUILD_JOBS=2 cargo run --locked --offline -- dynamic_island
```

This ordinary-window example shows both modes together, without requiring a
Wayland layer-shell surface. The passive capsule is above the interactive one.
It cycles through local hours (`HH:MM`), date and weekday as three separate
summaries every three seconds, sliding the outgoing text upward before the next
summary enters from below (240ms per phase). The clock refreshes at minute
boundaries; month and weekday names currently use English formatting. Its
accessibility label exposes the complete current clock snapshot independently
of the outgoing visual text, and the passive capsule has no tab stop or action.

The interactive playback capsule grows from **240×76px to 404×252px** over
240ms: width and height animate together, with its horizontal center retained.
Widths are constrained to the available page content at narrow window sizes.
Hover or activate the header to expand; Pause/Resume, Collapse details and Escape
exercise real interactions. Details remain scrollable during expansion, and
the page scrolls in short windows. The theme selector supports both themes.
Only active transitions schedule frame wakeups; the idle clock uses deadlines
for the next content change rather than continuous frame polling.

## Hover behavior in the native example

A fresh pointer entry into the interactive capsule expands it without a click.
Repeated movement does not restart expansion; moving between header and details
stays inside the same capsule. Transparent padding and rounded corners do not
count as entry. Leaving only clears the entry latch; it does **not** collapse.
Escape collapses when this surface owns keyboard focus. After Escape, remaining
inside does not expand again until the pointer leaves and reenters. Header
activation by pointer, keyboard or accessibility also requests idempotent expansion
rather than toggling collapse. Passive mode never expands or observes input.

This policy belongs to the example, not `DynamicIsland::interactive_view` itself:
the composed widget still emits the caller-supplied header message.

`MultiWindowAppLogic::surface_pointer_hovered` is an optional native mouse
entry/movement/leave observer, using surface-local logical positions (`None` for
leave). It does not consume child input or request focus, and excludes native
menu windows. Its default return value `None` means no mutation or commands and
avoids model publication and layout-cache invalidation. Return `Some(Vec::new())`
for a state change or `Some(commands)` for a state change with runtime actions;
visual changes also require a redraw request. The example publishes membership
changes and queues expansion through its normal wakeup lifecycle.

## Expansion and text motion

`DynamicIslandMotion` and `DynamicIslandSlide` are exported alongside the widget.
The controller owns logical geometry, not a renderer, widget tree or clock:

- `new(host, visible_bounds)` validates positive finite geometry, containment and
  a 16384px limit on host axes/absolute input edges.
- `transition_to_at(bounds, duration, now)` animates size/position to a new visible
  target. `slide_at(direction, duration, now)` remains available for hosts that
  intentionally want geometry entry/exit; the example does not use it for text.
- `sample_at(now)` advances using elapsed time and smoothstep easing; store the
  returned `bounds()` for **both** layout and native input. Never sample again
  independently in `view()`.
- `is_animating()` tells a host whether it needs another frame. Zero duration
  completes immediately. Late wakeups skip missed frames; interruptions start
  at sampled geometry without preserving velocity.
- `reconcile_at(host, visible_bounds, now)` retargets changed metrics using the
  remaining duration. Sampled dimensions are preserved up to the new host size,
  rather than proportionally shrinking controls below their padding. Position
  maps between fully off-host edges; idle/hidden state remains idle/hidden.

Directions are physical, independent of RTL:

| Transition | Direction |
| --- | --- |
| `SlideInLeft` | Enter from the left |
| `SlideInRight` | Enter from the right |
| `SlideInUp` | Enter from below, moving upward |
| `SlideInDown` | Enter from above, moving downward |
| `SlideOutLeft` | Exit toward the left |
| `SlideOutRight` | Exit toward the right |
| `SlideOutUp` | Exit above |
| `SlideOutDown` | Exit below |

The example animates capsule expansion/retraction over **240ms** and summary text
over **280ms**, with absolute 16ms wake deadlines only while moving. Text uses
`RichText::with_bounds_relative_visual_offset`, which moves painting by fractions
of the actual content box inside its original clip, without changing wrapping,
layout, hits or accessibility geometry. A full-box offset hides wrapped text;
the existing pixel-based `with_visual_offset` remains available and additive. It does
not move the capsule or its native input region. The default text entry is
`SlideInDown`; the passive default exit is `SlideOutUp`.

Interactive summary changes replace semantic and visual content immediately and
start only the selected entry animation. Close publishes Empty input and closes
the native surface immediately, including during entry; there is no pending exit.
Collapse removes details immediately at transition start while the capsule
shrinks, so disappearing controls do not retain focus/actions. Expansion reveals
the ordinary scrollable document; text is not scaled or stretched. Passive
summary replacement retains the outgoing-text exit followed by the new entry.
During interruption, both displacement axes are sampled at event time and
retargeted without resetting position (velocity is not preserved). Passive updates
coalesce into the latest incoming summary without restarting the outgoing phase.
Late wakeups advance both phases from the actual exit boundary. The passive
RichText uses `with_accessibility_label` to expose the current summary immediately,
independently of the outgoing visual payload; without an override, RichText keeps
its existing concatenated-span label.

The general `rutter::animation::{Animation, Slide}` API supplies explicitly timed
scalar interpolation and directional offsets. It does not automatically transform
widgets, provide opacity or schedule frames. Hosts must apply sampled values and
keep paint, layout, hits and accessibility consistent.

```sh
WAYLAND_DISPLAY=wayland-1 cargo run --locked --offline --example dynamic_island -- interactive --enter SlideInLeft
WAYLAND_DISPLAY=wayland-1 cargo run --locked --offline --example dynamic_island -- passive --enter SlideInUp --exit SlideOutRight
WAYLAND_DISPLAY=wayland-1 cargo run --locked --offline --example dynamic_island -- interactive --no-animation
```

An optional output must precede flags; duplicate/unknown/misplaced flags and
entry/exit direction mismatches are errors. `--exit` remains accepted and validated
in interactive mode for CLI compatibility, but is ignored there.
`--no-animation` disables all motion
without changing controls or mode. No automatic OS reduced-motion preference
detection is provided; embedding applications should use zero duration when their
own policy requests reduced motion. Example text slides use the text content clip,
not physical monitor edges. The layer role stays fixed. Hosts using geometry
slides must build rounded input from the original translated capsule, not from a
clipped rectangle with newly invented rounded corners.

## Transparent native host

```sh
CARGO_BUILD_JOBS=2 cargo run --locked --offline --example dynamic_island -- interactive
CARGO_BUILD_JOBS=2 cargo run --locked --offline --example dynamic_island -- passive DP-2
```

The optional output name is exact and case-sensitive on this Wayland connection;
omit it to let the compositor choose. A nested FRAYM output may not have the same
name as a physical connector such as `DP-2` in the outer desktop. `WAYLAND_DISPLAY`
and `XDG_RUNTIME_DIR` select the connection; `FRAYM_SOCKET` is not used for output
discovery by this example. The example requests a
top-centered, transparent Wayland Overlay layer, 420×300 logical pixels, no work
area reservation, with no silent ordinary-window fallback. Role geometry is fixed
for that native surface. The capsule expands **inside** the reserved transparent
host: 64px passive height, **240×76px compact interactive** and **404×252px
expanded**, bounded by actual surface metrics. Both interactive dimensions
animate together around the same horizontal center. An 8px host inset is
intentionally compact. Interactive content
scrolls when taller than its allocated body; Pause/Resume and Close are real
actions. Escape collapses when keyboard focus belongs to this surface. Passive
content updates every three seconds through application wakeups, not continuous
polling. The example's short passive summaries are not a general long-document
presentation policy; hosts supplying arbitrary text must allocate its measured
height or choose an accessible compact presentation.

Transparency needs an alpha-capable backend; RGB-only CPU presentation cannot
provide it. Alpha is not blur and does not make a surface click-through.

### Input policy is independent

`WindowConfig::with_input_region(region)` specifies the initial native policy;
`SurfaceCommand::SetInputRegion { surface, region }` changes it. The default is
`SurfaceInputRegion::Full`. `Empty` passes pointer/touch input through even painted
pixels. `Rectangles` is a union of up to 256 validated rectangles, built with
`SurfaceInputRect::new` and `SurfaceInputRegion::rectangles`. Units are integer
surface-local **logical** pixels, not buffer pixels. Negative origins are allowed;
nonpositive extents and overflowing signed endpoints are rejected.

Only Wayland implements the opt-in setter currently; unsupported native requests
return contextual errors instead of silently accepting a rectangular fallback.
Initial policy is applied before the first buffer presentation, including resume.
Successful updates persist in the surface configuration; suspended updates defer
until recreation. These operations do not alter keyboard policy, grabs, opacity
or shell roles. Adding `SetInputRegion` requires updating downstream exhaustive
`SurfaceCommand` matches; no serialized-data migration is needed.

`island.input_region(actual_bounds, radius)` returns `Empty` for passive mode and
a rounded rectangle union for interactive mode. Hosts must supply the **painted
capsule bounds**, then publish the returned policy on geometry/state changes.
The helper cannot infer layout or track ancestor offsets. Rounded corners use
merged one-logical-pixel bands (up to 1px optical approximation); invalid geometry
and effective radius above 127px are rejected to bound work and region count.
The example shares bounds between layout and input, starts with Empty and
reconciles after `SurfaceMetrics` using a wakeup. Padding and corners therefore
remain click-through while the interactive capsule receives input.

Passive hosting always requests keyboard `None`. Interactive hosting starts with
`OnDemand` (layer-shell v4). With explicit user approval, this example now requests
temporary **`Exclusive` while expanded**, which may take keyboard focus from other
applications. Collapse/Escape restores `OnDemand`. There are no synthetic keyboard
events or global key grabs.

Each expansion makes one request and, unless already focused, schedules one 750ms
focus-grant deadline. Native `SurfaceEvent::FocusChanged(true)` cancels that deadline;
there is no idle polling or repeated focus acquisition. If no grant arrives by the
deadline, the explicit application callback `IslandDemo::focus_by_click_fallback`
restores `OnDemand` while keeping details expanded, allowing normal click focus.
**A timeout is an application inference, not proven compositor refusal:** layer-shell
has no explicit denial response. Focus loss while expanded invokes the same callback
with a distinct reason and does not re-grab. A later native click-focus grant enables
Escape normally. Collapse and both application/native close cancel the deadline.
Click focus remains subject to compositor policy; it is not guaranteed.

`SurfaceCommand::SetLayerKeyboardInteractivity { surface, interactivity }` uses
the fallible native `Window::set_layer_keyboard_interactivity` setter. It validates
the live layer role and negotiated v4 requirement for `OnDemand`, queues state for
the next presentation commit, and requests a redraw. Successful configuration
persists across suspension/resume; suspended requests defer to recreation. Failures
retain the previous configuration and follow the runtime's existing error policy
with a contextual `MultiWindowRunError::LayerKeyboardInteractivity` and cause.
Unsupported backends and ordinary/xdg surfaces fail explicitly. Success reports
queuing, not focus acquisition. The runtime does not infer compositor denial.
Downstream exhaustive `SurfaceCommand` and `MultiWindowRunError` matches must handle
the new variants; configuration defaults and serialized data are unchanged.

## Validation and remaining limitations

Deterministic widget/example tests cover 320/800px widths, a short viewport, both
themes, long/localized text, LTR/RTL, focus/disabled child behavior, stable IDs,
collapsed accessibility, scrolling/hits and 1x/1.5x/2x raster containment. There are
no island-specific field-validation messages or popup states to validate.

Build and run the native driver **sequentially**, on its owned displays only:

```sh
CARGO_BUILD_JOBS=2 cargo build --locked --offline --example dynamic_island_probe
python3 tests/native/dynamic_island_smoke.py --labwc /absolute/path/to/labwc --xvfb /absolute/path/to/Xvfb --keep
# Alternative: existing current FRAYM binary, private IPC and config:
python3 tests/native/dynamic_island_smoke.py --fraym /absolute/path/to/fraym --xvfb /absolute/path/to/Xvfb --keep
```

The driver uses private Xvfb plus a nested compositor and real XTEST input, clears
inherited session/display addresses, and records protocol logs, PPM screenshots
and pixel measurements. It requires the existing native harness libraries, no new
Python packages. Its native pixel coverage is 1100×800 at 1x, not the software
raster matrix or a manual visual review.

The labwc run established real GL alpha composition, full passive click-through,
interactive corner/padding exclusion, header/action clicks and input changes on
expand/collapse/reopen. Initial keyboard delivery matched None/OnDemand. However,
after click-through moved focus to the underlying window, subsequent island
clicks expanded it without a new native `wl_keyboard.enter`; Escape still reached
the underlying window. **The historical native driver fails its keyboard
reacquisition assertion.** It intentionally retains that assertion. Protocol
evidence locates the gap at native/compositor focus, but does not prove an internal
labwc cause. This predates the explicitly approved temporary Exclusive policy above;
the unchanged probe does not exercise that new policy or establish its acceptance.

**User-reported FRAYM acceptance:** the user confirmed the pre-animation example
works with an updated nested FRAYM. Their log shows `initialized opengl backend`
despite Haswell Vulkan/Zink and EGL driver warnings. This supersedes the earlier
untested-current-build status for basic hosting, but is not an agent-executed
native smoke, a focus-reacquisition result, or acceptance of these new animations.
Earlier layer-shell findings are historical, not evidence about that update.

For request64.7 the user additionally confirmed that request64.6's temporary
expanded keyboard capture works in FRAYM, while reporting expansion/retraction
stutter. This is user acceptance of the focus behavior, not measured frame pacing
or acceptance of the subsequent performance changes.

Animation tests additionally cover all eight geometry endpoints/midpoints,
geometry interruption, late sampling, zero duration, idle deadlines, metrics
changes during motion, short-host layout/input agreement and close safety.
Hover regressions cover fresh entry, repeated motion, header-to-details movement,
Escape/reentry, rounded exclusions and passive isolation. Host tests cover default
no-op cache preservation, logical coordinate conversion and surface routing; a
private Xvfb test covers real mouse/leave routing and native-menu isolation.
The final all-target/all-feature suite passed **1,279 tests, 38 ignored**;
**315 doctests**, all-target/all-feature Clippy, formatting and whitespace checks
passed. Ignored native opt-in tests are not evidence of compositor acceptance.

Additional regressions cover fully hidden wrapped summaries taller than the old
44px offset at all eight endpoints, both themes, 320/800 widths and 1x/1.5x/2x
scales; interrupted and perpendicular transitions; passive coalescing and current
accessibility labels; owned metadata conversion; and wakeups spanning both phases.
Offsets are relative to live text allocation: reflow or resize may move pixels
even when normalized displacement is continuous. Text replacement intentionally
changes glyphs immediately in interactive mode; no crossfade or velocity continuity
is promised.

The native probe above still uses immediate transitions and its earlier click
policy; it does **not** establish the new hover behavior, animated frame pacing or
real compositor input during motion. No new owned-compositor/manual visual run
was performed for this stage. Optimized native animation pacing, non-Linux runtime
and live assistive technology remain pending, in addition to the retained labwc
focus-reacquisition failure.

For request64.6's approved temporary Exclusive policy, focused validation covers
25 example tests (including the existing geometry/raster matrix), eight command
tests, native queue-before-redraw/version/retirement validation, and an isolated
X11 test verifying the default unsupported setter as well as native hover routing.
Application tests cover focus grant, bounded no-grant fallback, later click-focus
notification/Escape, focus loss without re-grabbing, and close cancellation.
These deterministic focus notifications are not live Wayland focus acceptance.
No new owned Wayland compositor or manual visual run was performed for this policy;
the historical probe and its focus-reacquisition failure remain unchanged.

## Animation performance work (64.7)

The application scheduler retains a next-frame deadline that becomes overdue
while the same event cycle is rendering. It still executes at most one callback
per cycle and suppresses stale deadlines to avoid an idle busy loop. The example
reuses rounded input bands for unchanged bounds and submits native input only
when the effective region changes; text-only frames still redraw. Metrics and
native lifetime changes force input republication, and close clears input.

RichText painting reuses laid-out paragraphs keyed by content/style, exact width,
direction and fallback color. Translation, accessibility labels and clip height
do not invalidate that paint cache. It retains at most 32 entries and 64 KiB of
accounted owned-spec storage, bypasses entries above 16 KiB, and clears with the
renderer. These limits do not measure Skia's native paragraph heap. Measurement
semantics and the 240ms capsule/280ms text durations are unchanged.

An ignored offline CPU/raster probe is available with:

```sh
CARGO_BUILD_JOBS=2 CARGO_PROFILE_TEST_DEBUG=0 cargo test --lib rich_paragraph_cpu_profile --locked --offline -- --ignored --nocapture
```

In an unoptimized test-profile before/after run of 32 batches of 120 measure/draw
frames, paragraph builds fell from 7,680 to 3,852, median batch time from 137.333ms
to 98.458ms and p95 from 142.915ms to 107.069ms. This isolates client CPU/raster
work; it is not a FRAYM, GPU-presentation, refresh-rate or release-profile result.
No wall-clock performance threshold is asserted by ordinary tests. Optimized
native visual/pacing acceptance remains pending.
