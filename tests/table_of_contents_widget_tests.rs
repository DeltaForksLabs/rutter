use rutter::{HeadingLevel, Widget, WidgetIdSnapshot};
use taffy::prelude::Style;

#[test]
fn table_of_contents_constructor_preserves_semantic_headings_and_manual_ids() {
    let widget: Widget<'_, ()> = Widget::table_of_contents(
        "Contents",
        Widget::Column {
            children: vec![Widget::heading(
                HeadingLevel::H1,
                "Overview",
                Style::default(),
            )],
            style: Style::default(),
        },
        Style::default(),
    )
    .with_id(81);

    assert!(WidgetIdSnapshot::capture(&widget).is_ok());
    assert!(matches!(
        widget,
        Widget::TableOfContents {
            id: 81,
            title: "Contents",
            ..
        }
    ));
}
