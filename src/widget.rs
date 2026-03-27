// ============================================================
// Rutter Framework — widget.rs  (Fase 4)
//
// Novos widgets:
//   Modal      — overlay com backdrop e conteúdo filho
//   Toast      — notificação temporária (auto-dismiss)
//   TabBar     — navegação horizontal por abas
//   VirtualList— render lazy de listas longas por viewport
//
// Mantidos da Fase 3: todos os anteriores.
// ============================================================

use skia_safe::Color as SkiaColor;
use taffy::prelude::Style;

// ── Tipos auxiliares ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum InputState {
    #[default]
    Idle,
    Focused,
    Error,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Ghost,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

/// Nível semântico do Toast — afeta cor e ícone.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ToastKind {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

// ── Árvore de widgets ─────────────────────────────────────────

pub enum Widget<'a, Msg> {
    // ── Layout ────────────────────────────────────────────────
    Column {
        children: Vec<Widget<'a, Msg>>,
        style: Style,
    },
    Row {
        children: Vec<Widget<'a, Msg>>,
        style: Style,
    },
    Container {
        child: Box<Widget<'a, Msg>>,
        style: Style,
        color: Option<SkiaColor>,
        radius: f32,
    },
    Spacer {
        style: Style,
    },
    Divider {
        style: Style,
        orientation: Orientation,
    },

    // ── Conteúdo ──────────────────────────────────────────────
    Text {
        content: String,
        style: Style,
        color: Option<SkiaColor>,
        size: f32,
    },
    Image {
        data: &'a [u8],
        style: Style,
        radius: f32,
    },

    // ── Ações ─────────────────────────────────────────────────
    Button {
        text: &'a str,
        on_press: Msg,
        style: Style,
        color: Option<SkiaColor>,
        variant: ButtonVariant,
    },

    // ── Entradas ──────────────────────────────────────────────
    TextInput {
        on_change: fn(String) -> Msg,
        on_submit: Option<Msg>,
        style: Style,
        id: u64,
        label: &'a str,
        placeholder: &'a str,
        state: InputState,
        error_msg: Option<String>,
        is_password: bool,
    },
    Checkbox {
        checked: bool,
        on_change: fn(bool) -> Msg,
        label: &'a str,
        style: Style,
    },
    Switch {
        checked: bool,
        on_change: fn(bool) -> Msg,
        style: Style,
    },
    Radio {
        selected: bool,
        on_select: fn() -> Msg,
        label: &'a str,
        style: Style,
    },
    Slider {
        id: u64,
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        on_change: fn(f32) -> Msg,
        style: Style,
        label: &'a str,
    },
    Select {
        id: u64,
        options: &'a [&'a str],
        selected_index: usize,
        on_change: fn(usize) -> Msg,
        style: Style,
        label: &'a str,
        placeholder: &'a str,
    },

    // ── Indicadores ───────────────────────────────────────────
    ProgressBar {
        /// Identificador para animação indeterminada.
        id: u64,
        value: f32,
        indeterminate: bool,
        style: Style,
    },
    Spinner {
        id: u64,
        style: Style,
    },

    // ── Containers ────────────────────────────────────────────
    ScrollView {
        id: u64,
        child: Box<Widget<'a, Msg>>,
        style: Style,
    },
    Tooltip {
        child: Box<Widget<'a, Msg>>,
        text: &'a str,
        style: Style,
    },

    // ── Navegação — Fase 4 ────────────────────────────────────
    /// Barra de abas horizontal.
    /// `tabs` lista os rótulos; `active` é o índice selecionado.
    TabBar {
        id: u64,
        tabs: &'a [&'a str],
        active: usize,
        on_change: fn(usize) -> Msg,
        style: Style,
    },

    // ── Overlay — Fase 4 ──────────────────────────────────────
    /// Overlay com backdrop escuro e conteúdo filho centralizado.
    /// Renderizado por último (acima de tudo).
    Modal {
        /// Índice de identificação para controle de exibição.
        id: u64,
        visible: bool,
        child: Box<Widget<'a, Msg>>,
        /// Mensagem emitida ao clicar no backdrop.
        on_dismiss: Option<Msg>,
        style: Style,
    },

    // ── Notificações — Fase 4 ─────────────────────────────────
    /// Notificação temporária que aparece na parte inferior.
    /// O engine remove-a automaticamente após `duration_ms`.
    Toast {
        id: u64,
        message: &'a str,
        kind: ToastKind,
        /// Duração em ms (0 = permanece até dismiss manual).
        duration_ms: u32,
        on_dismiss: Option<Msg>,
    },

    // ── Lista virtual — Fase 4 ────────────────────────────────
    /// Lista que renderiza apenas os itens visíveis na viewport,
    /// permitindo listas de milhares de itens sem custo de layout.
    VirtualList {
        id: u64,
        item_height: f32,
        item_count: usize,
        /// Renderiza o item no índice `i` como Widget.
        /// NOTA: retorna Option<Box<Widget>> para Fase 4.
        /// Fase 5 migrará para closure via trait object.
        items: &'a dyn Fn(usize) -> Option<String>,
        on_select: fn(usize) -> Msg,
        style: Style,
    },
}
