use color_eyre::eyre::{Context, Result};
use rig::{client::CompletionClient, providers::gemini};
use std::io;

mod agent;
mod tools;

use agent::runtime::AgentRuntime;
use tools::math::Adder;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let api_key =
        std::env::var("GEMINI_API_KEY").wrap_err("GEMINI_API_KEY variable is missing in .envrc")?;

    let agent = gemini::Client::new(api_key)?
        .agent("gemini-3.1-flash-lite")
        .tool(Adder)
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

        let response = runtime.chat(input.to_string()).await?;
        println!("{}", response);
    }
}
