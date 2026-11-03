//! Callback ownership preserves table geometry, painting, identity and semantics.
use super::*;
use crate::accessibility::{AccessibilityInputs, build_accessibility_update};
use crate::render::hit_test::HitResult;
use crate::table::*;

const SELECTED: &[TableRowKey] = &[TableRowKey::new(30), TableRowKey::new(99)];

fn table(captured: bool, mode: usize, width: f32, enabled: bool) -> Widget<'static, ()> {
    let columns = vec![
        TableColumn::new(
            TableColumnKey::new(1),
            "Long localized name 東京 القاهرة",
            TableColumnWidth::flex(140.0, 1).unwrap(),
        )
        .with_sortable(true),
        TableColumn::new(
            TableColumnKey::new(2),
            "Status",
            TableColumnWidth::fixed(140.0).unwrap(),
        ),
    ];
    let rows = [30, 10, 20, 40]
        .into_iter()
        .map(|key| {
            TableRow::new(
                TableRowKey::new(key),
                [
                    TableCell::new("Long localized content 東京 القاهرة"),
                    TableCell::new("Ready"),
                ],
            )
        })
        .collect::<Vec<_>>();
    let selected = match mode {
        0 => TableSelection::none(),
        1 => TableSelection::single(Some(SELECTED[0]), |_| ()),
        _ => TableSelection::multiple(SELECTED, |_| ()),
    };
    let sort = Some(TableSort::new(
        TableColumnKey::new(1),
        TableSortDirection::Descending,
    ));
    let options = if captured {
        let context = String::from("owned selection context");
        let selected = match mode {
            0 => TableSelectionWith::none(),
            1 => TableSelectionWith::single(Some(SELECTED[0]), move |_| {
                let _keep_alive = &context;
            }),
            _ => TableSelectionWith::multiple(SELECTED, move |_| {
                let _keep_alive = &context;
            }),
        };
        let context = String::from("owned sort context");
        TableOptions::default()
            .selection_with(selected)
            .sorting_with(TableSortingWith::new(sort, move |_| {
                let _keep_alive = &context;
            }))
    } else {
        TableOptions::default()
            .with_selection(selected)
            .with_sorting(TableSorting::new(sort, |_| ()))
    };
    Widget::table_with_options(
        TableModel::new("Inventory", columns, rows).unwrap(),
        options,
        Style {
            size: Size::from_lengths(width, 132.0),
            ..Style::default()
        },
    )
    .with_id(42)
    .enabled(enabled)
}

#[test]
fn captured_tables_geometry_raster_accessibility_and_hit_parity() {
    let mut fonts = FontSystem::new();
    let mut swash = SwashCache::new();
    let mut font_cache = HashMap::new();
    let mut text_cache = TextBufferCache::default();
    let renderer = RichTextRenderer::new();
    for mode in 0..3 {
        for enabled in [true, false] {
            for (width, direction, theme) in [
                (320, LayoutDirection::Ltr, Theme::light()),
                (320, LayoutDirection::Rtl, Theme::dark()),
                (800, LayoutDirection::Rtl, Theme::light()),
                (800, LayoutDirection::Ltr, Theme::dark()),
            ] {
                for scale in [1.0_f32, 1.5, 2.0] {
                    let mut expected_geometry = None;
                    let mut expected_accessibility = None;
                    let mut expected_hits = None;
                    let mut expected_identity = None;
                    let mut pixels = Vec::new();
                    for captured in [false, true] {
                        let widget = table(captured, mode, width as f32, enabled);
                        let mut tree = TaffyTree::new();
                        let root = build_layout_tree(&mut tree, &widget, direction);
                        compute_layout_with_renderer(
                            &mut tree,
                            root,
                            PhysicalSize::new(width, 96),
                            &renderer,
                        );
                        let actual = geometry(&tree, root);
                        assert_eq!(actual[0].2, width as f32);
                        assert_eq!(actual[0].3, 132.0);
                        if let Some(expected) = &expected_geometry {
                            assert_eq!(&actual, expected);
                        } else {
                            expected_geometry = Some(actual);
                        }
                        let identity = validate_widget_id_snapshot(&widget).unwrap();
                        if let Some(expected) = &expected_identity {
                            assert_eq!(&identity, expected);
                        } else {
                            expected_identity = Some(identity);
                        }
                        let states = HashMap::new();
                        let inputs = HashMap::new();
                        let update = build_accessibility_update(
                            &tree,
                            &widget,
                            root,
                            AccessibilityInputs {
                                input_states: &inputs,
                                widget_states: &states,
                                focused_widget_id: Some(42),
                                viewport: (width as f32, 96.0),
                                direction,
                            },
                        );
                        let props: Vec<_> = update
                            .nodes
                            .iter()
                            .map(|(id, node)| {
                                (
                                    *id,
                                    node.role(),
                                    node.label().map(str::to_owned),
                                    node.bounds(),
                                    node.is_selected(),
                                    node.sort_direction(),
                                    node.is_multiselectable(),
                                    node.is_disabled(),
                                    node.supports_action(accesskit::Action::Click),
                                )
                            })
                            .collect();
                        if let Some(expected) = &expected_accessibility {
                            assert_eq!(&props, expected);
                        } else {
                            expected_accessibility = Some(props);
                        }
                        assert!(update.nodes.iter().any(|(_, node)| node.sort_direction()
                            == Some(accesskit::SortDirection::Descending)));
                        let hits: Vec<_> = [Point::new(20.0, 20.0), Point::new(20.0, 60.0)]
                            .into_iter()
                            .map(|point| {
                                match crate::render::hit_test::hit_test(
                                    &widget,
                                    &tree,
                                    root,
                                    point,
                                    Point::new(0.0, 0.0),
                                    &states,
                                ) {
                                    Some(HitResult::TableActivate { id, target }) => {
                                        Some((id, Some(target)))
                                    }
                                    Some(HitResult::ScrollFocus(id)) => Some((id, None)),
                                    None => None,
                                    _ => panic!("unexpected table hit"),
                                }
                            })
                            .collect();
                        if let Some(expected) = &expected_hits {
                            assert_eq!(&hits, expected);
                        } else {
                            expected_hits = Some(hits);
                        }
                        let mut surface = surfaces::raster_n32_premul((
                            (width as f32 * scale) as i32,
                            (96.0 * scale) as i32,
                        ))
                        .unwrap();
                        surface.canvas().clear(Color::TRANSPARENT);
                        surface.canvas().scale((scale, scale));
                        draw_widgets(
                            surface.canvas(),
                            &tree,
                            root,
                            &widget,
                            &mut fonts,
                            &mut swash,
                            Point::new(-1.0, -1.0),
                            Some(42),
                            &inputs,
                            &states,
                            &mut font_cache,
                            &mut text_cache,
                            true,
                            &theme,
                            scale,
                        );
                        pixels.push(surface.peek_pixels().unwrap().bytes().unwrap().to_vec());
                    }
                    assert!(pixels[0].iter().any(|byte| *byte != 0));
                    assert!(
                        pixels[0] == pixels[1],
                        "mode={mode} enabled={enabled} width={width} scale={scale}"
                    );
                }
            }
        }
    }
}
