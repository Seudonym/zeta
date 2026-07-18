use color_eyre::eyre::Result;
use crossterm::style::Stylize;
use crossterm::{ExecutableCommand, cursor, style, terminal};
use rig::completion::CompletionModel;
use std::io::{self, Stdout, Write};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::agent::runtime::{AgentEvent, AgentRuntime};
use crate::util;

const BANNER: &str = "
 ▄▄▄▄▄           ▄          
  ▄█▀    ▄▄▄   ▄▄█▄▄   ▄▄▄  
 ▄▀     █▀  █    █    ▀   █ 
 █      █▀▀▀▀    █    ▄▀▀▀█ 
 ▀█▄▄   ▀█▄▄▀    ▀▄▄  ▀▄▄▀█ 
     █                      ";

use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Error, Debug)]
enum CliError {
    #[error("IO Error")]
    IoError(#[from] std::io::Error),
}
pub struct Tui<M>
where
    M: CompletionModel + Send + Sync + 'static,
{
    runtime: Arc<Mutex<AgentRuntime<M>>>,
    event_rx: UnboundedReceiver<AgentEvent>,
    stdout: Stdout,
}

impl<M> Tui<M>
where
    M: CompletionModel + Send + Sync + 'static,
{
    pub async fn new(runtime: Arc<Mutex<AgentRuntime<M>>>) -> Self {
        let event_rx = runtime.lock().await.subscribe();
        Self {
            runtime,
            event_rx,
            stdout: io::stdout(),
        }
    }

    pub async fn run_tui(&mut self) -> Result<()> {
        let mut reader = BufReader::new(tokio::io::stdin());

        self.stdout
            .execute(terminal::Clear(terminal::ClearType::All))?;
        self.stdout.execute(cursor::MoveTo(0, 0))?;

        println!("{}\n\n", BANNER);
        print!(">>> ");
        self.stdout.flush()?;

        loop {
            let mut input = String::new();

            tokio::select! {
                _ = reader.read_line(&mut input) => self.handle_input(&mut input)?,
                Some(event) = self.event_rx.recv() => self.handle_agent_event(event)?
            }
        }
    }

    fn handle_input(&mut self, input: &mut String) -> Result<(), CliError> {
        let clean_input = input.trim().to_string();

        if clean_input.is_empty() {
            print!(">>> ");
            self.stdout.flush()?;
            return Ok(());
        }

        let runtime = self.runtime.clone();
        tokio::spawn(async move {
            let mut rt = runtime.lock().await;
            rt.chat(clean_input).await.ok();
        });
        Ok(())
    }

    fn handle_agent_event(&mut self, event: AgentEvent) -> Result<(), CliError> {
        match event {
            AgentEvent::Token(token) => {
                print!("{}", token);
                self.stdout.flush()?;
            }

            AgentEvent::ToolCall(tool_call) => {
                let fn_name = util::to_pascal_case(&tool_call.function.name);
                let fn_args = util::to_str_arguments(tool_call.function.arguments);
                self.stdout.execute(style::PrintStyledContent(
                    format!("\n[+] {} {}\n", fn_name, fn_args).dark_cyan(),
                ))?;
            }

            AgentEvent::ToolResult { content, .. } => {
                let truncated = util::truncate(&content, 100);
                self.stdout.execute(style::PrintStyledContent(
                    format!("{}\n", util::indent(&truncated, 6)).dark_grey(),
                ))?;
            }

            AgentEvent::Error(err) => {
                self.stdout.execute(style::PrintStyledContent(
                    format!("\n[error] {}\n", err).red(),
                ))?;
            }

            AgentEvent::Done => {
                print!("\n>>> ");
                self.stdout.flush()?;
            }
        }

        Ok(())
    }
}
