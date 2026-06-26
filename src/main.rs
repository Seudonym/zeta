use color_eyre::eyre::{Context, Result};
use rig::{client::CompletionClient, completion::ToolDefinition, providers::gemini, tool::Tool};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{convert::Infallible, io};

mod agent;
use agent::runtime::AgentRuntime;

#[derive(Deserialize)]
struct AddArgs {
    x: i32,
    y: i32,
}
#[derive(Deserialize, Serialize)]
struct Adder;

impl Tool for Adder {
    const NAME: &'static str = "add";
    // TODO: giga ass, pls change after testing
    type Error = Infallible;
    type Args = AddArgs;
    type Output = i32;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "add".to_string(),
            description: "Add x and y together".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "x": { "type": "number", "description": "First number" },
                    "y": { "type": "number", "description": "Second number" }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok(args.x + args.y)
    }
}

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
