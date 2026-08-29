use skia_safe::{Color, Contains, Point, Surface, surfaces};

use super::*;
use crate::widgets::search::SEARCH_BAR_LEADING_TEXT_INSET;

#[test]
fn search_magnifier_draws_separate_lens_and_handle_regions() {
    let mut surface = transparent_surface();

    let bounds = draw_search_magnifier(surface.canvas(), Point::new(8.0, 8.0), 3.5, Color::BLACK);
    let text_origin = 16.0 + SEARCH_BAR_LEADING_TEXT_INSET;

    assert!(bounds.left >= 3.0);
    assert!(text_origin - bounds.right >= 4.0);
    assert!(region_has_ink(&mut surface, (4, 4), (12, 12)));
    assert!(region_has_ink(&mut surface, (11, 11), (16, 16)));
}

#[test]
fn control_chevron_tip_follows_each_requested_direction() {
    let cases = [
        (ControlChevronDirection::Up, (12, 9)),
        (ControlChevronDirection::Down, (12, 15)),
        (ControlChevronDirection::Left, (9, 12)),
        (ControlChevronDirection::Right, (15, 12)),
    ];

    for (direction, tip) in cases {
        let mut surface = transparent_surface();
        let bounds = draw_control_chevron(
            surface.canvas(),
            Point::new(12.0, 12.0),
            4.0,
            direction,
            Color::BLACK,
        );
        assert!(bounds.contains(Point::new(tip.0 as f32, tip.1 as f32)));
        assert!(region_has_ink(&mut surface, tip, (tip.0 + 1, tip.1 + 1)));
    }
}

fn transparent_surface() -> Surface {
    let mut surface = surfaces::raster_n32_premul((24, 24)).unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    surface
}

fn region_has_ink(surface: &mut Surface, start: (i32, i32), end_exclusive: (i32, i32)) -> bool {
    let pixels = surface.peek_pixels().unwrap();
    (start.1..end_exclusive.1)
        .any(|y| (start.0..end_exclusive.0).any(|x| pixels.get_color((x, y)).a() > 0))
}
