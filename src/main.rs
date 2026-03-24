mod framework;
use framework::{AppLogic, RutterRunner, Widget, InputState};
use skia_safe::Color;
use taffy::prelude::*;
use cosmic_text::{Editor, FontSystem, Metrics, Buffer, Edit};
use arboard::Clipboard;

struct FormApp {
    name_editor: Editor<'static>,
    email_editor: Editor<'static>,
    pass_editor: Editor<'static>,
    
    email_state: InputState,
    email_error: Option<String>,
    
    saved_name: String, 
}

#[derive(Debug, Clone)]
// #[warn(unused)]
enum Message {
    NameChanged(String),
    EmailChanged(String),
    PassChanged(String),
    Submit,
    Cancel,
}

impl AppLogic for FormApp {
    type State = FormApp;
    type Message = Message;

    fn new(fs: &mut FontSystem) -> Self::State {
        let mut name_ed = Editor::new(Buffer::new(fs, Metrics::new(20.0, 24.0)));
        name_ed.insert_string("Visitante", None); 

        let email_ed = Editor::new(Buffer::new(fs, Metrics::new(20.0, 24.0)));
        let pass_ed = Editor::new(Buffer::new(fs, Metrics::new(20.0, 24.0)));
        
        FormApp {
            name_editor: name_ed,
            email_editor: email_ed,
            pass_editor: pass_ed,
            email_state: InputState::Idle,
            email_error: None,
            saved_name: String::new(),
        }
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Self::Message> {
        use Widget::*;
        
        let px = |v: f32| LengthPercentage::length(v);
        let pct = |v: f32| LengthPercentage::percent(v / 100.0);
        let zero = LengthPercentageAuto::length(0.0);

        Container {
            color: Some(Color::from_rgb(240, 240, 245)),
            radius: 0.0,
            style: Style { size: Size { width: pct(100.0).into(), height: pct(100.0).into() }, align_items: Some(AlignItems::Center), justify_content: Some(JustifyContent::Center), ..Default::default() },
            child: Box::new(
                Container {
                    color: Some(Color::WHITE),
                    radius: 16.0,
                    style: Style { 
                        size: Size { width: px(400.0).into(), height: Dimension::auto() }, 
                        padding: Rect { left: px(30.0).into(), right: px(30.0).into(), top: px(30.0).into(), bottom: px(30.0).into() },
                        flex_direction: FlexDirection::Column,
                        // AUMENTADO: Gap maior para acomodar labels flutuantes e erros
                        gap: Size { width: px(0.0), height: px(30.0) },
                        ..Default::default() 
                    },
                    child: Box::new(Column {
                        // AUMENTADO: Gap interno entre widgets
                        style: Style { size: Size::AUTO, gap: Size { width: px(0.0), height: px(20.0) }, ..Default::default() },
                        children: vec![
                            Text { content: "Login Seguro".into(), color: Color::BLACK, size: 24.0, style: Style::default() },
                            
                            Text { content: format!("Digitando: {}", state.saved_name), color: Color::from_rgb(150,150,150), size: 12.0, style: Style::default() },

                            TextInput {
                                id: 1, label: "Nome", editor: &mut state.name_editor, 
                                on_change: Message::NameChanged, on_submit: None, state: InputState::Idle, error_msg: None, is_password: false,
                                style: Style { size: Size { width: pct(100.0).into(), height: px(50.0).into() }, ..Default::default() }
                            },

                            TextInput {
                                id: 2, label: "E-mail", editor: &mut state.email_editor,
                                on_change: Message::EmailChanged, on_submit: None, state: state.email_state, error_msg: state.email_error.clone(), is_password: false,
                                style: Style { size: Size { width: pct(100.0).into(), height: px(50.0).into() }, ..Default::default() }
                            },

                            TextInput {
                                id: 3, label: "Senha", editor: &mut state.pass_editor,
                                on_change: Message::PassChanged, on_submit: Some(Message::Submit), state: InputState::Idle, error_msg: None, 
                                is_password: true, 
                                style: Style { size: Size { width: pct(100.0).into(), height: px(50.0).into() }, ..Default::default() }
                            },

                            Row {
                                style: Style { 
                                    gap: Size { width: px(10.0), height: px(0.0) },
                                    margin: Rect { top: px(10.0).into(), bottom: zero, left: zero, right: zero },
                                    ..Default::default() 
                                },
                                children: vec![
                                    Container {
                                        color: None, radius: 0.0, style: Style { flex_grow: 1.0, ..Default::default() },
                                        child: Box::new(Button {
                                            text: "Cancelar", on_press: Message::Cancel, color: Color::from_rgb(180, 180, 180),
                                            style: Style { size: Size { width: pct(100.0).into(), height: px(45.0).into() }, ..Default::default() }
                                        })
                                    },
                                    Container {
                                        color: None, radius: 0.0, style: Style { flex_grow: 1.0, ..Default::default() },
                                        child: Box::new(Button {
                                            text: "Entrar", on_press: Message::Submit, color: Color::from_rgb(103, 80, 164),
                                            style: Style { size: Size { width: pct(100.0).into(), height: px(45.0).into() }, ..Default::default() }
                                        })
                                    },
                                ]
                            }
                        ]
                    })
                }
            )
        }
    }

    fn update(state: &mut Self::State, message: Self::Message, _clipboard: &mut Clipboard) {
        match message {
            Message::NameChanged(txt) => state.saved_name = txt,
            // CORREÇÃO: '_' para ignorar o valor e remover warning
            Message::PassChanged(_) => {}, 
            
            Message::EmailChanged(txt) => {
                if txt.contains('@') {
                    state.email_state = InputState::Success;
                    state.email_error = None;
                } else if txt.is_empty() {
                    state.email_state = InputState::Idle;
                    state.email_error = None;
                } else {
                    state.email_state = InputState::Error;
                    state.email_error = Some("Email inválido".into());
                }
            },
            Message::Submit => println!("Login enviado: {}", state.saved_name),
            Message::Cancel => state.saved_name.clear(),
        }
    }
}

fn main() {
    RutterRunner::<FormApp>::run();
}