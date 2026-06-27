use color_eyre::eyre::{Context, Result};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
};
use rig::{client::CompletionClient, providers::gemini, tool::ToolDyn};
use std::io;
use tokio::sync::mpsc;

mod agent;
mod tools;
mod ui;

use agent::runtime::{AgentEvent, AgentRuntime};
use tools::fs::{ListFiles, ReadFile};

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
    let mut app = ui::app::App::new(rx);
    let res = ui::app::run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
