use color_eyre::eyre::Result;
use rig::{
    agent::Agent,
    completion::{Chat, CompletionModel},
    message::Message,
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

    pub async fn chat(&mut self, input: &str) -> Result<String> {
        let response = self.agent.chat(input, &mut self.chat_history);
        let response = response.await?;
        Ok(response)
    }
}
