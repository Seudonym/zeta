use std::{path::Path, str::FromStr};

use color_eyre::eyre::Result;
use directories::ProjectDirs;
use futures::StreamExt;
use rig::{
    agent::{Agent, MultiTurnStreamItem},
    completion::CompletionModel,
    message::{Message, ToolCall},
    streaming::{StreamedAssistantContent, StreamedUserContent, StreamingChat},
};
use std::fs;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

#[derive(Debug)]
pub enum AgentEvent {
    Token(String),
    ToolCall(ToolCall),
    ToolCallDone,
    Done,

    Error(String),
}

pub struct AgentRuntime<M>
where
    M: CompletionModel,
{
    agent: Agent<M>,
    chat_history: Vec<Message>,
    sender: UnboundedSender<AgentEvent>,

    session_id: Uuid,
}

#[derive(Debug, Error)]
pub enum AgentRuntimeError {
    #[error("Failed to read directory: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to serialize/deserialize: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Invalid session ID: {0}")]
    Uuid(#[from] uuid::Error),
}

impl<M> AgentRuntime<M>
where
    M: CompletionModel + 'static,
{
    pub fn new(agent: Agent<M>, sender: UnboundedSender<AgentEvent>) -> Self {
        let session_id = Uuid::now_v7();
        Self {
            agent,
            chat_history: Vec::<Message>::new(),
            sender,

            session_id,
        }
    }

    pub async fn chat(&mut self, input: String) -> Result<()> {
        let history = self.chat_history.clone();
        let mut stream = self.agent.stream_chat(input, history).await;
        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    self.sender.send(AgentEvent::Error(e.to_string()))?;
                    return Err(e.into());
                }
            };
            match chunk {
                MultiTurnStreamItem::StreamAssistantItem(item) => match item {
                    StreamedAssistantContent::Text(msg) => {
                        self.sender.send(AgentEvent::Token(msg.text))?
                    }
                    StreamedAssistantContent::ToolCall { tool_call, .. } => {
                        self.sender.send(AgentEvent::ToolCall(tool_call))?
                    }
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
        self.sender.send(AgentEvent::Done)?;

        Ok(())
    }

    pub fn save_session(&mut self) -> Result<(), AgentRuntimeError> {
        if let Some(proj_dirs) = ProjectDirs::from("", "", "zeta") {
            let cache = proj_dirs.cache_dir();
            fs::create_dir_all(cache)?;
            let filename = format!("{}.json", self.session_id.to_string());
            let filename = Path::new(&filename);
            let abs_path = cache.join(filename);

            let json_string = serde_json::to_string(&self.chat_history)?;
            fs::write(abs_path, json_string)?;
        }

        Ok(())
    }

    pub fn load_session(&mut self, session_id: String) -> Result<(), AgentRuntimeError> {
        let uuid = Uuid::from_str(session_id.as_str())?;
        if let Some(proj_dirs) = ProjectDirs::from("", "", "zeta") {
            let cache = proj_dirs.cache_dir();
            let filename = format!("{}.json", uuid);
            let filename = Path::new(&filename);
            let abs_path = cache.join(filename);

            let file = fs::File::open(abs_path)?;
            let chat_history: Vec<Message> = serde_json::from_reader(file)?;

            self.session_id = uuid;
            self.chat_history = chat_history;
        }

        Ok(())
    }
}
