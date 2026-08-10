// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use skia_safe::canvas::Canvas;

pub(super) fn logical_canvas_size(canvas: &Canvas, scale: f32) -> (f32, f32) {
    let dimensions = canvas.image_info().dimensions();
    let logical_scale = scale.max(f32::EPSILON);
    (
        dimensions.width as f32 / logical_scale,
        dimensions.height as f32 / logical_scale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use skia_safe::surfaces;

    #[test]
    fn logical_size_divides_physical_canvas_by_scale() {
        let mut surface = surfaces::raster_n32_premul((300, 180)).unwrap();

        assert_eq!(logical_canvas_size(surface.canvas(), 2.0), (150.0, 90.0));
    }
}
