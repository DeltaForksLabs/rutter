// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::{CustomAccessibility, CustomWidgetV1};
use crate::widget::Widget;

pub(crate) fn custom_accessibility_is_valid<Message>(widget: &dyn CustomWidgetV1<Message>) -> bool {
    if widget.interaction() != super::CustomInteraction::VisualOnly {
        return true;
    }
    match widget.accessibility() {
        CustomAccessibility::VisualOnly => true,
        CustomAccessibility::Node(node) => node.actions.is_empty(),
    }
}

pub(crate) fn collect_custom_widget_ids<Message>(widget: &Widget<'_, Message>, ids: &mut Vec<u64>) {
    match widget {
        Widget::Custom { id, .. } => ids.push(id.get()),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            collect_custom_child_ids(children, ids);
        }
        Widget::Container { child, .. }
        | Widget::PointerRegion { child, .. }
        | Widget::ButtonContent { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::TableOfContents { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => collect_custom_widget_ids(child, ids),
        Widget::Popover {
            anchor, content, ..
        } => {
            collect_custom_widget_ids(anchor, ids);
            collect_custom_widget_ids(content, ids);
        }
        _ => {}
    }
}

pub(crate) fn collect_custom_widget_ids_at_path<Message>(
    widget: &Widget<'_, Message>,
    ids: &mut Vec<u64>,
    path: &mut Vec<usize>,
) {
    match widget {
        Widget::Custom { .. } => ids.push(widget.resolved_id(path).unwrap()),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate() {
                path.push(index);
                collect_custom_widget_ids_at_path(child, ids, path);
                path.pop();
            }
        }
        Widget::Container { child, .. }
        | Widget::PointerRegion { child, .. }
        | Widget::ButtonContent { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::TableOfContents { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            collect_custom_widget_ids_at_path(child, ids, path);
            path.pop();
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            path.push(0);
            collect_custom_widget_ids_at_path(anchor, ids, path);
            path.pop();
            if *open {
                path.push(1);
                collect_custom_widget_ids_at_path(content, ids, path);
                path.pop();
            }
        }
        _ => {}
    }
}

fn collect_custom_child_ids<Message>(children: &[Widget<'_, Message>], ids: &mut Vec<u64>) {
    for child in children {
        collect_custom_widget_ids(child, ids);
    }
}

pub(crate) fn find_custom_widget<'tree, 'widget, Message>(
    widget: &'tree Widget<'widget, Message>,
    target_id: u64,
) -> Option<&'tree (dyn CustomWidgetV1<Message> + 'widget)> {
    find_custom_widget_at_path(widget, target_id, &mut Vec::new())
}

fn find_custom_widget_at_path<'tree, 'widget, Message>(
    widget: &'tree Widget<'widget, Message>,
    target_id: u64,
    path: &mut Vec<usize>,
) -> Option<&'tree (dyn CustomWidgetV1<Message> + 'widget)> {
    match widget {
        Widget::Custom { widget: custom, .. } if widget.resolved_id(path) == Some(target_id) => {
            Some(custom.as_ref())
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            find_custom_in_children_at_path(children, target_id, path)
        }
        Widget::Container { child, .. }
        | Widget::PointerRegion { child, .. }
        | Widget::ButtonContent { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::TableOfContents { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => find_custom_single_child(child, target_id, path),
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => find_custom_single_child(anchor, target_id, path).or_else(|| {
            open.then(|| find_custom_single_child(content, target_id, path))
                .flatten()
        }),
        _ => None,
    }
}

fn find_custom_in_children_at_path<'tree, 'widget, Message>(
    children: &'tree [Widget<'widget, Message>],
    target_id: u64,
    path: &mut Vec<usize>,
) -> Option<&'tree (dyn CustomWidgetV1<Message> + 'widget)> {
    for (index, child) in children.iter().enumerate() {
        path.push(index);
        let result = find_custom_widget_at_path(child, target_id, path);
        path.pop();
        if result.is_some() {
            return result;
        }
    }
    None
}

fn find_custom_single_child<'tree, 'widget, Message>(
    child: &'tree Widget<'widget, Message>,
    target_id: u64,
    path: &mut Vec<usize>,
) -> Option<&'tree (dyn CustomWidgetV1<Message> + 'widget)> {
    path.push(0);
    let result = find_custom_widget_at_path(child, target_id, path);
    path.pop();
    result
}

#[cfg(test)]
mod tests {
    use taffy::prelude::Style;

    use super::*;
    use crate::{
        CustomAccessibilityActions, CustomAccessibilityNode, CustomAccessibilityRole,
        CustomAccessibilityState, CustomPaintContext, WidgetId, WidgetIdError, WidgetIdSnapshot,
    };
    use crate::{CustomWidgetState, CustomWidgetStateError, MAX_CUSTOM_WIDGET_STATE_BYTES};

    struct PassiveCustom;

    impl CustomWidgetV1<()> for PassiveCustom {
        fn paint(&self, _: CustomPaintContext<'_>) {}
    }

    #[test]
    fn custom_state_rejects_bytes_over_the_runtime_limit() {
        let mut state = CustomWidgetState::default();
        let oversized = vec![0; MAX_CUSTOM_WIDGET_STATE_BYTES + 1];

        assert!(matches!(
            state.replace_bytes(&oversized),
            Err(CustomWidgetStateError::BytesExceeded { .. })
        ));
    }

    #[test]
    fn custom_state_replaces_and_clears_retained_bytes() {
        let mut state = CustomWidgetState::default();
        state.replace_bytes(&[2, 4]).unwrap();
        state.clear();

        assert!(state.bytes().is_empty());
    }

    #[test]
    fn visual_only_custom_node_rejects_accessibility_actions() {
        let widget = Widget::custom(
            WidgetId::manual(73).unwrap(),
            InvalidVisualOnly,
            Style::default(),
        );

        assert!(!custom_accessibility_is_valid(&InvalidVisualOnly));
        assert!(matches!(
            WidgetIdSnapshot::capture(&widget),
            Err(WidgetIdError::InvalidCustomAccessibility { value: 73 })
        ));
    }

    #[test]
    fn manual_custom_id_survives_tree_reordering() {
        let id = WidgetId::manual(91).unwrap();
        let first = custom_tree_with_spacer(id, true);
        let second = custom_tree_with_spacer(id, false);
        let mut first_ids = Vec::new();
        let mut second_ids = Vec::new();
        collect_custom_widget_ids(&first, &mut first_ids);
        collect_custom_widget_ids(&second, &mut second_ids);

        assert_eq!(first_ids, vec![id.get()]);
        assert_eq!(second_ids, vec![id.get()]);
        assert!(find_custom_widget(&second, id.get()).is_some());
    }

    struct InvalidVisualOnly;

    impl CustomWidgetV1<()> for InvalidVisualOnly {
        fn paint(&self, _: CustomPaintContext<'_>) {}

        fn accessibility(&self) -> CustomAccessibility {
            CustomAccessibility::Node(CustomAccessibilityNode {
                role: CustomAccessibilityRole::Button,
                name: String::from("Invalid"),
                value: None,
                state: CustomAccessibilityState::Default,
                actions: CustomAccessibilityActions {
                    click: true,
                    ..Default::default()
                },
            })
        }
    }

    fn custom_tree_with_spacer(id: WidgetId, custom_first: bool) -> Widget<'static, ()> {
        let custom = Widget::custom(id, PassiveCustom, Style::default());
        let spacer = Widget::Spacer {
            style: Style::default(),
        };
        let children = if custom_first {
            vec![custom, spacer]
        } else {
            vec![spacer, custom]
        };
        Widget::Column {
            children,
            style: Style::default(),
        }
    }
}
