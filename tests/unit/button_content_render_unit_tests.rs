use super::*;
use crate::layout::{build_taffy_tree, compute_layout};
use skia_safe::surfaces;
use taffy::prelude::{AlignItems, JustifyContent, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

#[test]
fn button_content_inherits_variant_foreground_and_preserves_explicit_colors() {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    for theme in [Theme::light(), Theme::dark()] {
        for variant in [
            ButtonVariant::Primary,
            ButtonVariant::Ghost,
            ButtonVariant::Text,
        ] {
            for hovered in [false, true] {
                for enabled in [false, true] {
                    for explicit in [None, Some(SkiaColor::RED)] {
                        let accent = Some(SkiaColor::from_rgb(70, 100, 150));
                        let expected_color = explicit
                            .unwrap_or_else(|| button_text_color(accent, variant, hovered, &theme));
                        let actual = content_button(variant, accent, explicit)
                            .enabled(enabled)
                            .with_id(71);
                        let expected = content_button(variant, accent, Some(expected_color))
                            .enabled(enabled)
                            .with_id(71);
                        let mut tree = TaffyTree::new();
                        let root =
                            build_taffy_tree(&mut tree, &actual, fonts.clone(), &HashMap::new());
                        compute_layout(
                            &mut tree,
                            root,
                            PhysicalSize::new(60, 36),
                            fonts.clone(),
                            &RichTextRenderer::default(),
                        );
                        let mut raster = Vec::new();
                        for widget in [&actual, &expected] {
                            let mut surface = surfaces::raster_n32_premul((60, 36)).unwrap();
                            surface.canvas().clear(theme.surface);
                            draw_widgets(
                                surface.canvas(),
                                &tree,
                                root,
                                widget,
                                &mut fonts.borrow_mut(),
                                &mut SwashCache::new(),
                                if hovered {
                                    Point::new(30.0, 18.0)
                                } else {
                                    Point::new(-1.0, -1.0)
                                },
                                Some(71),
                                &HashMap::new(),
                                &HashMap::new(),
                                &mut HashMap::new(),
                                &mut TextBufferCache::default(),
                                true,
                                &theme,
                                1.0,
                            );
                            let pixels = surface.peek_pixels().unwrap();
                            raster.push(
                                (0..36)
                                    .flat_map(|y| (0..60).map(move |x| (x, y)))
                                    .map(|point| pixels.get_color(point))
                                    .collect::<Vec<_>>(),
                            );
                        }
                        for (index, (actual, expected)) in
                            raster[0].iter().zip(&raster[1]).enumerate()
                        {
                            assert_eq!(
                                actual, expected,
                                "pixel={index}, variant={variant:?}, hovered={hovered}, enabled={enabled}, explicit={explicit:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

fn content_button(
    variant: ButtonVariant,
    accent: Option<SkiaColor>,
    color: Option<SkiaColor>,
) -> Widget<'static, ()> {
    Widget::ButtonContent {
        label: "28",
        child: Box::new(Widget::Container {
            child: Box::new(Widget::Text {
                content: "28".into(),
                style: Style::default(),
                color,
                size: 16.0,
            }),
            style: Style {
                size: Size::percent(1.0_f32),
                align_items: Some(AlignItems::Center),
                justify_content: Some(JustifyContent::Center),
                ..Style::default()
            },
            color: None,
            radius: 0.0,
        }),
        on_press: (),
        style: Style {
            size: Size::from_lengths(60.0, 36.0),
            ..Style::default()
        },
        color: accent,
        variant,
    }
}
