use color_eyre::eyre::{Context, Result};
use rig::{client::CompletionClient, providers::deepseek, tool::ToolDyn};
use tokio::fs;

mod agent;
mod interface;
mod tools;
mod util;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .wrap_err("DEEPSEEK_API_KEY variable is missing in .envrc")?;

    let mut tools: Vec<Box<dyn ToolDyn>> = Vec::new();
    tools.extend(tools::fs::toolset());
    tools.extend(tools::shell::toolset());
    tools.extend(tools::memory::toolset());
    tools.extend(tools::web::toolset());

    let base_prompt = fs::read_to_string("./src/md/SYSTEM.md").await?;
    let system_prompt = util::construct_system_prompt(base_prompt);
    let agent = deepseek::Client::new(api_key)?
        .agent("deepseek-v4-flash")
        .tools(tools)
        .default_max_turns(10)
        .preamble(&system_prompt)
        .build();

    let runtime = std::sync::Arc::new(tokio::sync::Mutex::new(
        agent::runtime::AgentRuntime::new(agent),
    ));

    let mut tui = interface::tui::Tui::new(runtime.clone()).await;
    tui.run_tui().await?;

    Ok(())
}
