use super::*;
use crate::widgets::dropdown_menu::{
    DropdownMenuEntry, DropdownMenuState, build_open_menu_surfaces,
};
use skia_safe::{Color, Surface, surfaces};
use taffy::prelude::{Dimension, Rect, Size, Style};

const MENU_ID: u64 = 42;
const VIEWPORT: (f32, f32) = (800.0, 500.0);

struct NativeMenuRaster<'a> {
    widget: Widget<'a, ()>,
    states: HashMap<u64, WidgetState>,
    taffy: TaffyTree<RutterContext>,
    root: NodeId,
    direction: LayoutDirection,
}

impl<'a> NativeMenuRaster<'a> {
    fn dropdown(direction: LayoutDirection, background: Color) -> Self {
        let entries = vec![
            DropdownMenuEntry::item("Run", ()),
            DropdownMenuEntry::item("Save", ()),
            DropdownMenuEntry::submenu(
                "More",
                vec![
                    DropdownMenuEntry::item("Child", ()),
                    DropdownMenuEntry::submenu("Deeper", vec![DropdownMenuEntry::item("Leaf", ())]),
                    DropdownMenuEntry::item("Other", ()),
                ],
            ),
            DropdownMenuEntry::item("Quit", ()),
        ];
        let mut state = DropdownMenuState::default();
        assert!(state.open_submenu(&entries, vec![2, 1]));
        let widget = Widget::Container {
            child: Box::new(
                Widget::dropdown_menu(
                    "Actions",
                    entries,
                    Style {
                        size: Size {
                            width: Dimension::length(120.0),
                            height: Dimension::length(40.0),
                        },
                        ..Style::default()
                    },
                )
                .with_id(MENU_ID),
            ),
            style: Style {
                size: Size::percent(1.0_f32),
                padding: Rect::length(32.0_f32),
                ..Style::default()
            },
            color: Some(background),
            radius: 0.0,
        };
        let states = HashMap::from([(MENU_ID, WidgetState::DropdownMenu(state))]);
        Self::from_widget(widget, states, direction)
    }

    fn context(entries: &'a [ContextMenuEntry<'a, ()>], direction: LayoutDirection) -> Self {
        let widget = Widget::context_menu(
            Widget::Spacer {
                style: Style::default(),
            },
            entries,
            Style::default(),
        )
        .with_id(MENU_ID);
        let mut state = crate::engine::widget_state::ContextMenuState::default();
        state.open_at(330.0, 60.0);
        state.navigation.expand_submenu(vec![2, 1], Some(0));
        Self::from_widget(
            widget,
            HashMap::from([(MENU_ID, WidgetState::ContextMenu(state))]),
            direction,
        )
    }

    fn from_widget(
        widget: Widget<'a, ()>,
        states: HashMap<u64, WidgetState>,
        direction: LayoutDirection,
    ) -> Self {
        let fonts = Rc::new(RefCell::new(FontSystem::new()));
        let mut taffy = TaffyTree::new();
        let root =
            build_taffy_tree_with_direction(&mut taffy, &widget, fonts.clone(), &states, direction);
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(800, 500),
            fonts,
            &RichTextRenderer::default(),
        );
        Self {
            widget,
            states,
            taffy,
            root,
            direction,
        }
    }

    fn panels(&self) -> Vec<SkiaRect> {
        if self.states[&MENU_ID].as_context_menu().is_some() {
            let font = get_cached_font(&mut HashMap::new(), "sans-serif", Theme::dark().font_body);
            let mut menus = Vec::new();
            collect_open_context_menus(&self.widget, &self.states, &mut Vec::new(), &mut menus);
            return context_menu_overlay::context_surfaces(
                &menus[0],
                VIEWPORT,
                self.direction,
                &font,
            )
            .into_iter()
            .map(|panel| panel.rect)
            .collect();
        }
        let overlays = select_overlay::collector::collect_open_dropdown_overlays(
            &self.widget,
            &self.taffy,
            self.root,
            &self.states,
            VIEWPORT,
        );
        let menu = &overlays[0];
        build_open_menu_surfaces(
            menu.anchor,
            menu.entries,
            &menu.state,
            SkiaRect::from_xywh(0.0, 0.0, VIEWPORT.0, VIEWPORT.1),
            self.direction,
        )
        .into_iter()
        .map(|panel| panel.rect)
        .collect()
    }

    fn paint(&self, surface: &mut Surface, origin: Point, scale: f32, theme: &Theme) {
        draw_native_menu(
            surface.canvas(),
            &self.taffy,
            self.root,
            &self.widget,
            &self.states,
            if self.states[&MENU_ID].as_context_menu().is_some() {
                NativeMenuKind::Context(MENU_ID)
            } else {
                NativeMenuKind::Dropdown(MENU_ID)
            },
            origin,
            VIEWPORT,
            Point::new(-10.0, -10.0),
            &mut HashMap::new(),
            &mut ImageRenderCache::default(),
            theme,
            scale,
            self.direction,
        );
    }
}

fn popup_bounds(panels: &[SkiaRect]) -> SkiaRect {
    panels.iter().skip(1).fold(panels[0], |bounds, panel| {
        SkiaRect::from_ltrb(
            bounds.left.min(panel.left),
            bounds.top.min(panel.top),
            bounds.right.max(panel.right),
            bounds.bottom.max(panel.bottom),
        )
    })
}

fn pixel_at(surface: &mut Surface, point: Point, origin: Point, scale: f32) -> Color {
    surface.peek_pixels().unwrap().get_color((
        ((point.x - origin.x) * scale) as i32,
        ((point.y - origin.y) * scale) as i32,
    ))
}

#[test]
fn native_dropdown_clears_union_gaps_without_painting_parent_layout() {
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        let menu = NativeMenuRaster::dropdown(direction, Color::RED);
        let panels = menu.panels();
        assert_eq!(panels.len(), 3);
        let bounds = popup_bounds(&panels);
        let origin = Point::new(bounds.left, bounds.top);
        let gap = Point::new(panels[1].center_x(), panels[0].top + 12.0);
        assert!(bounds.contains(gap));
        assert!(panels.iter().all(|panel| !panel.contains(gap)));
        for theme in [Theme::light(), Theme::dark()] {
            for scale in [1.0, 1.5] {
                let mut surface = surfaces::raster_n32_premul((
                    (bounds.width() * scale).ceil() as i32,
                    (bounds.height() * scale).ceil() as i32,
                ))
                .unwrap();
                surface.canvas().clear(Color::RED);
                menu.paint(&mut surface, origin, scale, &theme);
                assert_eq!(
                    pixel_at(&mut surface, gap, origin, scale),
                    Color::TRANSPARENT
                );
                for panel in &panels {
                    let inside = Point::new(panel.left + 10.0, panel.top + 10.0);
                    assert_ne!(
                        pixel_at(&mut surface, inside, origin, scale),
                        Color::TRANSPARENT
                    );
                }
            }
        }
    }
}

#[test]
fn native_dropdown_repaint_removes_collapsed_submenu_pixels() {
    let mut menu = NativeMenuRaster::dropdown(LayoutDirection::Ltr, Color::BLUE);
    let panels = menu.panels();
    let bounds = popup_bounds(&panels);
    let origin = Point::new(bounds.left, bounds.top);
    let removed = Point::new(panels[2].left + 10.0, panels[2].top + 10.0);
    let mut surface =
        surfaces::raster_n32_premul((bounds.width().ceil() as i32, bounds.height().ceil() as i32))
            .unwrap();
    menu.paint(&mut surface, origin, 1.0, &Theme::dark());
    assert_ne!(
        pixel_at(&mut surface, removed, origin, 1.0),
        Color::TRANSPARENT
    );
    assert!(
        menu.states
            .get_mut(&MENU_ID)
            .unwrap()
            .as_dropdown_menu_mut()
            .unwrap()
            .collapse_submenu()
    );
    // Reuse the same buffer without a test-side clear, as in a subsequent frame.
    menu.paint(&mut surface, origin, 1.0, &Theme::dark());
    assert_eq!(
        pixel_at(&mut surface, removed, origin, 1.0),
        Color::TRANSPARENT
    );
    assert_ne!(
        pixel_at(
            &mut surface,
            Point::new(panels[0].left + 10.0, panels[0].top + 10.0),
            origin,
            1.0
        ),
        Color::TRANSPARENT,
    );
}

#[test]
fn native_context_menu_union_gaps_and_collapsed_panels_are_transparent() {
    let icon = br##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="#ff0000"/></svg>"##;
    let entries = [
        ContextMenuEntry::item("Run", ())
            .with_svg_icon(icon)
            .with_shortcut_label("CTRL+R"),
        ContextMenuEntry::separator(),
        ContextMenuEntry::submenu(
            "More",
            vec![
                ContextMenuEntry::item("Child", ())
                    .with_svg_icon(icon)
                    .with_shortcut_label("CTRL+C"),
                ContextMenuEntry::submenu(
                    "Deeper",
                    vec![
                        ContextMenuEntry::item("Leaf", ())
                            .with_svg_icon(icon)
                            .with_shortcut_label("CTRL+L"),
                    ],
                )
                .with_svg_icon(icon),
            ],
        )
        .with_shortcut_label("ALT+M")
        .with_svg_icon(icon),
    ];
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        let mut menu = NativeMenuRaster::context(&entries, direction);
        let panels = menu.panels();
        assert_eq!(panels.len(), 3);
        let bounds = popup_bounds(&panels);
        let origin = Point::new(bounds.left, bounds.top);
        let gap = Point::new(panels[1].center_x(), panels[0].top + 12.0);
        assert!(bounds.contains(gap));
        assert!(panels.iter().all(|panel| !panel.contains(gap)));
        let removed = Point::new(panels[2].center_x(), panels[2].center_y());
        for scale in [1.0, 1.5, 2.0] {
            let mut surface = surfaces::raster_n32_premul((
                (bounds.width() * scale).ceil() as i32,
                (bounds.height() * scale).ceil() as i32,
            ))
            .unwrap();
            surface.canvas().clear(Color::RED);
            menu.paint(&mut surface, origin, scale, &Theme::dark());
            assert_eq!(
                pixel_at(&mut surface, gap, origin, scale),
                Color::TRANSPARENT
            );
            assert_ne!(
                pixel_at(&mut surface, removed, origin, scale),
                Color::TRANSPARENT
            );
        }
        let mut surface = surfaces::raster_n32_premul((
            bounds.width().ceil() as i32,
            bounds.height().ceil() as i32,
        ))
        .unwrap();
        menu.paint(&mut surface, origin, 1.0, &Theme::dark());
        assert!(
            menu.states
                .get_mut(&MENU_ID)
                .unwrap()
                .as_context_menu_mut()
                .unwrap()
                .navigation
                .collapse_submenu()
        );
        menu.paint(&mut surface, origin, 1.0, &Theme::dark());
        assert_eq!(
            pixel_at(&mut surface, removed, origin, 1.0),
            Color::TRANSPARENT
        );
    }
}
