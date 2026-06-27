use color_eyre::eyre::Result;
use futures::StreamExt;
use rig::{
    agent::{Agent, MultiTurnStreamItem},
    completion::CompletionModel,
    message::{Message, ToolCall},
    streaming::{StreamedAssistantContent, StreamedUserContent, StreamingChat},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub enum AgentEvent {
    Token(String),
    ToolCall(ToolCall),
    ToolCallDone,
    Done,
}

pub struct AgentRuntime<M>
where
    M: CompletionModel,
{
    agent: Agent<M>,
    chat_history: Vec<Message>,
    sender: UnboundedSender<AgentEvent>,
}

impl<M> AgentRuntime<M>
where
    M: CompletionModel + 'static,
{
    pub fn new(agent: Agent<M>, sender: UnboundedSender<AgentEvent>) -> Self {
        Self {
            agent,
            chat_history: Vec::<Message>::new(),
            sender,
        }
    }

    pub async fn chat(&mut self, input: String) -> Result<()> {
        let history = self.chat_history.clone();
        let mut stream = self.agent.stream_chat(input, history).await;
        while let Some(chunk) = stream.next().await {
            match chunk? {
                MultiTurnStreamItem::StreamAssistantItem(item) => match item {
                    StreamedAssistantContent::Text(msg) => {
                        self.sender.send(AgentEvent::Token(msg.text))?
                    }
                    StreamedAssistantContent::ToolCall { tool_call, .. } => {
                        self.sender.send(AgentEvent::ToolCall(tool_call))?
                    }
                    StreamedAssistantContent::Final(_) => self.sender.send(AgentEvent::Done)?,
                    _ => {}
                },
                MultiTurnStreamItem::StreamUserItem(content) => match content {
                    StreamedUserContent::ToolResult { .. } => {
                        self.sender.send(AgentEvent::ToolCallDone)?
                    }
                },
                MultiTurnStreamItem::FinalResponse(fin) => {
                    self.chat_history
                        .extend_from_slice(fin.history().unwrap_or_default());
                }
                _ => {}
            }
        }

        Ok(())
    }
}
