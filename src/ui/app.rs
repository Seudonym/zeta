use ratatui::{
    Frame, Terminal,
    backend::Backend,
    crossterm::event::{self, Event, KeyCode},
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
};
use std::io;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::agent::runtime::AgentEvent;

enum MessageLine {
    User(String),
    Assistant(String),
    ToolCall(String, String),
}

pub struct App {
    input: String,
    messages: Vec<MessageLine>,
    waiting: bool,
    exit: bool,
    rx: Option<UnboundedReceiver<AgentEvent>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            input: String::from(""),
            messages: Vec::<MessageLine>::new(),
            waiting: false,
            exit: false,
            rx: None,
        }
    }

    pub fn with_receiver(self, rx: UnboundedReceiver<AgentEvent>) -> Self {
        Self {
            rx: Some(rx),
            ..self
        }
    }
}

pub fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(frame.area());

    let input_block = Block::default()
        .borders(Borders::TOP)
        .style(Style::default().bg(Color::Black));

    let input_text_widget = if app.input.is_empty() {
        Paragraph::new(Text::styled(
            "Prompt goes here",
            Style::default().fg(Color::DarkGray).italic(),
        ))
        .block(input_block)
    } else {
        Paragraph::new(Text::styled(&app.input, Style::default().fg(Color::White)))
            .block(input_block)
    };

    frame.render_widget(input_text_widget, chunks[1]);
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool>
where
    io::Error: From<B::Error>,
{
    loop {
        if app.exit {
            return Ok(true);
        }
        terminal.draw(|f| ui(f, app))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Release {
                continue;
            }

            match key.code {
                KeyCode::Esc => {
                    app.exit = true;
                }
                KeyCode::Enter => {
                    app.input.push('\n');
                }
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Char(value) => {
                    app.input.push(value);
                }
                _ => {}
            }
        }
    }
}
