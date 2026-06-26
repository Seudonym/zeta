use std::io::{self, Write};

use color_eyre::eyre::Result;
use futures::StreamExt;
use rig::{
    agent::{Agent, MultiTurnStreamItem},
    completion::{CompletionModel, GetTokenUsage},
    message::Message,
    streaming::{StreamedAssistantContent, StreamingChat},
};

pub struct AgentRuntime<M>
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

    pub async fn chat(&mut self, input: String) -> Result<String> {
        let history = self.chat_history.clone();
        let mut stream = self.agent.stream_chat(input, history).await;
        while let Some(chunk) = stream.next().await {
            match chunk? {
                // TODO: please fucking remove the prints after testign
                MultiTurnStreamItem::StreamAssistantItem(item) => match item {
                    StreamedAssistantContent::Text(msg) => {
                        print!("{}", msg);
                        io::stdout().flush().unwrap();
                    }
                    StreamedAssistantContent::Final(usage) => {
                        println!();
                        println!("Statistics\n=============\n{:?}", usage.token_usage());
                    }
                    _ => {}
                },
                MultiTurnStreamItem::FinalResponse(fin) => {
                    self.chat_history
                        .extend_from_slice(fin.history().unwrap_or_default());

                    println!("{:?}", self.chat_history);
                }
                _ => {}
            }
        }

        Ok(String::new())
    }
}
