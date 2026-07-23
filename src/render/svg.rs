// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

pub(crate) const MAX_SVG_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_SVG_ELEMENTS: usize = 10_000;
pub(crate) const MAX_SVG_DEPTH: usize = 64;
pub(crate) const MAX_SVG_RASTER_PIXELS: u64 = 16_777_216;

pub(crate) fn validate_svg_source(data: &[u8]) -> bool {
    if data.len() > MAX_SVG_BYTES {
        return false;
    }
    let Ok(source) = std::str::from_utf8(data) else {
        return false;
    };
    svg_markup_within_limits(source)
}

pub(crate) fn checked_svg_raster_size(size: (f32, f32), scale: f32) -> Option<(i32, i32)> {
    if !valid_svg_raster_input(size, scale) {
        return None;
    }
    let width = (size.0 * scale).ceil();
    let height = (size.1 * scale).ceil();
    svg_raster_dimensions(width, height)
}

fn valid_svg_raster_input(size: (f32, f32), scale: f32) -> bool {
    size.0.is_finite()
        && size.1.is_finite()
        && scale.is_finite()
        && size.0 > 0.0
        && size.1 > 0.0
        && scale > 0.0
}

fn svg_raster_dimensions(width: f32, height: f32) -> Option<(i32, i32)> {
    if !width.is_finite()
        || !height.is_finite()
        || width > i32::MAX as f32
        || height > i32::MAX as f32
    {
        return None;
    }
    let pixels = (width as u64).checked_mul(height as u64)?;
    if pixels > MAX_SVG_RASTER_PIXELS {
        return None;
    }
    Some((width as i32, height as i32))
}

fn svg_markup_within_limits(source: &str) -> bool {
    let mut elements = 0;
    let mut depth = 0;
    for fragment in source.split('<').skip(1) {
        if !update_svg_markup_limits(fragment, &mut elements, &mut depth) {
            return false;
        }
    }
    depth == 0
}

fn update_svg_markup_limits(fragment: &str, elements: &mut usize, depth: &mut usize) -> bool {
    let tag = fragment.trim_start();
    if tag.starts_with('!') || tag.starts_with('?') {
        return true;
    }
    if tag.starts_with('/') {
        return close_svg_element(depth);
    }
    *elements += 1;
    if *elements > MAX_SVG_ELEMENTS {
        return false;
    }
    if tag
        .split('>')
        .next()
        .is_some_and(|opening| opening.trim_end().ends_with('/'))
    {
        return true;
    }
    *depth += 1;
    *depth <= MAX_SVG_DEPTH
}

fn close_svg_element(depth: &mut usize) -> bool {
    let Some(next_depth) = depth.checked_sub(1) else {
        return false;
    };
    *depth = next_depth;
    true
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_SVG_BYTES, MAX_SVG_RASTER_PIXELS, checked_svg_raster_size, validate_svg_source,
    };

    #[test]
    fn checked_svg_raster_size_rejects_invalid_or_excessive_dimensions() {
        assert_eq!(checked_svg_raster_size((10.1, 5.0), 1.5), Some((16, 8)));
        assert_eq!(checked_svg_raster_size((f32::NAN, 5.0), 1.0), None);
        assert_eq!(checked_svg_raster_size((1.0, 1.0), f32::INFINITY), None);
        assert_eq!(
            checked_svg_raster_size((MAX_SVG_RASTER_PIXELS as f32 + 1024.0, 1.0), 1.0),
            None
        );
    }

    #[test]
    fn validate_svg_source_rejects_oversized_and_deep_documents() {
        let oversized = vec![b' '; MAX_SVG_BYTES + 1];
        let deep = format!("<svg>{}</svg>", "<g>".repeat(65) + &"</g>".repeat(65));

        assert!(!validate_svg_source(&oversized));
        assert!(!validate_svg_source(deep.as_bytes()));
        assert!(validate_svg_source(br#"<svg><path d="M0 0"/></svg>"#));
    }
}
