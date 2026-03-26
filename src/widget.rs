// ============================================================
// Rutter Framework — widget.rs  (Fase 3)
//
// Novos widgets nesta fase:
//   Checkbox, Switch, Radio, Slider, ProgressBar, Spinner,
//   Divider, Spacer, ScrollView, Select, Tooltip, Image
// ============================================================

use skia_safe::Color as SkiaColor;
use taffy::prelude::Style;

// ── Tipos auxiliares ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum InputState { #[default] Idle, Focused, Error, Success }

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ButtonVariant {
    #[default] Primary,
    Ghost,
    Text,
}

/// Orientação para Divider e futuras propriedades direcionais.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Orientation { #[default] Horizontal, Vertical }

// ── Árvore de widgets ─────────────────────────────────────────

pub enum Widget<'a, Msg> {
    // ── Layout ────────────────────────────────────────────────
    Column {
        children: Vec<Widget<'a, Msg>>,
        style:    Style,
    },
    Row {
        children: Vec<Widget<'a, Msg>>,
        style:    Style,
    },
    Container {
        child:  Box<Widget<'a, Msg>>,
        style:  Style,
        color:  Option<SkiaColor>,
        radius: f32,
    },

    // ── Primitivos de layout ──────────────────────────────────
    /// Espaço flexível — equivalente ao Spacer do Flutter.
    Spacer { style: Style },
    /// Linha separadora horizontal ou vertical.
    Divider { style: Style, orientation: Orientation },

    // ── Conteúdo estático ─────────────────────────────────────
    Text {
        content: String,
        style:   Style,
        color:   Option<SkiaColor>,
        size:    f32,
    },
    /// Imagem raster carregada de bytes (PNG/JPEG/WebP).
    /// `data` é o conteúdo bruto do arquivo de imagem.
    Image {
        data:   &'a [u8],
        style:  Style,
        radius: f32,
    },

    // ── Ações ─────────────────────────────────────────────────
    Button {
        text:     &'a str,
        on_press: Msg,
        style:    Style,
        color:    Option<SkiaColor>,
        variant:  ButtonVariant,
    },

    // ── Entradas ──────────────────────────────────────────────
    TextInput {
        on_change:   fn(String) -> Msg,
        on_submit:   Option<Msg>,
        style:       Style,
        id:          u64,
        label:       &'a str,
        placeholder: &'a str,
        state:       InputState,
        error_msg:   Option<String>,
        is_password: bool,
    },

    /// Caixa de seleção booleana.
    Checkbox {
        checked:   bool,
        on_change: fn(bool) -> Msg,
        label:     &'a str,
        style:     Style,
    },

    /// Toggle animado (Switch iOS/Material).
    Switch {
        checked:   bool,
        on_change: fn(bool) -> Msg,
        style:     Style,
    },

    /// Botão de rádio — use vários com o mesmo `group_value` para
    /// construir um grupo de seleção exclusiva.
    Radio {
        selected:   bool,
        on_select:  fn() -> Msg,
        label:      &'a str,
        style:      Style,
    },

    /// Controle deslizante para valores contínuos.
    Slider {
        id:        u64,
        value:     f32,
        min:       f32,
        max:       f32,
        step:      f32,
        on_change: fn(f32) -> Msg,
        style:     Style,
        label:     &'a str,
    },

    // ── Indicadores ───────────────────────────────────────────
    /// Barra de progresso 0.0–1.0. `indeterminate = true` anima.
    ProgressBar {
        value:         f32,
        indeterminate: bool,
        style:         Style,
    },

    /// Spinner de carregamento animado.
    Spinner {
        id:    u64,
        style: Style,
    },

    // ── Navegação e overlay ───────────────────────────────────
    /// Região com scroll vertical. O filho pode ser maior que o container.
    ScrollView {
        id:    u64,
        child: Box<Widget<'a, Msg>>,
        style: Style,
    },

    /// Seletor de opções (dropdown inline).
    /// Quando aberto, expande verticalmente no layout.
    Select {
        id:             u64,
        options:        &'a [&'a str],
        selected_index: usize,
        on_change:      fn(usize) -> Msg,
        style:          Style,
        label:          &'a str,
        placeholder:    &'a str,
    },

    /// Mostra `text` ao passar o mouse sobre o filho.
    Tooltip {
        child: Box<Widget<'a, Msg>>,
        text:  &'a str,
        style: Style,
    },
}
