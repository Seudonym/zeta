use ratatui::{
    Frame, Terminal,
    backend::Backend,
    crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEvent, MouseEventKind},
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};
use ratatui_textarea::TextArea;
use std::{io, time::Duration};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tui_markdown::{Options, StyleSheet};

use crate::agent::runtime::AgentEvent;

const SPINNER_FRAMES: [&str; 4] = ["/", "-", "\\", "|"];

enum Role {
    User,
    Assistant,
}

struct ChatMessage {
    role: Role,
    text: String,
    rendered: Vec<Line<'static>>,
    dirty: bool,
}

enum Message {
    Chat(ChatMessage),
    ToolCall {
        name: String,
        args: String,
        rendered: Vec<Line<'static>>,
    },
    Error {
        text: String,
        rendered: Vec<Line<'static>>,
    },
}

#[derive(Clone)]
pub struct ZetaStyleSheet;
impl StyleSheet for ZetaStyleSheet {
    fn heading(&self, level: u8) -> Style {
        let base_style = Style::new().bold();
        match level {
            1 => base_style.red(),
            2 => base_style.green(),
            3 => base_style.blue(),
            4 => base_style.cyan(),
            5 => base_style.magenta(),
            6 => base_style.yellow(),
            _ => base_style.white(),
        }
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
    messages: Vec<Message>,

    scroll_offset: u16,
    max_scroll_offset: u16,
    auto_scroll: bool,

    waiting: bool,
    exit: bool,
    frame_count: usize,
    event_rx: UnboundedReceiver<AgentEvent>,
    cmd_tx: UnboundedSender<String>,
    md_options: Options<ZetaStyleSheet>,
}

impl<'a> App<'a> {
    pub fn new(event_rx: UnboundedReceiver<AgentEvent>, cmd_tx: UnboundedSender<String>) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_placeholder_text("Input goes here");
        textarea.set_placeholder_style(Style::default().italic());
        textarea.set_wrap_mode(ratatui_textarea::WrapMode::Word);

        let md_options = Options::new(ZetaStyleSheet);

        Self {
            textarea,
            messages: Vec::<Message>::new(),

            scroll_offset: 0,
            max_scroll_offset: 0,
            auto_scroll: false,

            frame_count: 0,
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
    let input_height = (input_line_count + 2).clamp(3, 20);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(input_height)])
        .split(frame.area());

    let input_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(chunks[1]);

    app.textarea
        .set_block(Block::new().borders(Borders::TOP | Borders::BOTTOM));

    let indicator_text = if app.waiting {
        let frame_idx = app.frame_count % SPINNER_FRAMES.len();
        SPINNER_FRAMES[frame_idx]
    } else {
        ">"
    };

    let indicator = Paragraph::new(Text::from(indicator_text))
        .block(Block::default().borders(Borders::TOP | Borders::BOTTOM));

    let mut lines: Vec<Line> = Vec::new();
    for msg in &app.messages {
        match msg {
            Message::Chat(chat) => {
                lines.extend(chat.rendered.iter().cloned());
            }
            Message::ToolCall { rendered, .. } => {
                lines.extend(rendered.iter().cloned());
            }

            Message::Error { rendered, .. } => {
                lines.extend(rendered.iter().cloned());
            }
        }

        lines.push(Line::default());
    }

    let logical_line_count = lines
        .iter()
        .map(|line| {
            let w = line.width();
            if w == 0 {
                1
            } else {
                w.div_ceil(chunks[0].width as usize)
            }
        })
        .sum::<usize>();
    app.max_scroll_offset = (logical_line_count as u16 + 2).saturating_sub(chunks[0].height);
    if app.auto_scroll {
        app.scroll_offset = app.max_scroll_offset;
    }

    let messages_para = Paragraph::new(Text::from(lines))
        .scroll((app.scroll_offset, 0))
        .block(Block::default().padding(Padding::new(0, 0, 1, 1)))
        .wrap(Wrap { trim: false });

    frame.render_widget(messages_para, chunks[0]);
    frame.render_widget(indicator, input_layout[0]);
    frame.render_widget(&app.textarea, input_layout[1]);
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool>
where
    io::Error: From<B::Error>,
{
    loop {
        while let Ok(event) = app.event_rx.try_recv() {
            match event {
                AgentEvent::Token(token) => match app.messages.last_mut() {
                    Some(Message::Chat(chat)) if matches!(chat.role, Role::Assistant) => {
                        chat.text.push_str(&token);
                        chat.dirty = true;
                    }

                    _ => {
                        app.messages.push(Message::Chat(ChatMessage {
                            role: Role::Assistant,
                            text: token,
                            rendered: Vec::new(),
                            dirty: true,
                        }));
                    }
                },
                AgentEvent::ToolCall(tool_call) => {
                    let lines = vec![
                        Line::from(format!(
                            "-> {}({})",
                            tool_call.function.name, tool_call.function.arguments
                        ))
                        .style(Style::default().fg(Color::Green)),
                    ];
                    app.messages.push(Message::ToolCall {
                        name: tool_call.function.name,
                        args: tool_call.function.arguments.to_string(),
                        rendered: lines,
                    });
                }
                AgentEvent::ToolCallDone => {}
                AgentEvent::Done => {
                    app.waiting = false;
                }

                AgentEvent::Error(error) => {
                    let lines =
                        vec![Line::from(error.to_string()).style(Style::default().fg(Color::Red))];
                    app.messages.push(Message::Error {
                        text: error,
                        rendered: lines,
                    });
                    app.waiting = false;
                }
            }
        }

        app.frame_count = app.frame_count.wrapping_add(1);
        update_render_cache(app);
        terminal.draw(|f| ui(f, app))?;
        if app.exit {
            return Ok(true);
        }

        if event::poll(Duration::from_millis(150))? {
            let event = event::read()?;
            if let Event::Mouse(MouseEvent { kind, .. }) = event {
                match kind {
                    MouseEventKind::ScrollDown => {
                        app.auto_scroll = false;
                        app.scroll_offset = (app.scroll_offset + 1).clamp(0, app.max_scroll_offset);
                    }
                    MouseEventKind::ScrollUp => {
                        app.auto_scroll = false;
                        app.scroll_offset = app.scroll_offset.saturating_sub(1);
                    }
                    _ => {}
                }
            }
            if let Event::Key(key) = event {
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
                            app.auto_scroll = true;
                            let input = app.textarea.lines().join("\n").trim().to_string();
                            if input.is_empty() {
                                app.waiting = false;
                                continue;
                            }

                            app.textarea.clear();
                            app.cmd_tx.send(input.clone()).ok();
                            app.messages.push(Message::Chat(ChatMessage {
                                role: Role::User,
                                text: input,
                                rendered: Vec::new(),
                                dirty: true,
                            }));
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

fn parse_markdown<'a>(text: &str, options: &Options<ZetaStyleSheet>) -> Vec<Line<'static>> {
    let md = tui_markdown::from_str_with_options(text, options);
    md.lines
        .into_iter()
        .map(|line| {
            let spans: Vec<Span<'static>> = line
                .spans
                .into_iter()
                .map(|s| Span::styled(s.content.into_owned(), s.style))
                .collect();
            Line::from(spans).style(line.style)
        })
        .collect()
}

fn update_render_cache(app: &mut App) {
    for message in &mut app.messages {
        match message {
            Message::Chat(chat) if chat.dirty => {
                chat.rendered = render_chat(chat, &app.md_options);
                chat.dirty = false;
            }

            _ => {}
        }
    }
}

fn render_chat(chat: &ChatMessage, options: &Options<ZetaStyleSheet>) -> Vec<Line<'static>> {
    match chat.role {
        Role::User => {
            vec![Line::from(chat.text.clone()).style(Style::default().fg(Color::Cyan).italic())]
        }

        Role::Assistant => parse_markdown(&chat.text, options),
    }
}
