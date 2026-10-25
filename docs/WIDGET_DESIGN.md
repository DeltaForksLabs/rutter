# Widget Design Standard

## Permanent rule

For every new or modified interface, widget or example, apply the
[`widget-design` skill](../.opencode/skills/widget-design/SKILL.md) before editing
and during final review. Preserve behavior, accessibility and identity while
improving composition. This standard applies across graphics libraries; the
Rutter implementation uses Taffy layout and Skia painting.

`opencode.json` loads this rule as project instructions. The project-local skill
is discoverable after restarting OpenCode. Other development tools should load
this document explicitly. No global tool configuration is required. A skill is
an agent workflow, **not a runtime auto-layout engine or a guarantee of visual
quality without testing**.

## Spacing vocabulary

All values are logical pixels, scaled once at the display boundary. The source
of truth for Rust consumers is [`rutter::spacing`](../src/spacing.rs).

| Token | Rust constant | Value | Intended relationship |
| --- | --- | ---: | --- |
| `space-xs` | `XS` | 4 | Icon/text and tightly related details |
| `space-sm` | `SM` | 8 | Labels, supporting text and related actions |
| `space-md` | `MD` | 12 | Members of a logical group |
| `space-lg` | `LG` | 16 | Separate controls and card padding |
| `space-xl` | `XL` | 24 | Titles, blocks, subsections and page inset |
| `space-2xl` | `TWO_XL` | 32 | Independent sections |
| `space-3xl` | `THREE_XL` | 48 | Major divisions, not every page edge |

These are references, not a mandate to round all numbers. Document deviations
for dense inspectors, optical alignment or platform constraints. Control
dimensions, minimum hit targets, typography, radii and borders are not spacing.
Do not increase `Theme.spacing` globally to compensate for a single crowded view.

## Composition and hierarchy

- A page owns its outer inset; a section owns its title/content separation; a
  group owns its sibling gaps; a field owns its annotations. Prefer one owner
  per spacing relationship, avoiding duplicate margins and gaps.
- Start with 24px page inset, 24–32px section separation, 12–16px group gaps and
  8px label/helper separation. A compact surface can use 12–16px outer inset.
- Keep a heading and its description together, with more space before the next
  independent group. Use semantic headings and restrained size/weight changes.
- Align form fields, descriptions and group titles on a common start edge.
  Use baseline alignment for mixed inline text where supported; center controls
  within action rows, not entire documents. Equal peers use equal padding.
- Give wrapped action rows both horizontal and vertical gaps. Use logical
  start/end alignment and verify RTL rather than assuming left/right everywhere.
- Keep header actions out of title text. Absolute controls require an explicitly
  reserved safe area, including on narrow surfaces.

## Content, constraints and widget internals

Measure and paint the same wrapped text, with the same font metrics, direction,
line height and content width. Respect padding and clip deliberately bounded
content; do not use clipping to hide missing content that should scroll or wrap.
Prefer natural text height, minimum control height and bounded fluid width over
fixed document height or arbitrary spacer widgets.

Field labels and errors must be accounted for inside allocated layout bounds;
control-body painting, focus, selection, pointer coordinates and popup anchors
must agree on the resulting body rectangle. Fixed control-body heights must not
be silently consumed by annotations. Compact single-line labels/errors may
ellipsize when the full text remains available through accessibility metadata.
Do not remove messages or interaction to obtain non-overlapping rectangles.

Use scroll viewports for overflowing documents and retain intrinsic document
height. Reflow multi-column groups or wrap action rows before shrinking fields
below their usable width. Constrain overlays to their actual containing block,
and verify scrolling/overlay hit testing, not just the closed view.
Keep blocking overlays after document siblings in paint order (before them in
hit-test priority), and consume inert card space rather than forwarding clicks.
Reconcile popup anchors from live ancestor offsets even when layout is unchanged;
check clipped-out triggers, restoring visibility and dismissal. Tab/Shift+Tab
must reveal the focused control through its scroll ancestors, not scroll the
underlying document when focus belongs to portaled content.

## Objective validation checklist

For the changed interfaces, record actual coverage of:

- **Geometry:** no unintended intersecting siblings; child paint contained in
  allocation or a documented overlay; <=1 logical px rounding tolerance for
  aligned edges; positive usable control-body dimensions; consistent peer inset.
- **Hierarchy:** within-group gaps smaller than independent-section gaps;
  title, description, group and action roles distinguishable without color alone;
  no unexplained off-scale spacing or accumulated gap+margin.
- **Responsive states:** 320px and 800px widths, short viewport, both themes,
  long/localized strings, validation errors, focus, disabled and open overlays.
  Also sample 1x/1.5x/2x scaling when rendering behavior changes.
- **Usability:** keyboard access and pointer hit positions match painted bodies;
  overflow remains reachable by scroll; no header-action/title overlap.
- **Evidence:** geometry tests for structure and bounds, raster tests for text,
  annotations and clipping, and manual visual review when a display is available.
  Do not claim manual or non-Linux acceptance from software raster tests.

Review examples by layout family rather than blindly replacing numeric values:
basic controls, forms, overlays, collections/documents, custom controls and
multi/native-window surfaces. Preserve each example's demonstration and IDs.
Run resource-intensive validation sequentially, following `AGENTS.md`. Report
exceptions, limitations and compatibility effects in the development log.
