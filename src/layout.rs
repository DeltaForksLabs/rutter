// ============================================================
// Rutter Framework — layout.rs
// Integração com Taffy (flexbox) e medição de texto via
// cosmic-text. Responsável por:
//   1. Converter a Widget tree em nós Taffy
//   2. Rodar o algoritmo de layout com medição de texto
// ============================================================

use std::cell::RefCell;
use std::rc::Rc;

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};
use taffy::prelude::*;
use winit::dpi::PhysicalSize;

use crate::widget::Widget;

// ── Contexto armazenado nos nós Taffy ────────────────────────

/// Dados anexados a nós de texto para medição durante layout.
#[derive(Debug, Clone)]
pub struct TextContext {
    pub content:   String,
    pub font_size: f32,
}

/// Contexto genérico do Rutter para nós Taffy.
#[derive(Debug, Clone, Default)]
pub enum RutterContext {
    #[default]
    None,
    Text(TextContext),
}

// ── Construção da árvore Taffy ───────────────────────────────

/// Percorre a `Widget` tree recursivamente e cria nós Taffy
/// equivalentes, preservando a hierarquia de layout.
pub fn build_taffy_tree<'a, Msg>(
    taffy:  &mut TaffyTree<RutterContext>,
    widget: &Widget<'a, Msg>,
    fs:     Rc<RefCell<FontSystem>>,
) -> NodeId {
    match widget {
        Widget::Column { children, style } => {
            let s = Style {
                flex_direction: FlexDirection::Column,
                ..style.clone()
            };
            let ids: Vec<_> = children
                .iter()
                .map(|c| build_taffy_tree(taffy, c, fs.clone()))
                .collect();
            taffy.new_with_children(s, &ids).unwrap()
        }

        Widget::Row { children, style } => {
            let s = Style {
                flex_direction: FlexDirection::Row,
                ..style.clone()
            };
            let ids: Vec<_> = children
                .iter()
                .map(|c| build_taffy_tree(taffy, c, fs.clone()))
                .collect();
            taffy.new_with_children(s, &ids).unwrap()
        }

        Widget::Container { child, style, .. } => {
            let id = build_taffy_tree(taffy, child, fs.clone());
            taffy.new_with_children(style.clone(), &[id]).unwrap()
        }

        // Folhas simples — o Style determina o tamanho fixo/flex
        Widget::Button { style, .. } | Widget::TextInput { style, .. } => {
            taffy.new_leaf(style.clone()).unwrap()
        }

        // Texto precisa de contexto para medição intrinsic
        Widget::Text { content, style, size, .. } => {
            let ctx = RutterContext::Text(TextContext {
                content:   content.clone(),
                font_size: *size,
            });
            taffy.new_leaf_with_context(style.clone(), ctx).unwrap()
        }
    }
}

// ── Cálculo de layout ────────────────────────────────────────

/// Executa o algoritmo de layout Taffy com measure callback para
/// nós de Texto, calculando dimensões intrinsic via cosmic-text.
pub fn compute_layout(
    taffy:  &mut TaffyTree<RutterContext>,
    root:   NodeId,
    size:   PhysicalSize<u32>,
    fs_rc:  Rc<RefCell<FontSystem>>,
) {
    let available = Size {
        width:  AvailableSpace::Definite(size.width  as f32),
        height: AvailableSpace::Definite(size.height as f32),
    };

    taffy
        .compute_layout_with_measure(root, available, |known, available, _, ctx, _| {
            // Apenas nós Text precisam de medição
            let Some(RutterContext::Text(t)) = ctx else {
                return Size::ZERO;
            };

            let mut fs = fs_rc.borrow_mut();
            let mut buf =
                Buffer::new(&mut fs, Metrics::new(t.font_size, t.font_size * 1.2));

            // Definir largura disponível para quebra de linha
            match available.width {
                AvailableSpace::Definite(px) => buf.set_size(&mut fs, Some(px), None),
                AvailableSpace::MaxContent    => buf.set_size(&mut fs, None,      None),
                AvailableSpace::MinContent    => buf.set_size(&mut fs, Some(0.0), None),
            }

            buf.set_text(&mut fs, &t.content, &Attrs::new(), Shaping::Advanced, None);
            buf.shape_until_scroll(&mut fs, true);

            let (w, h) = buf.size();
            Size {
                width:  known.width.unwrap_or(w.unwrap_or(0.0)),
                height: known.height.unwrap_or(h.unwrap_or(0.0)),
            }
        })
        .unwrap();
}
