use color_eyre::eyre::{Context, Result};
use crossterm::execute;
use crossterm::style::{Color, SetForegroundColor};
use rig::{client::CompletionClient, providers::deepseek, tool::ToolDyn};
use std::io::{self, Write, stdout};
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
    let system_prompt = construct_system_prompt(base_prompt);
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

    cmd_tx.send(clean_input)?;
    Ok(())
}

fn handle_agent_event(event: AgentEvent) -> Result<(), CliError> {
    // dbg!(&event);
    match event {
        AgentEvent::Token(token) => {
            print!("{}", token);
            io::stdout().flush()?;
        }

        AgentEvent::ToolCall(tool_call) => {
            execute!(stdout(), SetForegroundColor(Color::DarkCyan))?;
            let fn_name = util::to_pascal_case(&tool_call.function.name);
            let fn_args = to_str_arguments(tool_call.function.arguments);
            println!("\n[+] {} {}", fn_name, fn_args);
            execute!(stdout(), SetForegroundColor(Color::Reset))?;
        }

        AgentEvent::ToolResult { content, .. } => {
            execute!(stdout(), SetForegroundColor(Color::DarkGrey))?;
            let truncated = util::truncate(&content, 800);
            println!("{}", util::indent(&truncated, 6));
            execute!(stdout(), SetForegroundColor(Color::Reset))?;
            println!();
        }

        AgentEvent::Error(err) => {
            execute!(stdout(), SetForegroundColor(Color::Red))?;
            eprintln!("\n[error] {}\n", err);
            execute!(stdout(), SetForegroundColor(Color::Reset))?;
        }

        AgentEvent::Done => {
            print!("\n>>> ");
            io::stdout().flush()?;
        }
    }

    Ok(())
}

fn construct_system_prompt(preamble: String) -> String {
    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    format!("{}\nCurrent directory: {}", preamble, cwd)
}

fn to_str_arguments(args: serde_json::value::Value) -> String {
    let arguments = args.as_object().expect("failed to parse tool call args");
    let mut args_vec: Vec<_> = arguments.iter().collect();
    args_vec.sort_by_key(|&(key, _)| key);

    args_vec
        .iter()
        .map(|(key, value)| {
            if let Some(string) = value.as_str() {
                format!("({}: {})", key, string.to_string())
            } else {
                format!("({}: {})", key, value.to_string())
            }
        })
        .collect::<Vec<String>>()
        .join(", ")
}
