use super::*;
use crate::widget::InputState;
use taffy::prelude::Size;

struct SpacingApp;

impl AppLogic for SpacingApp {
    type State = ();
    type Message = ();

    fn new(_: &mut FontSystem) {}

    fn view(_: &mut ()) -> Widget<'_, ()> {
        Widget::Spacer {
            style: Style::default(),
        }
    }

    fn update(_: &mut (), _: (), _: &mut Clipboard) {}
}

#[test]
fn retained_input_viewports_exclude_annotations_and_match_narrow_painted_content() {
    for theme in [Theme::light(), Theme::dark()] {
        for width in [30.0, 60.0, 140.0] {
            for multiline in [false, true] {
                let style = Style {
                    size: Size::from_lengths(width, if multiline { 100.0 } else { 40.0 }),
                    ..Style::default()
                };
                let widget = if multiline {
                    Widget::text_area(
                        |_| (),
                        None,
                        style,
                        "Channel",
                        InputState::Error,
                        "",
                        Some("Invalid".into()),
                    )
                } else {
                    Widget::controlled_text_input(
                        "123",
                        |_| (),
                        None,
                        style,
                        "Channel",
                        "",
                        InputState::Error,
                        Some("Invalid".into()),
                        false,
                    )
                }
                .with_id(17);
                let fonts = Rc::new(RefCell::new(FontSystem::new()));
                let mut taffy = TaffyTree::new();
                let root = crate::layout::build_taffy_tree(
                    &mut taffy,
                    &widget,
                    fonts.clone(),
                    &HashMap::new(),
                );
                compute_layout(
                    &mut taffy,
                    root,
                    PhysicalSize::new(width as u32, 300),
                    fonts,
                    &crate::render::RichTextRenderer::default(),
                );
                let mut caches = WidgetRuntimeCaches::default();
                RutterEngine::<SpacingApp>::sync_runtime_metadata(
                    &mut caches,
                    &mut HashMap::new(),
                    &mut HashMap::new(),
                    RuntimeMetadataSources {
                        input_states: &HashMap::new(),
                        taffy: &taffy,
                    },
                    &widget,
                    Some(root),
                    theme.spacing,
                )
                .unwrap();
                let runtime = &caches.inputs[&17];
                assert_eq!(runtime.visible_w, (width - theme.spacing * 4.0).max(1.0));
                assert_eq!(
                    runtime.visible_h,
                    (if multiline { 100.0 } else { 40.0 }) - theme.spacing * 2.0
                );
                assert_eq!(runtime.is_multiline, multiline);
                assert_eq!(caches.input_order, vec![17]);
            }
        }
    }
}
