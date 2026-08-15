// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::marker::PhantomData;

use arboard::Clipboard;
use cosmic_text::FontSystem;

use crate::app::{AppLogic, LogicalPointerPosition};
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
        state.revision += 1;
        state.commands.extend(commands);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Default, PartialEq)]
    struct PointerState(Option<(SurfaceId, LogicalPointerPosition)>);

    struct PointerApp;

    impl MultiWindowAppLogic for PointerApp {
        type State = PointerState;
        type Message = ();

        fn new(_: &mut FontSystem) -> Self::State {
            PointerState::default()
        }

        fn view<'a>(_: &'a mut Self::State, _: SurfaceId) -> Widget<'a, Self::Message> {
            Widget::Spacer {
                style: Default::default(),
            }
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
        let position = LogicalPointerPosition::new(12.0, 24.0);

        SurfaceAppAdapter::<PointerApp>::secondary_pointer_pressed(&mut state, position);

        assert_eq!(state.model.0, Some((surface, position)));
        assert_eq!(state.revision, 4);
        assert_eq!(state.commands, [SurfaceCommand::RequestRedraw(surface)]);
    }
}
