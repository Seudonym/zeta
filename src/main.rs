use color_eyre::eyre::{Context, Result};
use crossterm::execute;
use crossterm::style::{Color, SetForegroundColor};
use rig::{client::CompletionClient, providers::gemini, tool::ToolDyn};
use serde_json::Value;
use std::io::{self, Write, stdout};
use std::iter::MapWhile;
use thiserror::Error;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

mod agent;
mod tools;

use agent::runtime::{AgentEvent, AgentRuntime};
use tools::fs::{ListFiles, ReadFile};

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
        std::env::var("GEMINI_API_KEY").wrap_err("GEMINI_API_KEY variable is missing in .envrc")?;
    let tools: Vec<Box<dyn ToolDyn>> = vec![Box::new(ListFiles), Box::new(ReadFile)];
    let agent = gemini::Client::new(api_key)?
        .agent("gemini-3.1-flash-lite")
        .tools(tools)
        .preamble(&fs::read_to_string("./src/md/SYSTEM.md").await?)
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

    println!("{}\n\n", BANNER);
    print!(">>> ");
    stdout.flush()?;

    loop {
        let mut input = String::new();

        tokio::select! {
            _ = reader.read_line(&mut input) => handle_input(&mut input, &cmd_tx)?,
            Some(event) = event_rx.recv() => handle_agent_event(event )?
        }
    }
}

fn handle_input(input: &mut String, cmd_tx: &UnboundedSender<String>) -> Result<(), CliError> {
    let clean_input = input.trim().to_string();

    if clean_input.is_empty() {
        print!(">>> ");
        io::stdout().flush()?;
        return Ok(());
    }
    println!();

    cmd_tx.send(clean_input)?;
    Ok(())
}

fn handle_agent_event(event: AgentEvent) -> Result<(), CliError> {
    dbg!(&event);
    match event {
        AgentEvent::Token(token) => {
            print!("{}", token);
            io::stdout().flush()?;
        }

        AgentEvent::ToolCall(tool_call) => {
            execute!(stdout(), SetForegroundColor(Color::DarkCyan))?;
            let fn_name = to_pascal_case(&tool_call.function.name);
            let fn_args = to_str_arguments(tool_call.function.arguments);
            println!("[+] {} {}\n", fn_name, fn_args);
            execute!(stdout(), SetForegroundColor(Color::Reset))?;
        }

        AgentEvent::Done => {
            print!("\n\n>>> ");
            io::stdout().flush()?;
        }
        _ => {}
    }

    Ok(())
}

fn to_pascal_case(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric()) // Split at underscores, spaces, hyphens, etc.
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect()
}

fn to_str_arguments(args: serde_json::value::Value) -> String {
    let arguments = args.as_object().expect("failed to parse tool call args");
    arguments
        .iter()
        .map(|(_, value)| {
            if let Some(string) = value.as_str() {
                string.to_string()
            } else {
                value.to_string()
            }
        })
        .collect::<Vec<String>>()
        .join(", ")
}
