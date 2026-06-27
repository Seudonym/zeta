use ratatui::{
    Frame, Terminal,
    backend::Backend,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Wrap},
};
use ratatui_textarea::TextArea;
use std::{fmt::format, io, time::Duration};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

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
    event_rx: UnboundedReceiver<AgentEvent>,
    cmd_tx: UnboundedSender<String>,
}

impl<'a> App<'a> {
    pub fn new(event_rx: UnboundedReceiver<AgentEvent>, cmd_tx: UnboundedSender<String>) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_placeholder_text("Input goes here");
        textarea.set_placeholder_style(Style::default().fg(Color::DarkGray).italic());
        textarea.set_wrap_mode(ratatui_textarea::WrapMode::Word);

        Self {
            textarea,
            messages: Vec::<MessageLine>::new(),
            waiting: false,
            exit: false,
            event_rx,
            cmd_tx,
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

    let mut spans = Vec::new();
    for msg in &app.messages {
        let (style, content) = match msg {
            MessageLine::User(text) => (
                Style::default().fg(Color::Rgb(0, 150, 150)),
                format!("{}\n", text.clone()),
            ),
            MessageLine::Assistant(text) => (
                Style::default().fg(Color::White),
                format!("{}\n", text.clone()),
            ),
            MessageLine::ToolCall(name, args) => (
                Style::default().fg(Color::Yellow),
                format!("{} {}", name, args),
            ),
        };

        for line in content.lines() {
            spans.push(Line::from(Span::styled(line.to_string(), style)));
        }
    }

    let messages_paragraph = Paragraph::new(spans)
        .block(
            Block::default()
                .bg(Color::Rgb(10, 10, 10))
                .padding(Padding::new(5, 5, 1, 1)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(messages_paragraph, chunks[0]);
    frame.render_widget(&app.textarea, chunks[1]);
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool>
where
    io::Error: From<B::Error>,
{
    loop {
        terminal.draw(|f| ui(f, app))?;
        if app.exit {
            return Ok(true);
        }

        while let Ok(event) = app.event_rx.try_recv() {
            match event {
                AgentEvent::Token(token) => {
                    if let Some(MessageLine::Assistant(text)) = app.messages.last_mut() {
                        text.push_str(&token);
                    } else {
                        app.messages.push(MessageLine::Assistant(token));
                    }
                }
                AgentEvent::ToolCall(tool_call) => {
                    app.messages.push(MessageLine::ToolCall(
                        tool_call.function.name,
                        tool_call.function.arguments.to_string(),
                    ));
                }
                AgentEvent::ToolCallDone => {
                    // Tool call finished -- you could add a visual separator here
                }
                AgentEvent::Done => {
                    app.waiting = false;
                }
            }
        }

        if event::poll(Duration::from_millis(150))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Release {
                    continue;
                }

                match key.code {
                    KeyCode::Esc => {
                        app.exit = true;
                    }
                    KeyCode::Enter => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            app.textarea.input(key);
                        } else {
                            app.waiting = true;
                            let input = app.textarea.lines().join("\n").trim().to_string();
                            if input.is_empty() {
                                app.waiting = false;
                                continue;
                            }

                            app.textarea.clear();
                            app.cmd_tx.send(input.clone()).ok();
                            app.messages.push(MessageLine::User(input));
                        }
                    }
                    _ => {
                        if !app.waiting {
                            app.textarea.input(key);
                        }
                    }
                }
            }
        }
    }
}
