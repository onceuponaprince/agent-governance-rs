use agent_governance_core::{
    planned_fanout, AgentLane, FanoutAdapter, FanoutRequest, FanoutSource,
};
use anyhow::Context;
use clap::Parser;
use std::{fs, path::PathBuf};

#[derive(Debug, Parser)]
#[command(name = "living-research")]
#[command(about = "Generate living-research fanout plans from the agent-governance app package")]
struct Args {
    #[arg(long)]
    prompt: Option<String>,
    #[arg(long)]
    file: Option<PathBuf>,
    #[arg(long, default_value = "living-research")]
    endpoint_name: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let prompt = match (args.prompt, args.file) {
        (Some(prompt), _) => prompt,
        (None, Some(path)) => {
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?
        }
        (None, None) => anyhow::bail!("provide --prompt or --file"),
    };
    let plan = planned_fanout(FanoutRequest {
        title: Some("Living research fanout".to_string()),
        prompt,
        timeout_seconds: 600,
        sources: vec![
            FanoutSource {
                lane: AgentLane::App,
                name: args.endpoint_name,
                endpoint: None,
                provider: None,
                workspace: None,
                model: None,
                adapter: FanoutAdapter::LivingResearch,
            },
            FanoutSource {
                lane: AgentLane::Browser,
                name: "browser-bot-reference".to_string(),
                endpoint: Some("http://browser-bot:5500".to_string()),
                provider: None,
                workspace: None,
                model: None,
                adapter: FanoutAdapter::BrowserBot,
            },
            FanoutSource {
                lane: AgentLane::Cli,
                name: "cli-runner-api-reference".to_string(),
                endpoint: Some("http://cli-runner:5600/exec".to_string()),
                provider: None,
                workspace: None,
                model: None,
                adapter: FanoutAdapter::CliRunnerApi,
            },
        ],
    });
    println!("{}", serde_json::to_string_pretty(&plan)?);
    Ok(())
}
