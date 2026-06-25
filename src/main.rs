use std::io;

use color_eyre::eyre::Result;
use rig::{
    OneOrMany,
    client::CompletionClient,
    completion::{Chat, Message},
    message::{AssistantContent, UserContent},
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

        let mut chat_history_clone = chat_history.clone();
        let response = agent.chat(input, &mut chat_history_clone);

        let user_message = Message::User {
            content: OneOrMany::one(UserContent::text(input)),
        };
        chat_history.push(user_message);

        let response = response.await?;
        println!("{}", response);

        let assistant_message = Message::Assistant {
            id: None,
            content: OneOrMany::one(AssistantContent::from(response)),
        };
        chat_history.push(assistant_message);
    }
}
