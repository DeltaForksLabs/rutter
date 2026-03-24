// ============================================================
// Rutter Framework — render/hit_test.rs
// Travessia da Widget tree para:
//   • hit_test         → encontrar o widget sob o cursor
//   • find_input_mut   → obter referência mutável ao Editor
//   • collect_input_ids→ listar inputs para navegação por Tab
// ============================================================

use skia_safe::{Contains, Point, Rect as SkiaRect};
use taffy::prelude::{NodeId, TaffyTree};

use crate::layout::RutterContext;
use crate::widget::Widget;

// ── Resultado de hit test ────────────────────────────────────

pub enum HitResult<Msg> {
    /// Clique em botão → emitir mensagem
    Message(Msg),
    /// Clique em campo de texto → dar foco ao input `id`
    InputFocus(u64),
}

// ── Hit test ─────────────────────────────────────────────────

/// Testa recursivamente qual widget está sob `mouse` (coordenadas
/// de janela). Itera filhos de trás para frente para que o widget
/// visualmente acima seja testado primeiro.
///
/// `abs` é a posição acumulada do nó pai em coordenadas de janela.
pub fn hit_test<Msg: Clone>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
) -> Option<HitResult<Msg>> {
    let layout = taffy.layout(node_id).unwrap();
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    let size = layout.size;
    let rect = SkiaRect::from_xywh(abs_pos.x, abs_pos.y, size.width, size.height);

    // Descartar imediatamente se o mouse não está dentro do rect
    if !rect.contains(mouse) {
        return None;
    }

    match widget {
        Widget::Button { on_press, .. } => Some(HitResult::Message(on_press.clone())),

        Widget::TextInput { id, .. } => Some(HitResult::InputFocus(*id)),

        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node_id).unwrap();
            // Iterar de trás pra frente: último filho fica "por cima" visualmente
            for (i, child) in children.iter().enumerate().rev() {
                if let Some(r) = hit_test(child, taffy, ids[i], mouse, abs_pos) {
                    return Some(r);
                }
            }
            None
        }

        Widget::Container { child, .. } => {
            let ids = taffy.children(node_id).unwrap();
            hit_test(child.as_ref(), taffy, ids[0], mouse, abs_pos)
        }

        // Text e outros widgets não recebem eventos de mouse
        _ => None,
    }
}

// ── Coleta de IDs para navegação Tab ────────────────────────

/// Coleta todos os IDs de `TextInput` em ordem de árvore (pré-ordem).
/// Usado para calcular o próximo/anterior campo no Tab/Shift+Tab.
pub fn collect_input_ids<Msg>(widget: &Widget<Msg>, ids: &mut Vec<u64>) {
    match widget {
        Widget::TextInput { id, .. } => ids.push(*id),

        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for child in children {
                collect_input_ids(child, ids);
            }
        }

        Widget::Container { child, .. } => collect_input_ids(child, ids),

        _ => {}
    }
}

// ── Busca mutável por TextInput ──────────────────────────────

/// Encontra um `TextInput` pelo `target_id` e retorna referências
/// mutáveis ao seu Editor e callbacks.
///
/// # FIX #4 — Lifetimes invariantes
/// A assinatura original usava o mesmo `'a` para a referência e
/// para o parâmetro de tipo do Widget, tornando-o invariante.
/// Com lifetimes distintos `'w` (duração do empréstimo) e `'a`
/// (parâmetro do Widget), o compilador aceita a reborrowagem.
pub fn find_input_callbacks<'w, 'a, Msg>(
    widget: &'w Widget<'a, Msg>,
    target_id: u64,
) -> Option<(fn(String) -> Msg, Option<Msg>)>
where
    Msg: Clone,
{
    match widget {
        Widget::TextInput {
            id,
            on_change,
            on_submit,
            ..
        } if *id == target_id => Some((*on_change, on_submit.clone())),

        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for child in children {
                if let Some(r) = find_input_callbacks(child, target_id) {
                    return Some(r);
                }
            }
            None
        }

        Widget::Container { child, .. } => find_input_callbacks(child.as_ref(), target_id),

        _ => None,
    }
}
