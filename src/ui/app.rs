use ratatui::{
    Frame, Terminal,
    backend::Backend,
    crossterm::event::{self, Event, KeyCode},
    layout::{Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, Padding},
};
use ratatui_textarea::TextArea;
use std::io;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::agent::runtime::AgentEvent;

enum MessageLine {
    User(String),
    Assistant(String),
    ToolCall(String, String),
}

pub struct App<'a> {
    textarea: TextArea<'a>,
    messages: Vec<MessageLine>,
    waiting: bool,
    exit: bool,
    rx: UnboundedReceiver<AgentEvent>,
}

impl<'a> App<'a> {
    pub fn new(rx: UnboundedReceiver<AgentEvent>) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_placeholder_text("Input goes here");
        textarea.set_placeholder_style(Style::default().fg(Color::DarkGray).italic());

        Self {
            textarea,
            messages: Vec::<MessageLine>::new(),
            waiting: false,
            exit: false,
            rx,
        }
    }
}

pub fn ui(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Max(6)])
        .split(frame.area());

    app.textarea
        .set_block(Block::new().padding(Padding::new(1, 1, 1, 0)));

    frame.render_widget(&app.textarea, chunks[1]);

    let test = Block::new().bg(Color::Rgb(10, 10, 10));
    frame.render_widget(test, chunks[0]);
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
                    app.waiting = true;
                }
                _ => {
                    app.textarea.input(key);
                }
            }
        }
    }
}
