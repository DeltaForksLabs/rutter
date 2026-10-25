---
name: widget-design
description: Use when creating, maintaining or reviewing UI widgets, examples, layouts, spacing, alignment, visual hierarchy or responsive interfaces; applies relationship-based spacing and verifies painted content as well as layout geometry.
---

# Widget Design

Apply this skill to every UI change, not just explicit design requests. It is
framework-independent; use the host project's equivalent layout primitives.
For Rutter, first read `docs/WIDGET_DESIGN.md` from the repository root and use
`rutter::spacing` plus the shared example layout helpers. Do not overwrite
unrelated work or perform a repository-wide numeric replacement.

## Design before editing

1. Identify page, section, group, control and supporting-text relationships.
2. Inspect existing render/measurement behavior and representative tests. A
   rectangle fitting does not prove that its painted label or error fits.
3. Choose a density and a left/start reference edge. State exceptions explicitly.
4. Preserve IDs, focus, callbacks, validation messages and demonstrated behavior.

## Relationship-based spacing

| Token | Logical px | Default relationship |
| --- | ---: | --- |
| `space-xs` | 4 | Icon/text, tightly related details |
| `space-sm` | 8 | Label/control, helper text, related actions |
| `space-md` | 12 | Elements of one group |
| `space-lg` | 16 | Separate controls, card padding |
| `space-xl` | 24 | Title/content, blocks, page inset |
| `space-2xl` | 32 | Independent sections |
| `space-3xl` | 48 | Major divisions, used sparingly |

Use these as adjustable references. Within-group spacing should normally be
smaller than between-group spacing. Do not force font sizes, dimensions, pointer
targets, border widths or optical corrections onto the spacing scale.

- Use parent `gap` for siblings, container padding for edges, margins only for
  genuine local exceptions. Avoid stacked gap + margins that double separation.
- Keep title/description closer than title/unrelated section. Use size/weight
  and semantic headings as well as spacing; do not make all text equally prominent.
- Align related control edges; center icon/button content, not entire forms.
- Reserve label/helper/error rows inside field allocation. Do not paint into
  neighboring components or hide meaningful validation to make a test pass.
- Provide enough width for localized text. Wrap descriptions. If compact labels
  must ellipsize, preserve full accessible text and document that choice.

## Responsive composition

- Prefer bounded fluid widths, intrinsic text heights, wrapping rows with gaps
  on both axes, and explicit scrolling for content taller than the viewport.
- Keep controls and text from shrinking to zero. Avoid absolute positioning for
  normal document flow; reserve a real safe area for overlay actions.
- Overlays need width constraints relative to the actual host, not an assumed
  viewport. Test their open state, anchors, dismissal and hit coordinates.
- Preserve usable controls before decorative whitespace. Reduce page padding
  or reorganize columns on narrow surfaces instead of compressing everything.

## Objective review and acceptance

1. Check at least a narrow (320px) and desktop (800px) logical width, a short
   viewport, both themes and representative scale factors (1x/1.5x/2x).
2. Inspect normal, focused, disabled, long-text, validation-error and open-popup
   states. Verify keyboard reachability, scrolling and hit targets after reflow.
3. Fail for overlapping painted content, unintended clipping/overflow, labels
   outside allocation, unequal peer padding or unexplained spacing off the scale.
4. Verify the chosen group/section hierarchy, aligned start edges (allow <=1px
   layout rounding), wrapping-row separation and title safe area.
5. Add layout tests for bounds/grouping and raster tests for painted-text and
   annotation regressions. Pixel checks must avoid machine-specific font shapes.
6. Run configured formatting, targeted tests, lint and relevant broader tests,
   **sequentially**. Never silently weaken a geometry assertion after a redesign:
   update its semantic expectation and preserve interaction coverage.
7. Review actual rendered screens where available. Automated geometry/raster
   checks are not manual visual, real-screen-reader or cross-platform acceptance.

Report migrated interfaces, exceptions, actual checks and remaining limitations.
The skill guides the development agent; it does not alter running UI by itself.
