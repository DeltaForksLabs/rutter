// ============================================================
// Rutter Framework — widget.rs
//
// MUDANÇA FASE 2:
//   Widget::TextInput não carrega mais `&'a mut Editor<'static>`.
//   O editor agora é gerenciado pelo RutterEngine em um
//   HashMap<u64, InputWidgetState>, eliminando todos os
//   problemas de lifetime invariante.
//
//   O widget descreve APENAS o que renderizar; o estado
//   interno (buffers, scroll, undo) pertence ao framework.
// ============================================================

use skia_safe::Color as SkiaColor;
use taffy::prelude::Style;

// ── Estado visual do TextInput ────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum InputState {
    #[default]
    Idle,
    Focused,
    Error,
    Success,
}

// ── Variante do Button ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ButtonVariant {
    /// Preenchido com cor accent — ação principal.
    #[default]
    Primary,
    /// Fundo transparente com borda — ação secundária.
    Ghost,
    /// Sem fundo nem borda — link ou ação terciária.
    Text,
}

// ── Árvore de widgets ─────────────────────────────────────────

/// Nó imutável da árvore de UI, construído por `AppLogic::view`
/// a cada frame que precisar de relayout.
///
/// O lifetime `'a` agora cobre apenas strings emprestadas
/// (label, placeholder, text de botão). Não há mais borrows
/// de `Editor`, o que torna a árvore trivialmente clonável
/// e livre de invariância.
pub enum Widget<'a, Msg> {
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
    Button {
        text:     &'a str,
        on_press: Msg,
        style:    Style,
        color:    Option<SkiaColor>,
        variant:  ButtonVariant,
    },
    Text {
        content: String,
        style:   Style,
        color:   Option<SkiaColor>,
        size:    f32,
    },
    /// Campo de texto — não carrega mais `&mut Editor`.
    /// O engine associa o `id` ao estado interno correspondente.
    TextInput {
        on_change:   fn(String) -> Msg,
        on_submit:   Option<Msg>,
        style:       Style,
        /// Identificador único estável; usado como chave no mapa
        /// de estados internos do engine.
        id:          u64,
        label:       &'a str,
        placeholder: &'a str,
        state:       InputState,
        error_msg:   Option<String>,
        is_password: bool,
    },
}
