use color_eyre::eyre::Result;
use rig::{client::CompletionClient, completion::Prompt, providers::ollama::Client};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let client = Client::new("http://127.0.0.1:11434")?;
    let agent = client
        .agent("smollm")
        .preamble("You are a local assistant. Say whatever.")
        .build();

    let response = agent.prompt("What model are you?").await?;
    println!("{}", response);
    Ok(())
}
