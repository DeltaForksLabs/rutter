// // ============================================================
// // Rutter Framework — demos/advanced_widgets_demo.rs
// // Demo dos novos widgets do v6.3.1:
// //   TextArea, SearchBar, Dialog e Accordion.
// // ============================================================

// use arboard::Clipboard;
// use cosmic_text::FontSystem;
// use taffy::prelude::*;

// use rutter::{AppLogic, ButtonVariant, InputState, RutterRunner, Theme, Widget};

// #[derive(Default)]
// pub struct AdvancedWidgetsState {
//     pub search: String,
//     pub notes: String,
//     pub dialog_open: bool,
//     pub accordion_open: bool,
// }

// #[derive(Debug, Clone)]
// pub enum Msg {
//     SearchChanged(String),
//     SubmitSearch,
//     NotesChanged(String),
//     ToggleDialog,
//     ToggleAccordion,
// }

// pub struct AdvancedWidgetsDemo;

// impl AppLogic for AdvancedWidgetsDemo {
//     type State = AdvancedWidgetsState;
//     type Message = Msg;

//     fn new(_: &mut FontSystem) -> Self::State {
//         AdvancedWidgetsState {
//             notes: "Primeira linha\nSegunda linha".into(),
//             ..Default::default()
//         }
//     }

//     fn view<'a>(s: &'a mut AdvancedWidgetsState) -> Widget<'a, Msg> {
//         let root = Style {
//             flex_direction: FlexDirection::Column,
//             align_items: Some(AlignItems::FlexStart),
//             size: Size {
//                 width: Dimension::percent(1.0),
//                 height: Dimension::percent(1.0),
//             },
//             padding: Rect::length(24.0),
//             gap: Size {
//                 width: LengthPercentage::length(0.0),
//                 height: LengthPercentage::length(16.0),
//             },
//             ..Default::default()
//         };

//         let field = Style {
//             size: Size {
//                 width: Dimension::length(460.0),
//                 height: Dimension::length(44.0),
//             },
//             ..Default::default()
//         };

//         let text_area_style = Style {
//             size: Size {
//                 width: Dimension::length(460.0),
//                 height: Dimension::length(132.0),
//             },
//             ..Default::default()
//         };

//         let accordion_body = Widget::Column {
//             style: Style {
//                 flex_direction: FlexDirection::Column,
//                 padding: Rect::length(16.0),
//                 gap: Size {
//                     width: LengthPercentage::length(0.0),
//                     height: LengthPercentage::length(10.0),
//                 },
//                 ..Default::default()
//             },
//             children: vec![
//                 Widget::Text {
//                     content: "O Accordion compartilha a mesma largura do cabeçalho e respeita o espaçamento interno.".into(),
//                     color: None,
//                     size: 13.0,
//                     style: Style::default(),
//                 },
//                 Widget::Button {
//                     text: "Abrir Dialog",
//                     on_press: Msg::ToggleDialog,
//                     style: Style {
//                         size: Size {
//                             width: Dimension::length(160.0),
//                             height: Dimension::length(36.0),
//                         },
//                         ..Default::default()
//                     },
//                     color: None,
//                     variant: ButtonVariant::Ghost,
//                 },
//             ],
//         };

//         let dialog = Widget::Dialog {
//             id: 140,
//             visible: s.dialog_open,
//             on_dismiss: Some(Msg::ToggleDialog),
//             style: Style {
//                 size: Size {
//                     width: Dimension::percent(1.0),
//                     height: Dimension::percent(1.0),
//                 },
//                 ..Default::default()
//             },
//             child: Box::new(Widget::Column {
//                 style: Style {
//                     flex_direction: FlexDirection::Column,
//                     align_items: Some(AlignItems::FlexStart),
//                     padding: Rect::length(24.0),
//                     gap: Size {
//                         width: LengthPercentage::length(0.0),
//                         height: LengthPercentage::length(16.0),
//                     },
//                     ..Default::default()
//                 },
//                 children: vec![
//                     Widget::Text {
//                         content: "Dialog component".into(),
//                         color: None,
//                         size: 18.0,
//                         style: Style::default(),
//                     },
//                     Widget::Text {
//                         content: "Este dialog reutiliza o pipeline do modal, mas como um componente semântico próprio.".into(),
//                         color: None,
//                         size: 13.0,
//                         style: Style::default(),
//                     },
//                     Widget::Button {
//                         text: "Fechar",
//                         on_press: Msg::ToggleDialog,
//                         style: Style {
//                             size: Size {
//                                 width: Dimension::length(120.0),
//                                 height: Dimension::length(36.0),
//                             },
//                             ..Default::default()
//                         },
//                         color: None,
//                         variant: ButtonVariant::Primary,
//                     },
//                 ],
//             }),
//         };

//         Widget::Column {
//             style: root,
//             children: vec![
//                 Widget::Text {
//                     content: "Advanced widgets".into(),
//                     color: None,
//                     size: 18.0,
//                     style: Style::default(),
//                 },
//                 Widget::SearchBar {
//                     id: 120,
//                     on_change: Msg::QueryChanged,
//                     on_submit: Some(Msg::SubmitSearch),
//                     on_search: Some(Msg::SubmitSearch),
//                     on_clear: Some(Msg::ClearSearch),
//                     style: field.clone(),
//                     placeholder: "Buscar widgets...",
//                 },
//                 Widget::TextArea {
//                     id: 121,
//                     on_change: Msg::NotesChanged,
//                     on_submit: None,
//                     style: text_area_style,
//                     label: "Notas",
//                     placeholder: "Digite múltiplas linhas...",
//                     state: InputState::Idle,
//                     error_msg: None,
//                 },
//                 Widget::Accordion {
//                     id: 122,
//                     title: "Accordion component",
//                     expanded: s.accordion_open,
//                     on_toggle: Msg::ToggleAccordion,
//                     style: Style {
//                         size: Size {
//                             width: Dimension::length(460.0),
//                             height: Dimension::length(if s.accordion_open { 180.0 } else { 44.0 }),
//                         },
//                         ..Default::default()
//                     },
//                     child: Box::new(accordion_body),
//                 },
//                 Widget::Row {
//                     style: Style {
//                         flex_direction: FlexDirection::Row,
//                         gap: Size {
//                             width: LengthPercentage::length(12.0),
//                             height: LengthPercentage::length(0.0),
//                         },
//                         ..Default::default()
//                     },
//                     children: vec![
//                         Widget::Button {
//                             text: "Toggle Accordion",
//                             on_press: Msg::ToggleAccordion,
//                             style: Style {
//                                 size: Size {
//                                     width: Dimension::length(160.0),
//                                     height: Dimension::length(36.0),
//                                 },
//                                 ..Default::default()
//                             },
//                             color: None,
//                             variant: ButtonVariant::Ghost,
//                         },
//                         Widget::Button {
//                             text: "Toggle Dialog",
//                             on_press: Msg::ToggleDialog,
//                             style: Style {
//                                 size: Size {
//                                     width: Dimension::length(160.0),
//                                     height: Dimension::length(36.0),
//                                 },
//                                 ..Default::default()
//                             },
//                             color: None,
//                             variant: ButtonVariant::Primary,
//                         },
//                     ],
//                 },
//                 Widget::Text {
//                     content: format!("Search='{}' | Notes={} chars", s.search, s.notes.len()),
//                     color: None,
//                     size: 12.0,
//                     style: Style::default(),
//                 },
//                 dialog,
//             ],
//         }
//     }

//     fn update(s: &mut AdvancedWidgetsState, msg: Msg, _: &mut Clipboard) {
//         match msg {
//             Msg::SearchChanged(v) => s.search = v,
//             Msg::NotesChanged(v) => s.notes = v,
//             Msg::ToggleDialog => s.dialog_open = !s.dialog_open,
//             Msg::ToggleAccordion => s.accordion_open = !s.accordion_open,
//         }
//     }

//     fn theme() -> Theme {
//         Theme::dark()
//     }
// }

// pub fn run() {
//     RutterRunner::<AdvancedWidgetsDemo>::run();
// }
