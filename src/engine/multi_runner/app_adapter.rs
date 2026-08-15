// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::marker::PhantomData;

use arboard::Clipboard;
use cosmic_text::FontSystem;

use crate::app::{AppLogic, LogicalPointerPosition, SecondaryPointerContext};
use crate::i18n::Locale;
use crate::input_limits::{InputKind, InputLimits};
use crate::multi_window::{MultiWindowAppLogic, SurfaceCommand, SurfaceId};
use crate::render::text::TextShapeCacheLimits;
use crate::theme::Theme;
use crate::widget::Widget;

pub(super) struct SurfaceAppState<A: MultiWindowAppLogic> {
    pub(super) surface: SurfaceId,
    pub(super) model: A::State,
    pub(super) revision: u128,
    pub(super) commands: Vec<SurfaceCommand>,
}

impl<A: MultiWindowAppLogic> SurfaceAppState<A> {
    pub(super) fn new(surface: SurfaceId, model: A::State, revision: u128) -> Self {
        Self {
            surface,
            model,
            revision,
            commands: Vec::new(),
        }
    }
}

pub(super) struct SurfaceAppAdapter<A: MultiWindowAppLogic>(PhantomData<A>);

impl<A: MultiWindowAppLogic> AppLogic for SurfaceAppAdapter<A> {
    type State = SurfaceAppState<A>;
    type Message = A::Message;

    fn new(_: &mut FontSystem) -> Self::State {
        panic!(
            "SurfaceAppAdapter has no logical SurfaceId; expected MultiWindowRunner initialization"
        )
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Self::Message> {
        A::view(&mut state.model, state.surface)
    }

    fn update(state: &mut Self::State, message: Self::Message, clipboard: &mut Clipboard) {
        let commands = A::update(&mut state.model, state.surface, message, clipboard);
        state.revision += 1;
        state.commands.extend(commands);
    }

    fn secondary_pointer_pressed(state: &mut Self::State, position: LogicalPointerPosition) {
        let commands = A::secondary_pointer_pressed(&mut state.model, state.surface, position);
        retain_secondary_pointer_commands(state, commands);
    }

    fn secondary_pointer_pressed_with_context(
        state: &mut Self::State,
        context: SecondaryPointerContext,
    ) {
        let commands =
            A::secondary_pointer_pressed_with_context(&mut state.model, state.surface, context);
        retain_secondary_pointer_commands(state, commands);
    }

    fn theme() -> Theme {
        A::theme()
    }

    fn theme_for(state: &Self::State) -> Theme {
        A::theme_for(&state.model)
    }

    fn locale() -> Locale {
        A::locale()
    }

    fn input_limits(id: u64, kind: InputKind) -> InputLimits {
        A::input_limits(id, kind)
    }

    fn text_shape_cache_limits() -> TextShapeCacheLimits {
        A::text_shape_cache_limits()
    }
}

fn retain_secondary_pointer_commands<A: MultiWindowAppLogic>(
    state: &mut SurfaceAppState<A>,
    commands: Vec<SurfaceCommand>,
) {
    state.revision += 1;
    state.commands.extend(commands);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer_test_view<'a, State>(_: &'a mut State, _: SurfaceId) -> Widget<'a, ()> {
        Widget::Spacer {
            style: Default::default(),
        }
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    struct PointerState(Option<(SurfaceId, SecondaryPointerContext)>);

    struct PointerApp;

    impl MultiWindowAppLogic for PointerApp {
        type State = PointerState;
        type Message = ();

        fn new(_: &mut FontSystem) -> Self::State {
            PointerState::default()
        }

        fn view<'a>(state: &'a mut Self::State, surface: SurfaceId) -> Widget<'a, Self::Message> {
            pointer_test_view(state, surface)
        }

        fn update(
            _: &mut Self::State,
            _: SurfaceId,
            _: Self::Message,
            _: &mut Clipboard,
        ) -> Vec<SurfaceCommand> {
            Vec::new()
        }

        fn secondary_pointer_pressed_with_context(
            state: &mut Self::State,
            surface: SurfaceId,
            context: SecondaryPointerContext,
        ) -> Vec<SurfaceCommand> {
            state.0 = Some((surface, context));
            vec![SurfaceCommand::RequestRedraw(surface)]
        }
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    struct LegacyPointerState(Option<(SurfaceId, LogicalPointerPosition)>);

    struct LegacyPointerApp;

    impl MultiWindowAppLogic for LegacyPointerApp {
        type State = LegacyPointerState;
        type Message = ();

        fn new(_: &mut FontSystem) -> Self::State {
            LegacyPointerState::default()
        }

        fn view<'a>(state: &'a mut Self::State, surface: SurfaceId) -> Widget<'a, Self::Message> {
            pointer_test_view(state, surface)
        }

        fn update(
            _: &mut Self::State,
            _: SurfaceId,
            _: Self::Message,
            _: &mut Clipboard,
        ) -> Vec<SurfaceCommand> {
            Vec::new()
        }

        fn secondary_pointer_pressed(
            state: &mut Self::State,
            surface: SurfaceId,
            position: LogicalPointerPosition,
        ) -> Vec<SurfaceCommand> {
            state.0 = Some((surface, position));
            vec![SurfaceCommand::RequestRedraw(surface)]
        }
    }

    #[test]
    fn secondary_pointer_event_retains_source_position_and_commands() {
        let surface = SurfaceId::new(7);
        let mut state = SurfaceAppState::new(surface, PointerState::default(), 3);
        let context = SecondaryPointerContext::new(
            LogicalPointerPosition::new(12.0, 24.0),
            Some(crate::app::PhysicalDesktopPosition::new(112, 224)),
            2.0,
        );

        SurfaceAppAdapter::<PointerApp>::secondary_pointer_pressed_with_context(
            &mut state, context,
        );

        assert_eq!(state.model.0, Some((surface, context)));
        assert_eq!(state.revision, 4);
        assert_eq!(state.commands, [SurfaceCommand::RequestRedraw(surface)]);
    }

    #[test]
    fn contextual_adapter_preserves_legacy_multi_window_callback() {
        let surface = SurfaceId::new(8);
        let logical = LogicalPointerPosition::new(16.0, 32.0);
        let context = SecondaryPointerContext::new(logical, None, 1.5);
        let mut state = SurfaceAppState::new(surface, LegacyPointerState::default(), 4);

        SurfaceAppAdapter::<LegacyPointerApp>::secondary_pointer_pressed_with_context(
            &mut state, context,
        );

        assert_eq!(state.model.0, Some((surface, logical)));
        assert_eq!(state.revision, 5);
        assert_eq!(state.commands, [SurfaceCommand::RequestRedraw(surface)]);
    }
}
