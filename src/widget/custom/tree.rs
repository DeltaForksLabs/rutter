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

fn collect_custom_child_ids<Message>(children: &[Widget<'_, Message>], ids: &mut Vec<u64>) {
    for child in children {
        collect_custom_widget_ids(child, ids);
    }
}

pub(crate) fn find_custom_widget<'tree, 'widget, Message>(
    widget: &'tree Widget<'widget, Message>,
    target_id: u64,
) -> Option<&'tree (dyn CustomWidgetV1<Message> + 'widget)> {
    match widget {
        Widget::Custom { id, widget, .. } if id.get() == target_id => Some(widget.as_ref()),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            find_custom_in_children(children, target_id)
        }
        Widget::Container { child, .. }
        | Widget::ButtonContent { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::TableOfContents { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => find_custom_widget(child, target_id),
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => find_custom_widget(anchor, target_id).or_else(|| {
            open.then(|| find_custom_widget(content, target_id))
                .flatten()
        }),
        _ => None,
    }
}

fn find_custom_in_children<'tree, 'widget, Message>(
    children: &'tree [Widget<'widget, Message>],
    target_id: u64,
) -> Option<&'tree (dyn CustomWidgetV1<Message> + 'widget)> {
    children
        .iter()
        .find_map(|child| find_custom_widget(child, target_id))
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
