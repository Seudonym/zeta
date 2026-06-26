use std::io;

use color_eyre::eyre::Result;
use rig::{
    client::CompletionClient,
    completion::{Chat, Message},
    providers::ollama::Client,
};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let client = Client::new("http://127.0.0.1:11434")?;
    let agent = client
        .agent("smollm")
        .preamble("You are a local assistant. Say whatever.")
        .build();
    let mut chat_history = Vec::<Message>::new();

    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        println!("{:?}", chat_history);

        let response = agent.chat(input, &mut chat_history);
        let response = response.await?;
        println!("{}", response);
    }
}
