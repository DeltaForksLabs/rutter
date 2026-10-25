use rutter::{HeadingLevel, Widget, spacing};
use taffy::prelude::*;

// A reserved manual ID, stable across reconstruction and independent of demo IDs.
pub(super) const PAGE_SCROLL_ID: u64 = 0x61_0000;
pub(super) const THEME_BUTTON_SIZE: f32 = 44.0;
pub(super) const PAGE_TOP: f32 = spacing::LG + THEME_BUTTON_SIZE + spacing::MD;

/// Normalizes a demo document, keeping viewport overlays and the theme action at
/// the root. Documents retain intrinsic height; short windows scroll the page
/// instead of compressing field bodies. The header action has its own safe row.
pub(super) fn page<'a, Message>(title: &str, view: Widget<'a, Message>) -> Widget<'a, Message> {
    let Widget::Column {
        children,
        mut style,
    } = view
    else {
        panic!("example page must be a column");
    };
    let mut root_children = Vec::new();
    let mut overlays = Vec::new();
    let mut document = Vec::new();
    for child in children {
        if matches!(&child, Widget::ButtonContent { label, .. }
            if label.starts_with("Switch to ") && label.ends_with(" theme"))
        {
            root_children.push(child);
        } else if matches!(
            &child,
            Widget::Modal { .. } | Widget::Dialog { .. } | Widget::Toast { .. }
        ) {
            overlays.push(child);
        } else {
            document.push(child);
        }
    }
    // Promote existing titles without changing their copy; demos that previously
    // started with a control or helper paragraph receive a real page heading.
    match document.first() {
        Some(Widget::Heading {
            level: HeadingLevel::H1,
            ..
        }) => {}
        Some(Widget::Text { size, .. }) if *size >= 18.0 => {
            let Widget::Text { content, .. } = document.remove(0) else {
                unreachable!()
            };
            document.insert(
                0,
                Widget::heading(HeadingLevel::H1, content, Style::default()),
            );
        }
        _ => document.insert(
            0,
            Widget::heading(HeadingLevel::H1, title, Style::default()),
        ),
    }
    if matches!(document.get(1), Some(Widget::Text { size, .. }) if *size <= 16.0) {
        let header = document.drain(..2).collect();
        document.insert(
            0,
            Widget::Column {
                children: header,
                style: Style {
                    gap: Size::length(spacing::SM),
                    flex_shrink: 0.0,
                    ..responsive_width(760.0, Dimension::auto())
                },
            },
        );
    }
    // A demo that already owns a document viewport keeps that stable ID. Its
    // document joins the page flow so the title also remains reachable when short.
    let scroll_id = if document.len() == 2 && matches!(document[1], Widget::ScrollView { .. }) {
        let Widget::ScrollView { id, child, .. } = document.pop().unwrap() else {
            unreachable!()
        };
        document.push(*child);
        id
    } else {
        PAGE_SCROLL_ID
    };
    style.size = Size {
        width: Dimension::percent(1.0),
        height: Dimension::auto(),
    };
    style.padding = Rect::zero();
    style.flex_grow = 0.0;
    style.flex_shrink = 0.0;
    style.justify_content = Some(JustifyContent::FlexStart);
    style.align_items = Some(AlignItems::Stretch);
    style.gap = Size::length(spacing::XL);
    root_children.push(scroll_document(
        scroll_id,
        Widget::Column {
            children: document,
            style,
        },
    ));
    // Root siblings paint forwards and hit-test backwards. Portaled page
    // overlays must follow the document to cover and intercept its controls.
    root_children.extend(overlays);
    Widget::Column {
        children: root_children,
        style: Style {
            size: Size::percent(1.0_f32),
            padding: Rect {
                top: LengthPercentage::length(PAGE_TOP),
                ..Rect::length(spacing::XL)
            },
            ..Style::default()
        },
    }
}

/// A group owns only sibling spacing; its parent owns section separation.
pub(super) fn section<'a, Message>(children: Vec<Widget<'a, Message>>) -> Widget<'a, Message> {
    Widget::Column {
        children,
        style: Style {
            gap: Size::length(spacing::MD),
            flex_shrink: 0.0,
            ..responsive_width(760.0, Dimension::auto())
        },
    }
}

/// Flow rows must leave space between wrapped lines as well as columns.
pub(super) fn flow() -> Style {
    Style {
        flex_wrap: FlexWrap::Wrap,
        align_items: Some(AlignItems::Center),
        gap: Size::length(spacing::MD),
        flex_shrink: 0.0,
        ..Style::default()
    }
}

pub(super) fn scroll_document<'a, Message>(
    id: u64,
    child: Widget<'a, Message>,
) -> Widget<'a, Message> {
    Widget::ScrollView {
        id,
        child: Box::new(child),
        style: Style {
            flex_grow: 1.0,
            align_items: Some(AlignItems::FlexStart),
            min_size: Size::from_lengths(0.0, 0.0),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(0.0),
            },
            ..Style::default()
        },
    }
}

/// Fills the available width on narrow surfaces without exceeding the desktop limit.
pub(super) fn responsive_width(max_width: f32, height: Dimension) -> Style {
    Style {
        flex_shrink: 0.0,
        size: Size {
            width: Dimension::percent(1.0),
            height,
        },
        max_size: Size {
            width: Dimension::length(max_width),
            height: Dimension::auto(),
        },
        ..Style::default()
    }
}

/// Semantic test entry point: page wrappers are not part of demo behavior.
#[cfg(test)]
pub(super) fn document_children<'w, 'a, Message>(
    view: &'w Widget<'a, Message>,
) -> &'w [Widget<'a, Message>] {
    let Widget::Column { children, .. } = view else {
        panic!("expected page root")
    };
    let document = children
        .iter()
        .find_map(|child| match child {
            Widget::ScrollView { child, .. } => Some(child.as_ref()),
            _ => None,
        })
        .expect("expected page scroll viewport");
    let Widget::Column { children, .. } = document else {
        panic!("expected page document")
    };
    children
}
