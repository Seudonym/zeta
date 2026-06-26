use color_eyre::eyre::Result;
use rig::{client::CompletionClient, providers::ollama};
use std::io;

mod agent;
use agent::runtime::AgentRuntime;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let agent = ollama::Client::new("http://127.0.0.1:11434")?
        .agent("smollm")
        .preamble("You are a local assistant. Say whatever.")
        .build();
    let mut runtime = AgentRuntime::new(agent);

    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        let response = runtime.chat(input).await?;
        println!("{}", response);
    }
}
