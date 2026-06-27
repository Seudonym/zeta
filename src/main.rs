use color_eyre::eyre::{Context, Result};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{
            DisableMouseCapture, EnableMouseCapture, KeyboardEnhancementFlags,
            PushKeyboardEnhancementFlags,
        },
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

    let (event_tx, event_rx) = mpsc::unbounded_channel::<AgentEvent>();
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<String>();
    let mut runtime = AgentRuntime::new(agent, event_tx);

    tokio::spawn(async move {
        while let Some(input) = cmd_rx.recv().await {
            runtime.chat(input).await.ok();
        }
    });

    // ui
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = ui::app::App::new(event_rx, cmd_tx);
    let _res = ui::app::run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
