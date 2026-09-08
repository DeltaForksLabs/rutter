use rutter::{HeadingLevel, TableOfContentsOptions, Widget, WidgetIdSnapshot};
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

#[test]
fn configured_table_of_contents_exposes_columns_and_an_expanded_accordion() {
    let options = TableOfContentsOptions::new(2).unwrap().with_accordion(());
    let widget: Widget<'_, ()> = Widget::table_of_contents_with_options(
        "On this page",
        Widget::heading(HeadingLevel::H2, "Overview", Style::default()),
        Style::default(),
        options,
    )
    .with_id(82);

    assert!(WidgetIdSnapshot::capture(&widget).is_ok());
    let Widget::TableOfContents { options, .. } = widget else {
        panic!("expected a configured TableOfContents widget");
    };
    assert_eq!(options.columns(), 2);
    assert!(options.is_accordion());
    assert!(options.is_expanded());
}
