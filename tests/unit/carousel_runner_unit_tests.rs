//! Real carousel publication, input routing, source callbacks and raster selection.
use super::*;
use crate::engine::test_surface::MemoryWindow;
use crate::widgets::carousel::{CarouselPosition, CarouselState};
use crate::{CarouselConfig, Theme, Widget};
use taffy::prelude::{Dimension, Size, Style};
use winit::dpi::PhysicalSize;

struct CarouselApp<const RTL: bool = false>;

#[derive(Default)]
struct Model {
    picks: Vec<usize>,
    snapping: bool,
    weighted: bool,
}

impl<const RTL: bool> AppLogic for CarouselApp<RTL> {
    type State = Model;
    type Message = usize;

    fn new(_: &mut FontSystem) -> Model {
        Model::default()
    }

    fn locale() -> crate::i18n::Locale {
        crate::i18n::Locale::parse(if RTL { "ar" } else { "en-US" }).unwrap()
    }

    fn view(model: &mut Model) -> Widget<'_, usize> {
        Widget::carousel_view(
            30,
            |index| {
                Some(Widget::Text {
                    content: format!("Projeto / Project #{index}"),
                    style: Style::default(),
                    color: None,
                    size: 16.0,
                })
            },
            |index| index,
            if model.weighted {
                CarouselConfig::weighted([1, 6, 1]).unwrap()
            } else {
                CarouselConfig::uncontained(100.0).unwrap()
            }
            .with_item_snapping(model.snapping),
            Style {
                size: Size {
                    width: Dimension::percent(1.0),
                    height: Dimension::length(90.0),
                },
                ..Style::default()
            },
        )
        .with_id(811)
    }

    fn update(model: &mut Model, index: usize, _: &mut arboard::Clipboard) {
        model.picks.push(index);
    }
}

fn publish<const RTL: bool>(runner: &mut RutterRunner<CarouselApp<RTL>>) {
    runner.refresh_layout_before_keyboard_input().unwrap();
}

fn runner(width: u32) -> RutterRunner<CarouselApp> {
    configured_runner(width, false)
}

fn configured_runner<const RTL: bool>(
    width: u32,
    weighted: bool,
) -> RutterRunner<CarouselApp<RTL>> {
    let mut engine = RutterEngine::<CarouselApp<RTL>>::new().unwrap();
    engine.app_state.weighted = weighted;
    engine.window = Some(Rc::new(MemoryWindow {
        size: PhysicalSize::new(width, 100),
        scale: 1.0,
        popup: false,
    }));
    let mut runner = RutterRunner::with_engine(engine);
    runner.engine.active_scroll_id = Some(811);
    publish(&mut runner);
    runner
}

fn state<const RTL: bool>(runner: &RutterRunner<CarouselApp<RTL>>) -> &CarouselState {
    runner.engine.widget_states[&811].as_carousel().unwrap()
}

fn key<const RTL: bool>(runner: &mut RutterRunner<CarouselApp<RTL>>, named: NamedKey) {
    assert!(runner.handle_carousel_key(811, &Key::Named(named)));
    publish(runner);
}

fn click<const RTL: bool>(runner: &mut RutterRunner<CarouselApp<RTL>>, x: f32, expected: usize) {
    let widget = CarouselApp::<RTL>::view(&mut runner.engine.app_state);
    let hit = hit_test(
        &widget,
        &runner.engine.taffy,
        runner.engine.last_root_node,
        Point::new(x, 45.0),
        Point::default(),
        &runner.engine.widget_states,
    );
    assert!(matches!(hit, Some(HitResult::CarouselSelect { id: 811, index }) if index == expected));
    drop(widget);
    runner.activate_carousel_item(811, expected);
    publish(runner);
    assert_eq!(runner.engine.app_state.picks.last(), Some(&expected));
}

#[test]
fn carousel_runner_click_wheel_keyboard_and_tail_keep_source_selection() {
    for width in [320, 800] {
        let mut runner = runner(width);
        click(&mut runner, 150.0, 1);
        click(&mut runner, 250.0, 2);
        assert_eq!(state(&runner).position, CarouselPosition::default());
        assert!(runner.scroll_active_target(1025.0, 0.0));
        publish(&mut runner);
        assert_eq!(
            state(&runner).position,
            CarouselPosition::with_progress(10, 0.25)
        );
        assert_eq!(runner.engine.app_state.picks, [1, 2]);
        key(&mut runner, NamedKey::ArrowRight);
        assert_eq!(runner.engine.app_state.picks, [1, 2, 11]);
        assert_eq!(
            state(&runner).position,
            CarouselPosition::with_progress(10, 0.25)
        );
        key(&mut runner, NamedKey::ArrowLeft);
        assert_eq!(state(&runner).selected_item, Some(10));
        key(&mut runner, NamedKey::End);
        let tail = state(&runner).position;
        assert_eq!(state(&runner).selected_item, Some(29));
        let expected_tail = CarouselPosition::exact(30).shifted(-(width as f32 / 100.0));
        assert_eq!(tail, expected_tail);
        key(&mut runner, NamedKey::ArrowLeft);
        assert_eq!(state(&runner).selected_item, Some(28));
        assert_eq!(state(&runner).position, tail);
        key(&mut runner, NamedKey::Home);
        assert_eq!(state(&runner).position, CarouselPosition::default());
        assert!(runner.scroll_active_target(1025.0, 0.0));
        publish(&mut runner);
        key(&mut runner, NamedKey::ArrowLeft);
        assert_eq!(state(&runner).selected_item, Some(9));
        assert_eq!(state(&runner).position, CarouselPosition::exact(9));
        assert!(runner.scroll_active_target(-125.0, 0.0));
        publish(&mut runner);
        key(&mut runner, NamedKey::ArrowRight);
        // Selection 9 is still visible after reverse wheel input: retain it as origin.
        assert_eq!(state(&runner).selected_item, Some(10));
    }
}

#[test]
fn carousel_weighted_runner_keyboard_origin_follows_scrolled_region_ltr_and_rtl() {
    weighted_keyboard_origin::<false>();
    weighted_keyboard_origin::<true>();
}

fn weighted_keyboard_origin<const RTL: bool>() {
    let forward = if RTL {
        NamedKey::ArrowLeft
    } else {
        NamedKey::ArrowRight
    };
    let reverse = if RTL {
        NamedKey::ArrowRight
    } else {
        NamedKey::ArrowLeft
    };
    let sign = if RTL { -1.0 } else { 1.0 };
    for width in [320, 800] {
        let mut runner = configured_runner::<RTL>(width, true);
        // Click the narrow trailing card through the actual directional layout.
        let x = width as f32 * if RTL { 0.0625 } else { 0.9375 };
        click(&mut runner, x, 1);
        assert_eq!(state(&runner).position, CarouselPosition::exact(1));
        let step = width as f32 / 8.0;
        assert!(runner.scroll_active_target(sign * step * 10.25, 0.0));
        publish(&mut runner);
        assert_eq!(
            state(&runner).position,
            CarouselPosition::with_progress(11, 0.25)
        );
        assert_eq!(state(&runner).selected_item, Some(1));
        assert_eq!(runner.engine.app_state.picks, [1]);
        key(&mut runner, forward);
        assert_eq!(state(&runner).position, CarouselPosition::exact(12));
        assert_eq!(state(&runner).selected_item, Some(12));
        assert_eq!(runner.engine.app_state.picks, [1, 12]);
        key(&mut runner, reverse);
        assert_eq!(state(&runner).selected_item, Some(11));
        assert!(runner.scroll_active_target(-sign * step * 5.25, 0.0));
        publish(&mut runner);
        assert_eq!(
            state(&runner).position,
            CarouselPosition::with_progress(5, 0.75)
        );
        assert_eq!(runner.engine.app_state.picks, [1, 12, 11]);
        key(&mut runner, reverse);
        assert_eq!(state(&runner).selected_item, Some(5));
        // An explicit selection that remains visible wins over the rounded origin.
        assert!(runner.scroll_active_target(-sign * step * 0.75, 0.0));
        publish(&mut runner);
        assert_eq!(
            state(&runner).position,
            CarouselPosition::with_progress(4, 0.25)
        );
        assert_eq!(state(&runner).selected_item, Some(5));
        assert_eq!(runner.engine.app_state.picks, [1, 12, 11, 5]);
        key(&mut runner, forward);
        assert_eq!(state(&runner).selected_item, Some(6));
        key(&mut runner, NamedKey::End);
        assert_eq!(state(&runner).position, CarouselPosition::exact(29));
        assert!(!runner.handle_carousel_key(811, &Key::Named(forward)));
        assert_eq!(state(&runner).selected_item, Some(29));
        assert_eq!(runner.engine.app_state.picks, [1, 12, 11, 5, 6, 29]);
        assert!(runner.scroll_active_target(-sign * step * 8.25, 0.0));
        publish(&mut runner);
        assert_eq!(
            state(&runner).position,
            CarouselPosition::with_progress(20, 0.75)
        );
        key(&mut runner, forward);
        assert_eq!(state(&runner).position, CarouselPosition::exact(22));
        key(&mut runner, NamedKey::Home);
        assert_eq!(state(&runner).position, CarouselPosition::default());
        assert!(!runner.handle_carousel_key(811, &Key::Named(reverse)));
        assert_eq!(state(&runner).selected_item, Some(0));
        assert_eq!(runner.engine.app_state.picks, [1, 12, 11, 5, 6, 29, 22, 0]);
    }
}

#[test]
fn carousel_runner_opt_in_snapping_remains_supported() {
    let mut runner = runner(320);
    runner.engine.app_state.snapping = true;
    runner.engine.layout_dirty = true;
    publish(&mut runner);
    assert!(runner.scroll_active_target(1.0, 0.0));
    publish(&mut runner);
    assert_eq!(state(&runner).position, CarouselPosition::exact(1));
    assert!(runner.engine.app_state.picks.is_empty());
    click(&mut runner, 150.0, 2);
    assert_eq!(state(&runner).position, CarouselPosition::exact(1));
}

#[test]
fn carousel_rtl_wheel_and_keyboard_follow_visible_region() {
    let config = CarouselConfig::uncontained(100.0).unwrap();
    let mut state = CarouselState::default();
    state.sync_viewport(320.0, &config, 30);
    state.select(0, &config, 30);
    let delta = carousel_wheel_delta(-1025.0, 0.0, LayoutDirection::Rtl);
    assert!(state.scroll_by_pixels(delta, &config, 30));
    let current = state.current_index(&config, 30).unwrap();
    let next = carousel_key_index(
        &Key::Named(NamedKey::ArrowLeft),
        current,
        30,
        LayoutDirection::Rtl,
    )
    .unwrap();
    state.select(next, &config, 30);
    assert_eq!(state.selected_item, Some(11));
    assert_eq!(state.position, CarouselPosition::with_progress(10, 0.25));
    let previous = carousel_key_index(
        &Key::Named(NamedKey::ArrowRight),
        next,
        30,
        LayoutDirection::Rtl,
    )
    .unwrap();
    state.select(previous, &config, 30);
    assert_eq!(state.selected_item, Some(10));
    assert_eq!(state.position, CarouselPosition::with_progress(10, 0.25));
}

fn raster(
    runner: &mut RutterRunner<CarouselApp>,
    width: u32,
    direction: LayoutDirection,
    theme: &Theme,
    scale: f32,
) -> skia_safe::Surface {
    let position = state(runner).position;
    let widget = CarouselApp::<false>::view(&mut runner.engine.app_state);
    let mut taffy = taffy::TaffyTree::new();
    let fonts = runner.engine.font_system.clone();
    let root = crate::layout::build_taffy_tree_with_direction(
        &mut taffy,
        &widget,
        fonts.clone(),
        &HashMap::new(),
        direction,
    );
    crate::layout::compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(width, 100),
        fonts,
        &crate::render::RichTextRenderer::default(),
    );
    let runtime = &runner.engine.runtime_caches.carousels[&811];
    for frame in crate::widgets::carousel::geometry::carousel_item_frames(
        &runtime.config,
        position,
        width as f32,
        30,
        direction,
    ) {
        let start = frame.x.max(0.0);
        let end = (frame.x + frame.width).min(width as f32);
        if end <= start {
            continue;
        }
        let hit = hit_test(
            &widget,
            &taffy,
            root,
            Point::new((start + end) / 2.0, 45.0),
            Point::default(),
            &runner.engine.widget_states,
        );
        assert!(
            matches!(hit, Some(HitResult::CarouselSelect { id: 811, index }) if index == frame.index)
        );
    }
    let mut surface = skia_safe::surfaces::raster_n32_premul((
        (width as f32 * scale) as i32,
        (100.0 * scale) as i32,
    ))
    .unwrap();
    surface.canvas().scale((scale, scale));
    crate::render::draw_widgets(
        surface.canvas(),
        &taffy,
        root,
        &widget,
        &mut runner.engine.font_system.borrow_mut(),
        &mut cosmic_text::SwashCache::new(),
        Point::new(-10.0, -10.0),
        None,
        &runner.engine.input_states,
        &runner.engine.widget_states,
        &mut HashMap::new(),
        &mut crate::render::text::TextBufferCache::default(),
        true,
        theme,
        scale,
    );
    surface
}

#[test]
fn carousel_painted_selection_matches_source_hits_at_fractional_scroll_in_ltr_and_rtl() {
    use crate::widgets::carousel::geometry::carousel_item_frames;
    for width in [320, 800] {
        let mut runner = runner(width);
        assert!(runner.scroll_active_target(1025.0, 0.0));
        publish(&mut runner);
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            let runtime = runner.engine.runtime_caches.carousels[&811].clone();
            let position = state(&runner).position;
            let frames =
                carousel_item_frames(&runtime.config, position, width as f32, 30, direction);
            for selected in [11, 12] {
                let frame = frames.iter().find(|frame| frame.index == selected).unwrap();
                let x = frame.x + frame.width / 2.0;
                assert_eq!(
                    state(&runner).index_at(x, &runtime.config, 30, direction),
                    Some(selected)
                );
                for theme in [Theme::light(), Theme::dark()] {
                    for scale in [1.0_f32, 1.5, 2.0] {
                        runner
                            .engine
                            .widget_states
                            .get_mut(&811)
                            .unwrap()
                            .as_carousel_mut()
                            .unwrap()
                            .selected_item = None;
                        let mut before = raster(&mut runner, width, direction, &theme, scale);
                        runner.activate_carousel_item(811, selected);
                        publish(&mut runner);
                        assert_eq!(state(&runner).position, position);
                        assert_eq!(runner.engine.app_state.picks.last(), Some(&selected));
                        let mut after = raster(&mut runner, width, direction, &theme, scale);
                        let before = before.peek_pixels().unwrap();
                        let after = after.peek_pixels().unwrap();
                        let mut changed = 0;
                        for y in 0..after.height() {
                            for x in 0..after.width() {
                                if before.get_color((x, y)) != after.get_color((x, y)) {
                                    let logical_x = x as f32 / scale;
                                    assert!(
                                        logical_x >= frame.x - 1.0
                                            && logical_x <= frame.x + frame.width + 1.0,
                                        "selection leaked into another source card"
                                    );
                                    changed += 1;
                                }
                            }
                        }
                        assert!(
                            changed > 0,
                            "selected source card must have a painted highlight"
                        );
                    }
                }
            }
        }
    }
}
