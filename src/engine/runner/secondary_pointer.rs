// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use winit::dpi::PhysicalPosition;
use winit::window::Window;

use super::RutterRunner;
use crate::app::{
    AppLogic, ContextMenuTarget, LogicalPointerPosition, PhysicalDesktopPosition,
    SecondaryPointerContext,
};
use crate::widget::Widget;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SecondaryPointerBlockers {
    select_popup_open: bool,
    context_menu_open: bool,
    popover_open: bool,
    blocking_overlay_visible: bool,
}

impl SecondaryPointerBlockers {
    pub(super) const fn new(
        select_popup_open: bool,
        context_menu_open: bool,
        popover_open: bool,
        blocking_overlay_visible: bool,
    ) -> Self {
        Self {
            select_popup_open,
            context_menu_open,
            popover_open,
            blocking_overlay_visible,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SecondaryPointerDestination {
    DismissSelect,
    DismissContextMenu,
    DismissPopover,
    ConsumeBlockingOverlay,
    OpenContextMenu(u64),
    DispatchApplication,
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn route_secondary_pointer_press(
        &mut self,
        blockers: SecondaryPointerBlockers,
        context_menu_target: Option<u64>,
    ) {
        let destination = secondary_pointer_destination(blockers, context_menu_target);
        match destination {
            SecondaryPointerDestination::DismissSelect => {
                self.close_all_selects();
                self.dismiss_all_search_suggestions();
            }
            SecondaryPointerDestination::DismissContextMenu => {
                self.engine.close_all_context_menus();
            }
            SecondaryPointerDestination::DismissPopover => {
                self.engine.close_all_popovers();
            }
            SecondaryPointerDestination::ConsumeBlockingOverlay => return,
            SecondaryPointerDestination::OpenContextMenu(id) => {
                self.dispatch_context_menu_open_message(id);
                self.engine
                    .open_context_menu(id, self.engine.last_mouse_pos);
            }
            SecondaryPointerDestination::DispatchApplication => {
                return self.dispatch_secondary_pointer_pressed();
            }
        }
        self.redraw();
    }

    fn dispatch_secondary_pointer_pressed(&mut self) {
        let context = self.resolved_secondary_pointer_context();
        A::secondary_pointer_pressed_with_context(&mut self.engine.app_state, context);
        self.engine.layout_dirty = true;
        self.redraw();
    }

    fn dispatch_context_menu_open_message(&mut self, context_menu_id: u64) {
        let Some(message) = context_menu_open_message::<A>(&self.engine.app_state, context_menu_id)
        else {
            return;
        };
        A::update(
            &mut self.engine.app_state,
            message,
            &mut self.engine.clipboard,
        );
        self.engine.layout_dirty = true;
    }

    fn resolved_secondary_pointer_context(&self) -> SecondaryPointerContext {
        let window = self
            .engine
            .window
            .as_ref()
            .expect("active pointer event requires a committed native window");
        resolve_secondary_pointer_context(
            self.cursor_physical,
            desktop_client_origin(window),
            window.scale_factor(),
        )
    }
}

#[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
fn desktop_client_origin(_: &Window) -> Option<PhysicalPosition<i32>> {
    None
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
fn desktop_client_origin(window: &Window) -> Option<PhysicalPosition<i32>> {
    window.inner_position().ok()
}

fn secondary_pointer_destination(
    blockers: SecondaryPointerBlockers,
    context_menu_target: Option<u64>,
) -> SecondaryPointerDestination {
    if blockers.select_popup_open {
        return SecondaryPointerDestination::DismissSelect;
    }
    if blockers.context_menu_open {
        return SecondaryPointerDestination::DismissContextMenu;
    }
    if blockers.popover_open {
        return SecondaryPointerDestination::DismissPopover;
    }
    if blockers.blocking_overlay_visible {
        return SecondaryPointerDestination::ConsumeBlockingOverlay;
    }
    context_menu_target.map_or(
        SecondaryPointerDestination::DispatchApplication,
        SecondaryPointerDestination::OpenContextMenu,
    )
}

fn context_menu_open_message<A: AppLogic>(
    state: &A::State,
    context_menu_id: u64,
) -> Option<A::Message> {
    A::context_menu_opening(state, ContextMenuTarget::from_resolved_id(context_menu_id))
}

pub(super) fn has_visible_blocking_overlay<Msg>(widget: &Widget<'_, Msg>) -> bool {
    match widget {
        Widget::Modal { visible, .. } | Widget::Dialog { visible, .. } => *visible,
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            children.iter().any(has_visible_blocking_overlay)
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::ButtonContent { child, .. } => has_visible_blocking_overlay(child),
        Widget::Accordion {
            expanded, child, ..
        } => *expanded && has_visible_blocking_overlay(child),
        Widget::Popover {
            open,
            anchor,
            content,
            ..
        } => has_visible_blocking_overlay(anchor) || *open && has_visible_blocking_overlay(content),
        _ => false,
    }
}

pub(super) fn resolve_secondary_pointer_context(
    client_physical: PhysicalPosition<f64>,
    client_origin: Option<PhysicalPosition<i32>>,
    scale_factor: f64,
) -> SecondaryPointerContext {
    let client_position = LogicalPointerPosition::new(
        (client_physical.x / scale_factor) as f32,
        (client_physical.y / scale_factor) as f32,
    );
    let desktop_position = client_origin.map(|origin| {
        PhysicalDesktopPosition::new(
            (f64::from(origin.x) + client_physical.x).round() as i32,
            (f64::from(origin.y) + client_physical.y).round() as i32,
        )
    });
    SecondaryPointerContext::new(client_position, desktop_position, scale_factor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::DialogPosition;

    #[derive(Clone, Debug, PartialEq)]
    enum ContextMenuMessage {
        Select(u64),
    }

    #[derive(Default)]
    struct ContextMenuSelectionState;

    struct ContextMenuSelectionApp;

    impl AppLogic for ContextMenuSelectionApp {
        type State = ContextMenuSelectionState;
        type Message = ContextMenuMessage;

        fn new(_: &mut cosmic_text::FontSystem) -> Self::State {
            ContextMenuSelectionState
        }

        fn view<'a>(_: &'a mut Self::State) -> Widget<'a, Self::Message> {
            Widget::Spacer {
                style: Default::default(),
            }
        }

        fn update(_: &mut Self::State, _: Self::Message, _: &mut arboard::Clipboard) {}

        fn context_menu_opening(
            _: &Self::State,
            target: ContextMenuTarget,
        ) -> Option<Self::Message> {
            Some(ContextMenuMessage::Select(target.id()))
        }
    }

    #[test]
    fn overlay_destination_precedes_context_target_and_application_callback() {
        let context_target = Some(9);
        assert_eq!(
            secondary_pointer_destination(
                SecondaryPointerBlockers::new(true, false, false, false),
                context_target,
            ),
            SecondaryPointerDestination::DismissSelect
        );
        assert_eq!(
            secondary_pointer_destination(
                SecondaryPointerBlockers::new(false, true, false, false),
                context_target,
            ),
            SecondaryPointerDestination::DismissContextMenu
        );
        assert_eq!(
            secondary_pointer_destination(
                SecondaryPointerBlockers::new(false, false, true, false),
                context_target,
            ),
            SecondaryPointerDestination::DismissPopover
        );
        assert_eq!(
            secondary_pointer_destination(
                SecondaryPointerBlockers::new(false, false, false, true),
                context_target,
            ),
            SecondaryPointerDestination::ConsumeBlockingOverlay
        );
    }

    #[test]
    fn context_target_precedes_unclaimed_application_callback() {
        let unblocked = SecondaryPointerBlockers::default();
        assert_eq!(
            secondary_pointer_destination(unblocked, Some(7)),
            SecondaryPointerDestination::OpenContextMenu(7)
        );
        assert_eq!(
            secondary_pointer_destination(unblocked, None),
            SecondaryPointerDestination::DispatchApplication
        );
    }

    #[test]
    fn context_menu_open_message_receives_the_targeted_menu_id() {
        let state = ContextMenuSelectionState;

        let message = context_menu_open_message::<ContextMenuSelectionApp>(&state, 73);

        assert_eq!(message, Some(ContextMenuMessage::Select(73)));
    }

    #[test]
    fn visible_modal_and_dialog_claim_secondary_pointer_events() {
        let spacer = || Widget::Spacer {
            style: Default::default(),
        };
        let modal: Widget<'_, ()> = Widget::Modal {
            id: 1,
            visible: true,
            child: Box::new(spacer()),
            on_dismiss: None,
            style: Default::default(),
        };
        let dialog = Widget::Dialog {
            id: 2,
            title: "Title",
            message: "Message",
            confirm_label: "Confirm",
            cancel_label: "Cancel",
            visible: true,
            on_confirm: (),
            on_cancel: (),
            on_dismiss: None,
            position: DialogPosition::Center,
            style: Default::default(),
            child: Box::new(spacer()),
        };
        assert!(has_visible_blocking_overlay(&modal));
        assert!(has_visible_blocking_overlay(&dialog));
    }

    #[test]
    fn hidden_blocking_overlay_does_not_claim_secondary_pointer_events() {
        let modal: Widget<'_, ()> = Widget::Modal {
            id: 1,
            visible: false,
            child: Box::new(Widget::Spacer {
                style: Default::default(),
            }),
            on_dismiss: None,
            style: Default::default(),
        };
        assert!(!has_visible_blocking_overlay(&modal));
    }

    #[test]
    fn pointer_context_resolves_logical_and_negative_desktop_coordinates() {
        let context = resolve_secondary_pointer_context(
            PhysicalPosition::new(150.0, 75.0),
            Some(PhysicalPosition::new(-1920, 100)),
            1.5,
        );
        assert_eq!(
            context.client_position(),
            LogicalPointerPosition::new(100.0, 50.0)
        );
        assert_eq!(
            context.desktop_position(),
            Some(PhysicalDesktopPosition::new(-1770, 175))
        );
        assert_eq!(context.scale_factor(), 1.5);
    }

    #[test]
    fn unsupported_desktop_origin_preserves_client_context() {
        let context =
            resolve_secondary_pointer_context(PhysicalPosition::new(25.0, 50.0), None, 2.0);
        assert_eq!(
            context.client_position(),
            LogicalPointerPosition::new(12.5, 25.0)
        );
        assert_eq!(context.desktop_position(), None);
        assert_eq!(context.scale_factor(), 2.0);
    }
}
