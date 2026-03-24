// ============================================================
// Rutter Framework — app.rs
// Define o contrato AppLogic que toda aplicação deve
// implementar: como criar o estado inicial, gerar a árvore
// de widgets e processar mensagens.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;

use crate::widget::Widget;

/// Contrato principal do padrão Elm usado pelo Rutter.
///
/// # Exemplo mínimo
/// ```rust
/// struct MyApp;
/// impl AppLogic for MyApp {
///     type State   = MyState;
///     type Message = MyMsg;
///     fn new(fs: &mut FontSystem) -> MyState { ... }
///     fn view<'a>(state: &'a mut MyState) -> Widget<'a, MyMsg> { ... }
///     fn update(state: &mut MyState, msg: MyMsg, _cb: &mut Clipboard) { ... }
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
    fn update(
        state:     &mut Self::State,
        message:   Self::Message,
        clipboard: &mut Clipboard,
    );

    /// Retorna o tema da aplicação.
    fn theme() -> crate::theme::Theme {
        crate::theme::Theme::default()
    }
}
