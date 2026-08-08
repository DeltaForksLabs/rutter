# RichText migration

Rutter 0.19 removes `Widget::StrongText` and `Widget::strong_text`. `Widget::RichText` replaces them with one non-interactive text leaf containing styled spans while the project remains in pre-1.0 beta development.

## StrongText replacement

Before:

```rust
Widget::strong_text("2026", style, Some(color), 16.0)
```

After:

```rust
let defaults = RichTextStyle::default()
    .with_size(RichTextSize::new(16.0)?)
    .with_color(RichTextColor::rgba(
        color.r(), color.g(), color.b(), color.a(),
    ));
let content = RichText::from_span(RichTextSpan::new("2026").bold())
    .with_default_style(defaults);
Widget::rich_text(content, style)
```

When no rich-text color is provided, rendering inherits `Theme::on_surface`. A span can call `with_theme_color()` to reset an explicit inherited color to that runtime fallback.

## Style mapping

| Removed argument | RichText equivalent |
| --- | --- |
| `content` | One or more `RichTextSpan` values |
| `size` | Validated `RichTextSize` in `RichTextStyle` or a span override |
| `color` | Project-owned `RichTextColor` in defaults or a span override |
| strong rendering | `RichTextSpan::bold()` or `RichTextWeight::BOLD` |

Spans additionally support italic/upright slant, underline resets, independent sizes and colors, borrowed or owned text, and exact logical concatenation for accessibility.

## Exhaustive matches

Downstream exhaustive matches must replace `Widget::StrongText` with `Widget::RichText`. The public `RutterContext` also adds a `RichText` variant because Taffy retains a lifetime-free snapshot for measurement. Plain `Text` and `RichText` remain in the same widget-identity and accessibility-ID family, so runtime transitions between them preserve structural compatibility.

Manual calls to `layout::compute_layout` now receive a reusable `&render::RichTextRenderer` argument. Retain one renderer alongside the Taffy tree instead of constructing it on every layout invalidation; `RutterEngine` manages this resource automatically.

## Ownership

`RichTextSpan` stores `Cow<'a, str>`. Use `RichText::into_owned()` when a value must outlive borrowed application state. Layout automatically snapshots transient widget spans before retaining them.
