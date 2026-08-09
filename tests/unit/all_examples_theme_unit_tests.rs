use cosmic_text::FontSystem;
use rutter::{AppLogic, MultiWindowAppLogic, SurfaceId, Theme, Widget};

use super::{
    accordion_demo::AccordionDemo, advanced_widgets_demo::AdvancedWidgetsDemo,
    button_content_demo::ButtonContentDemo, calendar_demo::CalendarDemo,
    carousel_demo::CarouselDemo, controls_demo::ControlsDemo, dialog_demo::DialogDemo,
    form_demo::MyApp, image_viewer_demo::ImageViewerDemo, modal_toast_demo::ModalToastDemo,
    multi_window_demo::MultiWindowDemo, popover_demo::PopoverDemo, progress_demo::ProgressDemo,
    rich_text_demo::RichTextDemo, scroll_demo::ScrollDemo, search_bar_demo::SearchBarDemo,
    slider_demo::SliderDemo, tab_demo::TabDemo, text_area_demo::TextAreaDemo,
    text_input_demo::TextInputDemo, vgrid_demo::VGridDemo, vlist_demo::VListDemo,
};

#[test]
fn foundational_examples_start_dark_with_theme_selectors() {
    assert_single_window_theme::<AccordionDemo>();
    assert_single_window_theme::<AdvancedWidgetsDemo>();
    assert_single_window_theme::<ButtonContentDemo>();
    assert_single_window_theme::<CalendarDemo>();
    assert_single_window_theme::<CarouselDemo>();
    assert_single_window_theme::<ControlsDemo>();
    assert_single_window_theme::<DialogDemo>();
}

#[test]
fn composite_examples_start_dark_with_theme_selectors() {
    assert_single_window_theme::<MyApp>();
    assert_single_window_theme::<ImageViewerDemo>();
    assert_single_window_theme::<ModalToastDemo>();
    assert_single_window_theme::<PopoverDemo>();
    assert_single_window_theme::<ProgressDemo>();
    assert_single_window_theme::<RichTextDemo>();
    assert_single_window_theme::<ScrollDemo>();
}

#[test]
fn input_and_virtual_examples_start_dark_with_theme_selectors() {
    assert_single_window_theme::<SearchBarDemo>();
    assert_single_window_theme::<SliderDemo>();
    assert_single_window_theme::<TabDemo>();
    assert_single_window_theme::<TextAreaDemo>();
    assert_single_window_theme::<TextInputDemo>();
    assert_single_window_theme::<VGridDemo>();
    assert_single_window_theme::<VListDemo>();
}

#[test]
fn multi_window_example_starts_dark_with_selectors_on_both_windows() {
    let mut fonts = FontSystem::new();
    let mut state = <MultiWindowDemo as MultiWindowAppLogic>::new(&mut fonts);
    assert_eq!(
        MultiWindowDemo::theme_for(&state).surface,
        Theme::dark().surface
    );
    let primary = MultiWindowDemo::view(&mut state, SurfaceId::PRIMARY);
    assert_theme_toggle(&primary);
    drop(primary);
    let second = MultiWindowDemo::view(&mut state, SurfaceId::new(1));
    assert_theme_toggle(&second);
}

fn assert_single_window_theme<Application: AppLogic>() {
    let mut fonts = FontSystem::new();
    let mut state = Application::new(&mut fonts);
    assert_eq!(
        Application::theme_for(&state).surface,
        Theme::dark().surface
    );
    assert_theme_toggle(&Application::view(&mut state));
}

fn assert_theme_toggle<Message>(widget: &Widget<'_, Message>) {
    assert!(root_contains_theme_toggle(widget, "Switch to Light theme"));
}

fn root_contains_theme_toggle<Message>(widget: &Widget<'_, Message>, expected: &str) -> bool {
    match widget {
        Widget::Column { children, .. } | Widget::Row { children, .. } => children.iter().any(
            |child| matches!(child, Widget::ButtonContent { label, .. } if *label == expected),
        ),
        _ => false,
    }
}
