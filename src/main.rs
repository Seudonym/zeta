use color_eyre::eyre::{Context, Result};
use crossterm::style::Stylize;
use crossterm::{ExecutableCommand, cursor, style, terminal};
use rig::{client::CompletionClient, providers::deepseek, tool::ToolDyn};
use std::io::{self, Stdout, Write};
use thiserror::Error;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

mod agent;
mod tools;
mod util;

use agent::runtime::{AgentEvent, AgentRuntime};

#[derive(Error, Debug)]
enum CliError {
    #[error("IO Error")]
    IoError(#[from] std::io::Error),

    #[error("Mpsc Error")]
    MpscError(#[from] mpsc::error::SendError<String>),
}

const BANNER: &str = "
 ▄▄▄▄▄           ▄          
  ▄█▀    ▄▄▄   ▄▄█▄▄   ▄▄▄  
 ▄▀     █▀  █    █    ▀   █ 
 █      █▀▀▀▀    █    ▄▀▀▀█ 
 ▀█▄▄   ▀█▄▄▀    ▀▄▄  ▀▄▄▀█ 
     █                      ";

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let api_key =
        // std::env::var("GEMINI_API_KEY").wrap_err("GEMINI_API_KEY variable is missing in .envrc")?;
    std::env::var("DEEPSEEK_API_KEY").wrap_err("DEEPSEEK_API_KEY variable is missing in .envrc")?;

    let mut tools: Vec<Box<dyn ToolDyn>> = Vec::new();
    tools.extend(tools::fs::toolset());
    tools.extend(tools::shell::toolset());
    tools.extend(tools::memory::toolset());
    tools.extend(tools::web::toolset());

    let base_prompt = fs::read_to_string("./src/md/SYSTEM.md").await?;
    let system_prompt = util::construct_system_prompt(base_prompt);
    let agent = deepseek::Client::new(api_key)?
        .agent("deepseek-v4-flash")
        // let agent = gemini::Client::new(api_key)?
        //     .agent("gemini-3.5-flash")
        .tools(tools)
        .default_max_turns(10)
        .preamble(&system_prompt)
        .build();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AgentEvent>();
    let mut runtime = AgentRuntime::new(agent, event_tx);
    // pipe in from cmd_rx to the runtime
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        while let Some(input) = cmd_rx.recv().await {
            runtime.chat(input).await.ok();
        }
    });

    run_cli(cmd_tx, &mut event_rx).await?;

    Ok(())
}

async fn run_cli(
    cmd_tx: UnboundedSender<String>,
    event_rx: &mut UnboundedReceiver<AgentEvent>,
) -> Result<()> {
    let mut stdout = io::stdout();
    let mut reader = BufReader::new(tokio::io::stdin());

    stdout.execute(terminal::Clear(terminal::ClearType::All))?;
    stdout.execute(cursor::MoveTo(0, 0))?;

    println!("{}\n\n", BANNER);
    print!(">>> ");
    stdout.flush()?;

    loop {
        let mut input = String::new();

        tokio::select! {
            _ = reader.read_line(&mut input) => handle_input(&mut input, &cmd_tx, &mut stdout)?,
            Some(event) = event_rx.recv() => handle_agent_event(event, &mut stdout)?
        }
    }
}

fn handle_input(
    input: &mut String,
    cmd_tx: &UnboundedSender<String>,
    stdout: &mut Stdout,
) -> Result<(), CliError> {
    let clean_input = input.trim().to_string();

    if clean_input.is_empty() {
        print!(">>> ");
        stdout.flush()?;
        return Ok(());
    }

    cmd_tx.send(clean_input)?;
    Ok(())
}

fn handle_agent_event(event: AgentEvent, stdout: &mut Stdout) -> Result<(), CliError> {
    match event {
        AgentEvent::Token(token) => {
            print!("{}", token);
            stdout.flush()?;
        }

        AgentEvent::ToolCall(tool_call) => {
            let fn_name = util::to_pascal_case(&tool_call.function.name);
            let fn_args = util::to_str_arguments(tool_call.function.arguments);
            stdout.execute(style::PrintStyledContent(
                format!("\n[+] {} {}\n", fn_name, fn_args).dark_cyan(),
            ))?;
        }

        AgentEvent::ToolResult { content, .. } => {
            let truncated = util::truncate(&content, 100);
            stdout.execute(style::PrintStyledContent(
                format!("{}\n", util::indent(&truncated, 6)).dark_grey(),
            ))?;
        }

        AgentEvent::Error(err) => {
            stdout.execute(style::PrintStyledContent(
                format!("\n[error] {}\n", err).red(),
            ))?;
        }

        AgentEvent::Done => {
            print!("\n>>> ");
            stdout.flush()?;
        }
    }

    Ok(())
}
