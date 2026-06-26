use std::io;

use color_eyre::eyre::Result;
use rig::{
    agent::Agent,
    client::CompletionClient,
    completion::{Chat, CompletionModel, Message},
    providers::ollama,
};

struct AgentRuntime<M>
where
    M: CompletionModel,
{
    agent: Agent<M>,
    chat_history: Vec<Message>,
}

impl<M> AgentRuntime<M>
where
    M: CompletionModel + 'static,
{
    pub fn new(agent: Agent<M>) -> Self {
        Self {
            agent,
            chat_history: Vec::<Message>::new(),
        }
    }

    pub async fn chat(&mut self, input: &str) -> Result<String> {
        let response = self.agent.chat(input, &mut self.chat_history);
        let response = response.await?;
        Ok(response)
    }
}

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
