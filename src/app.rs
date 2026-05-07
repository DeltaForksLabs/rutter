// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — app.rs
// Define o contrato AppLogic que toda aplicação deve
// implementar: como criar o estado inicial, gerar a árvore
// de widgets e processar mensagens.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;

use crate::i18n::Locale;
use crate::widget::Widget;

/// Contrato principal do padrão Elm usado pelo Rutter.
///
/// # Exemplo mínimo
/// ```rust
/// use arboard::Clipboard;
/// use cosmic_text::FontSystem;
/// use rutter::{AppLogic, Widget};
/// use taffy::prelude::Style;
///
/// #[derive(Debug, Clone)]
/// enum MyMsg {
///     Noop,
/// }
///
/// struct MyState;
/// struct MyApp;
///
/// impl AppLogic for MyApp {
///     type State = MyState;
///     type Message = MyMsg;
///
///     fn new(_fs: &mut FontSystem) -> MyState {
///         MyState
///     }
///
///     fn view<'a>(_state: &'a mut MyState) -> Widget<'a, MyMsg> {
///         Widget::Spacer {
///             style: Style::default(),
///         }
///     }
///
///     fn update(_state: &mut MyState, _msg: MyMsg, _cb: &mut Clipboard) {}
/// }
/// ```
pub trait AppLogic {
    /// Estado interno da aplicação.
    type State;

    /// Mensagens que os widgets podem emitir.
    type Message: Clone + std::fmt::Debug;

    /// Chamado uma vez na inicialização para criar o estado.
    fn new(font_system: &mut FontSystem) -> Self::State;

    /// Produz a árvore de widgets a partir do estado atual.
    /// Chamado a cada frame quando `layout_dirty = true`.
    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Self::Message>;

    /// Processa uma mensagem e muta o estado.
    fn update(state: &mut Self::State, message: Self::Message, clipboard: &mut Clipboard);

    /// Retorna o tema da aplicação.
    fn theme() -> crate::theme::Theme {
        crate::theme::Theme::default()
    }

    /// Retorna o locale usado para direção de layout e catálogos i18n.
    fn locale() -> Locale {
        Locale::default()
    }
}
