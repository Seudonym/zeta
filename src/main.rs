use color_eyre::eyre::{Context, Result};
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
};
use rig::{client::CompletionClient, providers::gemini, tool::ToolDyn};
use std::io;
use tokio::sync::mpsc::{self, UnboundedReceiver};

mod agent;
mod tools;

use agent::runtime::{AgentEvent, AgentRuntime};
use tools::fs::{ListFiles, ReadFile};

enum MessageLine {
    User(String),
    Assistant(String),
    ToolCall(String, String),
}

struct App {
    input: String,
    messages: Vec<MessageLine>,
    waiting: bool,
    exit: bool,
    rx: Option<UnboundedReceiver<AgentEvent>>,
}

impl App {
    fn new() -> Self {
        Self {
            input: String::from(""),
            messages: Vec::<MessageLine>::new(),
            waiting: false,
            exit: false,
            rx: None,
        }
    }

    fn with_receiver(self, rx: UnboundedReceiver<AgentEvent>) -> Self {
        Self {
            rx: Some(rx),
            ..self
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let api_key =
        std::env::var("GEMINI_API_KEY").wrap_err("GEMINI_API_KEY variable is missing in .envrc")?;
    let tools: Vec<Box<dyn ToolDyn>> = vec![Box::new(ListFiles), Box::new(ReadFile)];
    let agent = gemini::Client::new(api_key)?
        .agent("gemini-3.1-flash-lite")
        .tools(tools)
        .preamble("You are a local assistant. Say whatever.")
        .build();

    let (tx, mut rx) = mpsc::unbounded_channel::<AgentEvent>();
    let mut runtime = AgentRuntime::new(agent, tx);

    // ui
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new().with_receiver(rx);
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(frame: &mut Frame, app: &App) {
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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool>
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
