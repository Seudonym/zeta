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

enum MessageLine {
    User(String, Vec<Line<'static>>),
    Assistant(String, Vec<Line<'static>>),
    ToolCall(String, String, Vec<Line<'static>>),
    Error(String, Vec<Line<'static>>),
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
    messages: Vec<MessageLine>,

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
            messages: Vec::<MessageLine>::new(),

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
    for msg in app.messages.iter() {
        match msg {
            MessageLine::User(_, l)
            | MessageLine::Assistant(_, l)
            | MessageLine::ToolCall(_, _, l)
            | MessageLine::Error(_, l) => {
                lines.extend(l.iter().cloned());
            }
        }
        lines.push(Line::from(""));
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
                AgentEvent::Token(token) => {
                    if let Some(MessageLine::Assistant(text, cache)) = app.messages.last_mut() {
                        text.push_str(&token);
                        *cache = parse_markdown(text, &app.md_options)
                    } else {
                        let mut text = String::new();
                        text.push_str(&token);
                        let lines = parse_markdown(&text, &app.md_options);
                        app.messages.push(MessageLine::Assistant(text, lines));
                    }
                }
                AgentEvent::ToolCall(tool_call) => {
                    let lines = vec![
                        Line::from(format!(
                            "-> {}({})",
                            tool_call.function.name, tool_call.function.arguments
                        ))
                        .style(Style::default().fg(Color::Green)),
                    ];
                    app.messages.push(MessageLine::ToolCall(
                        tool_call.function.name,
                        tool_call.function.arguments.to_string(),
                        lines,
                    ));
                }
                AgentEvent::ToolCallDone => {}
                AgentEvent::Done => {
                    app.waiting = false;
                }

                AgentEvent::Error(error) => {
                    let lines =
                        vec![Line::from(error.to_string()).style(Style::default().fg(Color::Red))];
                    app.messages.push(MessageLine::Error(error, lines));
                    app.waiting = false;
                }
            }
        }

        app.frame_count = app.frame_count.wrapping_add(1);
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
                            let lines = vec![
                                Line::from(vec![Span::raw(input.clone())])
                                    .style(Style::default().fg(Color::Cyan).italic()),
                            ];
                            app.messages.push(MessageLine::User(input, lines));
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
