use ratatui::{
    Frame, Terminal,
    backend::Backend,
    crossterm::event::{self, Event, KeyCode, KeyModifiers},
    layout::{Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};
use ratatui_textarea::TextArea;
use std::{io, time::Duration};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tui_markdown::{Options, StyleSheet};

use crate::agent::runtime::AgentEvent;

enum MessageLine {
    User(String),
    Assistant(String),
    ToolCall(String, String),
}
#[derive(Clone)]
pub struct ZetaStyleSheet;
impl StyleSheet for ZetaStyleSheet {
    fn heading(&self, level: u8) -> Style {
        Style::new().bold()
    }

    fn code(&self) -> Style {
        Style::new().white().on_dark_gray()
    }

    fn link(&self) -> Style {
        Style::new().blue().underlined()
    }

    fn blockquote(&self) -> Style {
        Style::new().yellow()
    }

    fn heading_meta(&self) -> Style {
        Style::new().dim()
    }

    fn metadata_block(&self) -> Style {
        Style::new().light_yellow()
    }
}

pub struct App<'a> {
    textarea: TextArea<'a>,
    messages: Vec<MessageLine>,
    waiting: bool,
    exit: bool,
    event_rx: UnboundedReceiver<AgentEvent>,
    cmd_tx: UnboundedSender<String>,
    md_options: Options<ZetaStyleSheet>,
}

impl<'a> App<'a> {
    pub fn new(event_rx: UnboundedReceiver<AgentEvent>, cmd_tx: UnboundedSender<String>) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_placeholder_text("Input goes here");
        textarea.set_placeholder_style(Style::default().fg(Color::DarkGray).italic());
        textarea.set_wrap_mode(ratatui_textarea::WrapMode::Word);

        let md_options = Options::new(ZetaStyleSheet);

        Self {
            textarea,
            messages: Vec::<MessageLine>::new(),
            waiting: false,
            exit: false,
            event_rx,
            cmd_tx,
            md_options,
        }
    }
}

pub fn ui(frame: &mut Frame, app: &mut App) {
    let input_line_count = app.textarea.lines().len() as u16;
    let input_height = (input_line_count + 2).max(3).min(20);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(input_height)])
        .split(frame.area());

    app.textarea.set_block(
        Block::new()
            .padding(Padding::new(1, 1, 1, 1))
            .bg(Color::Rgb(40, 40, 40)),
    );

    let mut lines: Vec<Line> = Vec::new();
    for msg in app.messages.iter() {
        match msg {
            MessageLine::User(text) => {
                lines.push(
                    Line::from(vec![Span::raw(text.clone())])
                        .style(Style::default().fg(Color::Cyan)),
                );
            }
            MessageLine::Assistant(text) => {
                if !text.is_empty() {
                    let md = tui_markdown::from_str_with_options(text, &app.md_options);
                    lines.extend(md.lines);
                }
            }
            MessageLine::ToolCall(name, args) => {
                lines.push(
                    Line::from(format!("{name} {args}")).style(Style::default().fg(Color::Yellow)),
                );
            }
        }
        lines.push(Line::from(""));
    }

    let messages_para = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .padding(Padding::new(5, 5, 1, 1))
                .bg(Color::Rgb(10, 10, 10)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(messages_para, chunks[0]);
    frame.render_widget(&app.textarea, chunks[1]);
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool>
where
    io::Error: From<B::Error>,
{
    loop {
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

        terminal.draw(|f| ui(f, app))?;
        if app.exit {
            return Ok(true);
        }
    }
}
