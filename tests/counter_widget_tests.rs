use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::FontSystem;
use rutter::engine::widget_state::WidgetState;
use rutter::layout::{build_taffy_tree, compute_layout};
use rutter::render::hit_test::{HitResult, hit_test};
use rutter::{CounterConfig, CounterParameters, Widget, WidgetId};
use skia_safe::Point;
use taffy::prelude::{Dimension, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

#[derive(Clone, Debug, PartialEq)]
enum Message {
    QuantityChanged(i64),
    AmountChanged(f64),
}

fn fixed_counter_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(160.0),
            height: Dimension::length(40.0),
        },
        ..Style::default()
    }
}

fn automatic_counter_style() -> Style {
    Style {
        size: Size {
            width: Dimension::auto(),
            height: Dimension::length(40.0),
        },
        ..Style::default()
    }
}

fn quantity_changed(value: i64) -> Message {
    Message::QuantityChanged(value)
}

fn counter(value: i64) -> Widget<'static, Message> {
    counter_with_style(value, fixed_counter_style())
}

fn counter_with_style(value: i64, style: Style) -> Widget<'static, Message> {
    Widget::counter(value, 0, 3, 1, quantity_changed, style, "Quantity").with_id(7)
}

fn counter_layout(
    widget: &Widget<'_, Message>,
) -> (TaffyTree<rutter::layout::RutterContext>, taffy::NodeId) {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let states = HashMap::<u64, WidgetState>::new();
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, widget, fonts.clone(), &states);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(160, 40),
        fonts,
        &rutter::render::RichTextRenderer::default(),
    );
    (taffy, root)
}

#[test]
fn counter_constructor_retains_integer_configuration_and_callback() {
    let counter = Widget::counter(
        1,
        -2,
        8,
        2,
        quantity_changed,
        fixed_counter_style(),
        "Quantity",
    )
    .with_widget_id(WidgetId::manual(9).unwrap())
    .unwrap();

    assert!(matches!(
        counter,
        Widget::Counter {
            id: 9,
            config: CounterConfig::Integer(CounterParameters {
                value: 1, min: -2, max: 8, step: 2, on_change,
            }),
            label: "Quantity",
            ..
        } if on_change(3) == Message::QuantityChanged(3)
    ));

    let validated = Widget::try_counter(
        1,
        0,
        3,
        1,
        quantity_changed,
        fixed_counter_style(),
        "Quantity",
    );
    assert!(validated.is_ok());
}

#[test]
fn counter_is_a_keyed_leaf_with_three_pointer_targets() {
    let widget = counter(1);
    let states = HashMap::<u64, WidgetState>::new();
    let (taffy, root) = counter_layout(&widget);

    assert!(taffy.children(root).unwrap().is_empty());
    assert!(matches!(
        hit_test(
            &widget,
            &taffy,
            root,
            Point::new(10.0, 20.0),
            Point::new(0.0, 0.0),
            &states
        ),
        Some(HitResult::CounterAdjust {
            id: 7,
            increment: false,
        })
    ));
    assert!(matches!(
        hit_test(
            &widget,
            &taffy,
            root,
            Point::new(80.0, 20.0),
            Point::new(0.0, 0.0),
            &states
        ),
        Some(HitResult::CounterFocus(7))
    ));
    assert!(matches!(
        hit_test(
            &widget,
            &taffy,
            root,
            Point::new(150.0, 20.0),
            Point::new(0.0, 0.0),
            &states
        ),
        Some(HitResult::CounterAdjust {
            id: 7,
            increment: true,
        })
    ));
}

#[test]
fn automatic_counter_width_grows_with_value_character_count() {
    let single = counter_with_style(1, automatic_counter_style());
    let signed = counter_with_style(-1024, automatic_counter_style());
    let (single_layout, single_root) = counter_layout(&single);
    let (signed_layout, signed_root) = counter_layout(&signed);

    let single_width = single_layout.layout(single_root).unwrap().size.width;
    let signed_width = signed_layout.layout(signed_root).unwrap().size.width;

    assert!(single_width < signed_width);
    assert_eq!(single_width, 78.0);
}

#[test]
fn float_counter_retains_fractional_configuration_and_callback() {
    let widget = Widget::try_counter(
        -0.5,
        -1.0,
        2.0,
        0.125,
        Message::AmountChanged,
        fixed_counter_style(),
        "Amount",
    )
    .unwrap()
    .with_widget_id(WidgetId::manual(9).unwrap())
    .unwrap();

    let Widget::Counter {
        id,
        config:
            CounterConfig::Float(CounterParameters {
                value,
                min,
                max,
                step,
                on_change,
            }),
        label,
        ..
    } = widget
    else {
        panic!("the same Counter constructor must preserve floating-point configuration");
    };
    assert_eq!(id, 9);
    assert_eq!((value, min, max, step), (-0.5, -1.0, 2.0, 0.125));
    assert_eq!(label, "Amount");
    assert_eq!(on_change(-0.375), Message::AmountChanged(-0.375));
}

#[test]
fn float_counter_uses_the_same_three_native_pointer_targets() {
    let widget = Widget::counter(
        0.5,
        0.0,
        1.0,
        0.25,
        Message::AmountChanged,
        fixed_counter_style(),
        "Amount",
    )
    .with_id(7);
    let states = HashMap::new();
    let (taffy, root) = counter_layout(&widget);
    assert!(taffy.children(root).unwrap().is_empty());

    for (x, increment) in [(10.0, Some(false)), (80.0, None), (150.0, Some(true))] {
        let hit = hit_test(
            &widget,
            &taffy,
            root,
            Point::new(x, 20.0),
            Point::new(0.0, 0.0),
            &states,
        );
        match increment {
            Some(expected) => assert!(matches!(hit,
                Some(HitResult::CounterAdjust { id: 7, increment }) if increment == expected)),
            None => assert!(matches!(hit, Some(HitResult::CounterFocus(7)))),
        }
    }

    let disabled = widget.enabled(false);
    assert!(
        hit_test(
            &disabled,
            &taffy,
            root,
            Point::new(150.0, 20.0),
            Point::new(0.0, 0.0),
            &states
        )
        .is_none()
    );
}

#[test]
fn automatic_float_width_accounts_for_decimal_point_and_sign() {
    let widget = |value| {
        Widget::counter(
            value,
            -100.0,
            100.0,
            0.125,
            Message::AmountChanged,
            automatic_counter_style(),
            "Amount",
        )
    };
    let (short, short_root) = counter_layout(&widget(0.5));
    let (long, long_root) = counter_layout(&widget(-12.375));
    assert_eq!(short.layout(short_root).unwrap().size.width, 98.0);
    assert_eq!(long.layout(long_root).unwrap().size.width, 138.0);
}

#[test]
fn unified_constructor_infers_integer_and_float_literals_without_typed_callbacks() {
    let integer: Widget<'_, ()> = Widget::counter(1, 0, 9, 1, |_| (), Style::default(), "Count");
    let floating: Widget<'_, ()> =
        Widget::counter(0.5, 0.0, 1.0, 0.125, |_| (), Style::default(), "Amount");
    assert!(matches!(
        integer,
        Widget::Counter {
            config: CounterConfig::Integer(_),
            ..
        }
    ));
    assert!(matches!(
        floating,
        Widget::Counter {
            config: CounterConfig::Float(_),
            ..
        }
    ));
    let validated: Result<Widget<'_, ()>, rutter::CounterConfigError> =
        Widget::try_counter(1, 0, 9, 1, |_| (), Style::default(), "Count");
    let validated_float: Result<Widget<'_, ()>, rutter::CounterConfigError<f64>> =
        Widget::try_counter(0.5, 0.0, 1.0, 0.125, |_| (), Style::default(), "Amount");
    assert!(validated.is_ok());
    assert!(validated_float.is_ok());
}
