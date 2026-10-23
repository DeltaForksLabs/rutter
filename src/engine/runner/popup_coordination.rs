// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Application-wide transient arbitration without exposing menu internals publicly.

use super::*;

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(crate) fn transient_keyboard_modifiers(&self) -> winit::event::Modifiers {
        self.engine.modifiers
    }

    pub(crate) fn set_transient_keyboard_modifiers(&mut self, modifiers: winit::event::Modifiers) {
        self.engine.modifiers = modifiers;
    }

    pub(crate) fn has_transient_menu_request(&mut self) -> Result<bool, RutterRunError> {
        let Some(size) = self
            .engine
            .window
            .as_ref()
            .map(|window| window.surface_size())
        else {
            return Ok(false);
        };
        self.engine.try_ensure_layout_for_input(size)?;
        Ok(self.engine.any_context_menu_open() || self.any_dropdown_menu_open())
    }

    pub(crate) fn dismiss_transient_menus(&mut self) {
        let had_native_menu = self.native_menu.is_some();
        self.native_menu = None;
        self.engine.native_menu_layout = None;
        let contexts_closed = self.engine.close_all_context_menus();
        let dropdowns_closed = self.close_all_dropdown_menus();
        if had_native_menu || contexts_closed || dropdowns_closed {
            self.redraw();
        }
    }
}
