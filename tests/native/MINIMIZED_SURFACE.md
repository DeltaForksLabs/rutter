# Minimized Settings / shared OpenGL native regression

## Presentation policy

The OpenGL backend selects `SwapInterval::DontWait` from the actual Wayland
window handle and calls `pre_present_notify()` immediately before buffer swap.
Winit then paces redraw delivery using per-surface Wayland frame callbacks;
an invisible surface's withheld callback does not block the shared UI thread.
Glutin 0.32.3 documents that `SwapInterval::Wait` can block hidden Wayland windows
until visible again. Failure to establish interval zero is an explicit OpenGL
initialization error, not a silently accepted blocking configuration. Other
window systems retain their existing best-effort wait-one swap policy.

Interval zero is not a guarantee against driver overrides, GPU/resource stalls,
or blocking in other graphics operations. No focus-loss/minimized-state guess,
callback timeout, model-update suppression or popup authorization relaxation is
used. CPU and Vulkan presentation policies are unchanged.

## Native driver

`minimized_surface_smoke.py` starts an authenticated private Xvfb server and a
private nested FRAYM compositor, then runs one **already compiled** ignored Rust
library test. It never invokes Cargo or connects to an inherited desktop. Its
independent supervisor imposes a 150-second deadline and kills the entire owned
process group, including a worker blocked in Xlib or a fixture blocked in EGL.
The fixture also has a 90-second cooperative deadline, and command completions
have an eight-second deadline. Failed runs retain logs and compositor-state JSON
under `/tmp/opencode/rutter-minimized-*`; `--keep` retains successful runs too.

## Run sequentially

From the repository root, build once and choose the executable printed by Cargo
(the `rutter-<hash>` **library test**, not a `.d` file or example). Set
`RUTTER_TEST_BINARY` to that executable's absolute path before the native command.
Do not run this build concurrently with other compilation or validation:

```sh
CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --offline --lib --no-run
python3 -m unittest discover -s tests/native -p 'test_minimized_surface_smoke.py' -v
python3 tests/native/minimized_surface_smoke.py \
  --fraym /home/p_daniel/Labs/Isolated_Environment/rustLang@Projects/fraym_compositor/target/debug/fraym \
  --xvfb /tmp/opencode/graphics-tools/usr/bin/Xvfb \
  --test-binary "$RUTTER_TEST_BINARY" \
  --keep
```

Use Cargo's actual executable path (including any custom target directory). If
the locally extracted graphics tools require their shared
libraries, prefix the native command with
`LD_LIBRARY_PATH=/tmp/opencode/graphics-tools/usr/lib/x86_64-linux-gnu`.
The driver accepts other absolute or relative executable paths. `--timeout N`
sets the external deadline (1–300 seconds); it does not remove per-stage bounds.
The standalone driver needs no build tools after the test binary exists.

## Required runtime dependencies

- Linux, Python 3 standard library, `libX11.so.6`, `libXtst.so.6`, and `xauth`.
- Xvfb with XTEST, and working software OpenGL/EGL (normally Mesa llvmpipe).
- FRAYM with `--nested`, layer-shell, and version-1 JSON IPC requests `state`,
  `minimize-window {"id": N}`, and `restore-window {"id": N}`. `state` must report
  stable window IDs, title, `minimized`, geometry, and output scale/geometry.
- The existing `layer_shell_smoke.py` and `native_popup_smoke.py` modules next to
  the driver; their XTest and XGetImage support is reused without PIL.
- An existing writable `/tmp/opencode` directory.

No Rust or Python package dependencies are added. Labwc is deliberately not a
fallback: keyboard shortcuts alone would not establish actual minimized state.

## What a passing run proves

One `MultiWindowRunner` owns an ordinary Settings xdg-toplevel and an independent
64px top layer panel. OpenGL is required before creation, without backend probing
or fallback. Protocol assertions require exactly those two native roles and a
submitted panel buffer. The driver uses three minimize/restore cycles and checks:

1. Native XTest keyboard input edits the Settings draft before minimizing.
2. FRAYM confirms that the exact Settings window ID is minimized.
3. Three monotonically increasing model publications per cycle redraw Settings
   **first**, then the visible panel. The fixture explicitly routes redraw events
   through the real runner because compositor-scheduled frame callbacks may stop
   while minimized. Both windows must retain positive surface sizes. The
   `MINIMIZED_DONE` marker is emitted only after both redraw calls return without
   a fatal error: with the current nonzero-size engine path, this is after
   `GraphicsBackend::end_frame`, including the real EGL swap.
4. After every publication, native pointer input activates the visible panel.
   A native-input marker, the canonical click count, Wayland button events, and
   changed background pixels away from text/cursor establish actual interaction
   and presentation. Settings' minimized bit is checked before/after this work.
5. Restore preserves the same window ID, latest model and draft. The heading's
   pixels change, and subsequent native editing extends the existing draft,
   including after the final restore. Each command checks the expected model,
   click count and exact controlled draft, and redraws again before acknowledging.

The panel's blue/green background is tied only to activation, so periodic model
updates cannot satisfy its changed-pixel assertion. Input completion is awaited
before sending a state-check command, avoiding ordering assumptions between the
X server and the command-file polling. Command files are atomically replaced.

## Coverage limits and UI review

This is Linux software-EGL native acceptance only when actually run; neither the
driver contract unit tests nor code inspection establishes native acceptance.
It exercises real minimize/restore through compositor IPC, rather than a
titlebar minimize click or `xdg_toplevel.set_minimized`. It does not measure
physical display presentation or timing and is not cross-platform acceptance.
Forced redraw delivery intentionally tests bounded swap completion even if a
compositor would otherwise suppress minimized-window redraw scheduling.

The fixture uses one 24px page inset, 16px sibling gap, a 72px field allocation
including its label, and a full-width 64px panel target. These are intentionally
small fixed test interfaces, with no popup or scrolling requirement. The native
driver uses one 1600×1200, 1x output and the default theme. Narrow/short layouts,
both themes, localized/error/disabled states, fractional/2x scales, accessibility
and manual visual review are not covered by this regression. The fixture does
not run Quasar itself, an unrelated terminal control, delayed popup grabs or
CPU/Vulkan minimization tests. See `decisions/DEVLOG.md` for commands actually
run and the negative control using the previous blocking swap policy.
