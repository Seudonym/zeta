use color_eyre::eyre::{Context, Result};
use rig::{client::CompletionClient, providers::gemini, tool::ToolDyn};
use std::io::{self, Write};
use tokio::sync::mpsc;

mod agent;
mod tools;

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

    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                AgentEvent::Token(token) => {
                    print!("{token}");
                    io::stdout().flush().unwrap();
                }
                AgentEvent::ToolCall(tool_call) => {
                    println!(
                        "\n{} called with args {}\n",
                        tool_call.function.name, tool_call.function.arguments
                    )
                }
                AgentEvent::Done => println!("\n[DONE]"),
                _ => {}
            }
        }
    });

    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        runtime.chat(input.to_string()).await?;
    }
}
