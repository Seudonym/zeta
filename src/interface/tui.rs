use color_eyre::eyre::Result;
use crossterm::style::Stylize;
use crossterm::{ExecutableCommand, cursor, style, terminal};
use rig::agent::Agent;
use rig::completion::CompletionModel;
use std::io::{self, Stdout, Write};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::agent::runtime::{AgentEvent, AgentRuntime};
use crate::util;

const BANNER: &str = "
 ▄▄▄▄▄           ▄          
  ▄█▀    ▄▄▄   ▄▄█▄▄   ▄▄▄  
 ▄▀     █▀  █    █    ▀   █ 
 █      █▀▀▀▀    █    ▄▀▀▀█ 
 ▀█▄▄   ▀█▄▄▀    ▀▄▄  ▀▄▄▀█ 
     █                      ";

#[derive(Error, Debug)]
enum CliError {
    #[error("IO Error")]
    IoError(#[from] std::io::Error),

    #[error("Mpsc Error")]
    MpscError(#[from] mpsc::error::SendError<String>),
}
pub struct Tui {
    event_rx: UnboundedReceiver<AgentEvent>,
    cmd_tx: UnboundedSender<String>,
    stdout: Stdout,
}

impl Tui {
    pub fn with_agent<M>(agent: Agent<M>) -> Self
    where
        M: CompletionModel + Send + Sync + 'static,
    {
        let (event_tx, event_rx) = mpsc::unbounded_channel::<AgentEvent>();
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<String>();

        let mut runtime = AgentRuntime::new(agent, event_tx);

        tokio::spawn(async move {
            while let Some(input) = cmd_rx.recv().await {
                runtime.chat(input).await.ok();
            }
        });

        Self {
            event_rx,
            cmd_tx,
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

        self.cmd_tx.send(clean_input)?;
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
