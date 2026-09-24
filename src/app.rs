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
use crate::input_limits::{InputKind, InputLimits};
use crate::pointer::SelectedTextDrag;
use crate::render::text::TextShapeCacheLimits;
use crate::widget::Widget;

mod shortcut;

pub(crate) use shortcut::shortcut_event_from_winit;
pub use shortcut::{ShortcutEvent, ShortcutKey, ShortcutNamedKey, ShortcutOutcome};

/// Logical client coordinates for a pointer event dispatched by Rutter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalPointerPosition {
    x: f32,
    y: f32,
}

impl LogicalPointerPosition {
    /// Creates a position in logical pixels relative to the surface top-left corner.
    ///
    /// ```rust
    /// let position = rutter::LogicalPointerPosition::new(12.0, 24.0);
    /// assert_eq!((position.x(), position.y()), (12.0, 24.0));
    /// ```
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Returns the logical horizontal coordinate.
    ///
    /// ```rust
    /// assert_eq!(rutter::LogicalPointerPosition::new(7.0, 9.0).x(), 7.0);
    /// ```
    pub const fn x(self) -> f32 {
        self.x
    }

    /// Returns the logical vertical coordinate.
    ///
    /// ```rust
    /// assert_eq!(rutter::LogicalPointerPosition::new(7.0, 9.0).y(), 9.0);
    /// ```
    pub const fn y(self) -> f32 {
        self.y
    }
}

/// Physical desktop coordinates suitable for positioning a native popup window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalDesktopPosition {
    x: i32,
    y: i32,
}

impl PhysicalDesktopPosition {
    /// Creates a physical desktop position, including negative multi-monitor coordinates.
    ///
    /// ```rust
    /// let position = rutter::PhysicalDesktopPosition::new(-1600, 240);
    /// assert_eq!((position.x(), position.y()), (-1600, 240));
    /// ```
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Returns the physical horizontal desktop coordinate.
    ///
    /// ```rust
    /// assert_eq!(rutter::PhysicalDesktopPosition::new(12, 24).x(), 12);
    /// ```
    pub const fn x(self) -> i32 {
        self.x
    }

    /// Returns the physical vertical desktop coordinate.
    ///
    /// ```rust
    /// assert_eq!(rutter::PhysicalDesktopPosition::new(12, 24).y(), 24);
    /// ```
    pub const fn y(self) -> i32 {
        self.y
    }
}

/// Coordinates and scale resolved for an unclaimed secondary-button press.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SecondaryPointerContext {
    client_position: LogicalPointerPosition,
    desktop_position: Option<PhysicalDesktopPosition>,
    scale_factor: f64,
}

impl SecondaryPointerContext {
    /// Creates pointer context from logical client and optional physical desktop coordinates.
    ///
    /// ```rust
    /// use rutter::{LogicalPointerPosition, PhysicalDesktopPosition, SecondaryPointerContext};
    /// let context = SecondaryPointerContext::new(
    ///     LogicalPointerPosition::new(10.0, 20.0),
    ///     Some(PhysicalDesktopPosition::new(110, 220)),
    ///     2.0,
    /// );
    /// assert_eq!(context.scale_factor(), 2.0);
    /// ```
    pub const fn new(
        client_position: LogicalPointerPosition,
        desktop_position: Option<PhysicalDesktopPosition>,
        scale_factor: f64,
    ) -> Self {
        Self {
            client_position,
            desktop_position,
            scale_factor,
        }
    }

    /// Returns logical coordinates relative to the source surface client area.
    ///
    /// ```rust
    /// use rutter::{LogicalPointerPosition, SecondaryPointerContext};
    /// let context = SecondaryPointerContext::new(
    ///     LogicalPointerPosition::new(10.0, 20.0), None, 1.0,
    /// );
    /// assert_eq!(context.client_position().x(), 10.0);
    /// ```
    pub const fn client_position(self) -> LogicalPointerPosition {
        self.client_position
    }

    /// Returns physical desktop coordinates when the window system exposes its client origin.
    ///
    /// Wayland and Android normally return `None` because absolute top-level positions are not
    /// available through Winit.
    ///
    /// ```rust
    /// use rutter::{LogicalPointerPosition, SecondaryPointerContext};
    /// let context = SecondaryPointerContext::new(
    ///     LogicalPointerPosition::new(10.0, 20.0), None, 1.0,
    /// );
    /// assert!(context.desktop_position().is_none());
    /// ```
    pub const fn desktop_position(self) -> Option<PhysicalDesktopPosition> {
        self.desktop_position
    }

    /// Returns the physical-pixels-per-logical-pixel scale of the source surface.
    ///
    /// ```rust
    /// use rutter::{LogicalPointerPosition, SecondaryPointerContext};
    /// let context = SecondaryPointerContext::new(
    ///     LogicalPointerPosition::new(10.0, 20.0), None, 1.5,
    /// );
    /// assert_eq!(context.scale_factor(), 1.5);
    /// ```
    pub const fn scale_factor(self) -> f64 {
        self.scale_factor
    }
}

/// Identifies a virtual collection item beneath a context-menu secondary-button press.
///
/// `collection_id` is the runtime-resolved ID of the virtual collection and
/// `index` identifies the row or cell in its current item sequence.
///
/// ```rust
/// use rutter::ContextMenuVirtualItem;
///
/// let item = ContextMenuVirtualItem::List {
///     collection_id: 12,
///     index: 3,
/// };
/// assert!(matches!(item, ContextMenuVirtualItem::List { index: 3, .. }));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContextMenuVirtualItem {
    /// A row in a virtual list.
    List { collection_id: u64, index: usize },
    /// A cell in a virtual grid.
    Grid { collection_id: u64, index: usize },
}

/// Identifies the context-menu widget targeted by a secondary-button press.
///
/// The runtime resolves automatic widget IDs before invoking the application
/// callback. Assign a stable manual ID to a context menu when application
/// state must map this target to a domain item. A menu wrapping a virtual
/// collection also exposes the pressed row or cell through [`Self::virtual_item`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContextMenuTarget {
    id: u64,
    virtual_item: Option<ContextMenuVirtualItem>,
}

impl ContextMenuTarget {
    pub(crate) const fn from_resolved_id(id: u64) -> Self {
        Self {
            id,
            virtual_item: None,
        }
    }

    pub(crate) const fn from_virtual_item(id: u64, virtual_item: ContextMenuVirtualItem) -> Self {
        Self {
            id,
            virtual_item: Some(virtual_item),
        }
    }

    /// Returns the runtime-resolved ID of the targeted context-menu widget.
    ///
    /// ```rust
    /// use rutter::ContextMenuTarget;
    ///
    /// fn selected_target_id(target: ContextMenuTarget) -> u64 {
    ///     target.id()
    /// }
    /// ```
    pub const fn id(self) -> u64 {
        self.id
    }

    /// Returns the virtual row or cell under the press when the menu wraps a virtual collection.
    ///
    /// ```rust
    /// use rutter::{ContextMenuTarget, ContextMenuVirtualItem};
    ///
    /// fn selected_index(target: ContextMenuTarget) -> Option<usize> {
    ///     match target.virtual_item() {
    ///         Some(ContextMenuVirtualItem::List { index, .. }) => Some(index),
    ///         Some(ContextMenuVirtualItem::Grid { index, .. }) => Some(index),
    ///         None => None,
    ///     }
    /// }
    /// ```
    pub const fn virtual_item(self) -> Option<ContextMenuVirtualItem> {
        self.virtual_item
    }
}

/// Configures the compositor-facing top-level drawing surface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SurfaceConfig {
    transparent: bool,
}

impl SurfaceConfig {
    /// Requests a top-level surface whose alpha channel is presented by the compositor.
    ///
    /// ```rust
    /// use rutter::SurfaceConfig;
    ///
    /// let config = SurfaceConfig::transparent();
    /// assert!(config.is_transparent());
    /// ```
    pub const fn transparent() -> Self {
        Self { transparent: true }
    }

    /// Reports whether compositor transparency was requested.
    ///
    /// ```rust
    /// use rutter::SurfaceConfig;
    ///
    /// assert!(SurfaceConfig::transparent().is_transparent());
    /// ```
    pub const fn is_transparent(self) -> bool {
        self.transparent
    }
}

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
    ///
    /// O runtime pode chamar este método várias vezes no mesmo ciclo para estado,
    /// layout, interação e desenho. A implementação deve ser determinística e não
    /// deve produzir efeitos colaterais.
    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Self::Message>;

    /// Processa uma mensagem e muta o estado.
    fn update(state: &mut Self::State, message: Self::Message, clipboard: &mut Clipboard);

    /// Observes a secondary-button press not claimed by an in-surface context menu.
    ///
    /// Applications can use this source event to create a platform-owned popup
    /// instead of rendering an overlay that is clipped to the source surface.
    fn secondary_pointer_pressed(_state: &mut Self::State, _position: LogicalPointerPosition) {}

    /// Observes an unclaimed secondary-button press with native positioning context.
    ///
    /// The default forwards logical client coordinates to [`Self::secondary_pointer_pressed`],
    /// preserving implementations of the original callback.
    fn secondary_pointer_pressed_with_context(
        state: &mut Self::State,
        context: SecondaryPointerContext,
    ) {
        Self::secondary_pointer_pressed(state, context.client_position());
    }

    /// Produces a message to process before an in-surface context menu opens.
    ///
    /// Return a selection message for the targeted menu to keep selection transitions in
    /// [`Self::update`]. The runtime dispatches that message before rendering the menu overlay.
    /// Use a stable manual context-menu ID when the target must map to application-owned data.
    /// [`ContextMenuTarget::virtual_item`] identifies the pressed row or cell when a menu wraps
    /// a virtual list or grid.
    ///
    /// ```rust
    /// use rutter::{ContextMenuTarget, ContextMenuVirtualItem};
    ///
    /// #[derive(Clone, Debug, PartialEq)]
    /// enum FileMessage { Select(usize) }
    ///
    /// fn context_menu_message(target: ContextMenuTarget) -> Option<FileMessage> {
    ///     match target.virtual_item() {
    ///         Some(ContextMenuVirtualItem::List { index, .. }) => Some(FileMessage::Select(index)),
    ///         _ => None,
    ///     }
    /// }
    /// ```
    fn context_menu_opening(
        _state: &Self::State,
        _target: ContextMenuTarget,
    ) -> Option<Self::Message> {
        None
    }

    /// Matches a pressed key delivered to the focused Rutter surface.
    ///
    /// Return [`ShortcutOutcome::Message`] to dispatch a typed message through
    /// [`Self::update`], [`ShortcutOutcome::Consumed`] to suppress the event without a message,
    /// or [`ShortcutOutcome::Ignored`] to preserve text input, focus traversal, and built-in
    /// widget navigation. Plain printable key events in a focused Rutter input, including Shift
    /// variants, and IME commits bypass this hook so composition cannot become an accidental
    /// shortcut. Modified character chords and named keys are offered before toolkit key routing.
    ///
    /// ```rust
    /// use rutter::{AppLogic, ShortcutEvent, ShortcutKey, ShortcutOutcome};
    ///
    /// #[derive(Clone, Debug)]
    /// enum Message { OpenLauncher }
    ///
    /// fn shortcut(event: ShortcutEvent) -> ShortcutOutcome<Message> {
    ///     match (&event.key, event.control, event.repeat) {
    ///         (ShortcutKey::Character(key), true, false) if key.eq_ignore_ascii_case("p") => {
    ///             ShortcutOutcome::Message(Message::OpenLauncher)
    ///         }
    ///         _ => ShortcutOutcome::Ignored,
    ///     }
    /// }
    ///
    /// let _ = shortcut;
    /// ```
    fn shortcut(_state: &Self::State, _event: ShortcutEvent) -> ShortcutOutcome<Self::Message> {
        ShortcutOutcome::Ignored
    }

    /// Retorna o tema da aplicação.
    fn theme() -> crate::theme::Theme {
        crate::theme::Theme::default()
    }

    /// Resolves the application theme from its current state.
    ///
    /// The default delegates to [`Self::theme`], so existing applications remain compatible.
    ///
    /// ```rust
    /// use rutter::{AppLogic, Theme};
    ///
    /// fn active_theme<A: AppLogic>(state: &A::State) -> Theme {
    ///     A::theme_for(state)
    /// }
    /// ```
    fn theme_for(_state: &Self::State) -> crate::theme::Theme {
        Self::theme()
    }

    /// Retorna o locale usado para direção de layout e catálogos i18n.
    fn locale() -> Locale {
        Locale::default()
    }

    /// Returns limits for one resolved input without changing widget declarations.
    ///
    /// ```
    /// use rutter::{InputKind, InputLimits};
    ///
    /// let limits = InputLimits::for_kind(InputKind::SearchBar);
    /// assert_eq!(limits.max_lines, 1);
    /// ```
    fn input_limits(_id: u64, kind: InputKind) -> InputLimits {
        InputLimits::for_kind(kind)
    }

    /// Enables dragging an existing highlighted selection from one resolved text-input ID.
    ///
    /// By default inputs keep normal click, double-click, and drag-to-select behavior. An
    /// opted-in, non-password input starts a drag only when a subsequent press falls *inside*
    /// its nonempty selection and moves at least five logical pixels. A click without dragging
    /// clears the selection and places the caret at the pressed position. The selected substring
    /// is passed to `on_selected` before the drag starts; the application keeps the snapshot for
    /// its drop callback. Selections that cannot be shown in a bounded text badge keep normal
    /// editing behavior instead.
    fn selected_text_drag(
        _state: &Self::State,
        _id: u64,
    ) -> Option<SelectedTextDrag<Self::Message>> {
        None
    }

    /// Returns the shaping-cache budget used by the runtime.
    ///
    /// ```
    /// # use rutter::{AppLogic, TextShapeCacheLimits};
    /// let limits = TextShapeCacheLimits::default();
    /// assert!(limits.max_total_text_bytes > 0);
    /// ```
    fn text_shape_cache_limits() -> TextShapeCacheLimits {
        TextShapeCacheLimits::default()
    }

    /// Returns startup-only options for the top-level presentation surface.
    ///
    /// ```rust
    /// use rutter::SurfaceConfig;
    ///
    /// let config = SurfaceConfig::default();
    /// assert!(!config.is_transparent());
    /// ```
    fn surface_config() -> SurfaceConfig {
        SurfaceConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct LegacyPointerState(Option<LogicalPointerPosition>);

    struct LegacyPointerApp;

    impl AppLogic for LegacyPointerApp {
        type State = LegacyPointerState;
        type Message = ();

        fn new(_: &mut FontSystem) -> Self::State {
            LegacyPointerState::default()
        }

        fn view<'a>(_: &'a mut Self::State) -> Widget<'a, Self::Message> {
            Widget::Spacer {
                style: Default::default(),
            }
        }

        fn update(_: &mut Self::State, _: Self::Message, _: &mut Clipboard) {}

        fn secondary_pointer_pressed(state: &mut Self::State, position: LogicalPointerPosition) {
            state.0 = Some(position);
        }
    }

    #[test]
    fn contextual_callback_preserves_legacy_logical_callback() {
        let mut state = LegacyPointerState::default();
        let logical = LogicalPointerPosition::new(12.0, 24.0);
        let context = SecondaryPointerContext::new(logical, None, 1.5);

        LegacyPointerApp::secondary_pointer_pressed_with_context(&mut state, context);

        assert_eq!(state.0, Some(logical));
    }

    #[test]
    fn context_menu_target_preserves_the_resolved_menu_id() {
        let target = ContextMenuTarget::from_resolved_id(91);

        assert_eq!(target.id(), 91);
        assert_eq!(target.virtual_item(), None);
    }

    #[test]
    fn context_menu_target_exposes_the_virtual_list_item() {
        let item = ContextMenuVirtualItem::List {
            collection_id: 92,
            index: 3,
        };
        let target = ContextMenuTarget::from_virtual_item(91, item);

        assert_eq!(target.id(), 91);
        assert_eq!(target.virtual_item(), Some(item));
    }

    #[test]
    fn default_context_menu_opening_emits_no_message() {
        let state = LegacyPointerState::default();
        let target = ContextMenuTarget::from_resolved_id(91);

        assert_eq!(LegacyPointerApp::context_menu_opening(&state, target), None);
    }

    #[test]
    fn default_shortcut_hook_ignores_the_event() {
        let state = LegacyPointerState::default();
        let event = ShortcutEvent {
            key: ShortcutKey::Character("p".into()),
            control: true,
            alt: false,
            shift: false,
            super_key: false,
            repeat: false,
        };

        assert_eq!(
            LegacyPointerApp::shortcut(&state, event),
            ShortcutOutcome::Ignored
        );
    }
}
