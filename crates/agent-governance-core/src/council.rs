use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path};
use uuid::Uuid;

const DEFAULT_COUNCIL_PACK_JSON: &str = include_str!("default_council.json");

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CouncilMode {
    #[default]
    Quick,
    Duo,
    Full,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentLane {
    Cli,
    Browser,
    Api,
    App,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilTarget {
    pub lane: AgentLane,
    pub name: Option<String>,
    pub provider: Option<String>,
    pub workspace: Option<String>,
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilLlmProfile {
    pub name: String,
    pub lane: AgentLane,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub workspace: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilMemberSpec {
    pub name: String,
    pub persona: Option<String>,
    pub domain: Option<String>,
    pub polarity: Option<String>,
    #[serde(default)]
    pub expertise: Vec<String>,
    #[serde(default)]
    pub principles: Vec<String>,
    pub style: Option<String>,
    pub stance: Option<String>,
    pub llm: Option<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    pub target: Option<CouncilTarget>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilMember {
    pub name: String,
    pub persona: String,
    pub domain: String,
    pub polarity: String,
    #[serde(default)]
    pub expertise: Vec<String>,
    #[serde(default)]
    pub principles: Vec<String>,
    pub style: Option<String>,
    pub stance: Option<String>,
    pub llm: Option<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    pub target: Option<CouncilTarget>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilDomain {
    pub name: String,
    #[serde(default)]
    pub members: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromptScope {
    Member,
    Coordinator,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilPromptTemplate {
    pub name: String,
    pub round: String,
    pub scope: PromptScope,
    #[serde(default)]
    pub modes: Vec<CouncilMode>,
    pub template: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct CouncilPack {
    #[serde(default)]
    pub members: Vec<CouncilMember>,
    #[serde(default)]
    pub domains: Vec<CouncilDomain>,
    #[serde(default)]
    pub llms: Vec<CouncilLlmProfile>,
    pub default_llm: Option<String>,
    #[serde(default)]
    pub prompt_templates: Vec<CouncilPromptTemplate>,
    #[serde(default)]
    pub custom_member_fields: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilMemberPosition {
    pub member: String,
    pub position: String,
    pub vote: Option<String>,
    pub confidence: Option<f32>,
    #[serde(default)]
    pub dissent: Vec<String>,
    #[serde(default)]
    pub unresolved_questions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilRequest {
    pub title: Option<String>,
    pub problem: String,
    #[serde(default)]
    pub mode: CouncilMode,
    pub domain: Option<String>,
    pub members: Option<Vec<CouncilMemberSpec>>,
    #[serde(default)]
    pub llms: Vec<CouncilLlmProfile>,
    pub default_llm: Option<String>,
    #[serde(default)]
    pub targets: Vec<CouncilTarget>,
    #[serde(default = "default_consensus_threshold")]
    pub consensus_threshold: f32,
    #[serde(default)]
    pub positions: Vec<CouncilMemberPosition>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilRoundPrompt {
    pub member: String,
    pub round: String,
    pub prompt: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilConsensus {
    pub reached: bool,
    pub agreement_ratio: f32,
    pub threshold: f32,
    pub decision_rule: String,
    pub position: String,
    #[serde(default)]
    pub dissent: Vec<String>,
    #[serde(default)]
    pub unresolved_questions: Vec<String>,
    #[serde(default)]
    pub recommended_next_actions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilRun {
    pub council_id: Uuid,
    pub title: Option<String>,
    pub mode: CouncilMode,
    pub domain: String,
    pub problem: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub members: Vec<CouncilMember>,
    pub prompts: Vec<CouncilRoundPrompt>,
    pub positions: Vec<CouncilMemberPosition>,
    pub consensus: CouncilConsensus,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilPersonaCatalog {
    pub built_ins: Vec<CouncilMember>,
    pub triads: Vec<CouncilTriad>,
    pub llms: Vec<CouncilLlmProfile>,
    pub default_llm: Option<String>,
    pub custom_member_fields: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CouncilTriad {
    pub domain: String,
    pub members: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CouncilError {
    #[error("problem is required")]
    EmptyProblem,
    #[error("consensus_threshold must be between 0.5 and 1.0")]
    InvalidThreshold,
    #[error("at least one council member is required")]
    NoMembers,
    #[error("council pack has no domains and request did not provide members")]
    NoDomains,
    #[error("failed to load council pack: {0}")]
    Pack(String),
}

#[derive(Debug, Deserialize)]
struct MarkdownCouncilDoc {
    kind: String,
    name: Option<String>,
    persona: Option<String>,
    domain: Option<String>,
    polarity: Option<String>,
    #[serde(default)]
    expertise: Vec<String>,
    #[serde(default)]
    principles: Vec<String>,
    style: Option<String>,
    stance: Option<String>,
    llm: Option<String>,
    lane: Option<AgentLane>,
    provider: Option<String>,
    model: Option<String>,
    endpoint: Option<String>,
    workspace: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    #[serde(default)]
    metadata: Value,
    #[serde(default)]
    constraints: Vec<String>,
    #[serde(default)]
    members: Vec<String>,
    round: Option<String>,
    scope: Option<PromptScope>,
    #[serde(default)]
    modes: Vec<CouncilMode>,
    template: Option<String>,
}

pub fn default_council_pack() -> CouncilPack {
    load_council_pack_from_json(DEFAULT_COUNCIL_PACK_JSON)
        .expect("bundled default council pack must be valid JSON")
}

pub fn load_council_pack_from_json(text: &str) -> Result<CouncilPack, CouncilError> {
    serde_json::from_str(text).map_err(|err| CouncilError::Pack(err.to_string()))
}

pub fn load_council_pack_from_markdown(text: &str) -> Result<CouncilPack, CouncilError> {
    let (frontmatter, body) = split_frontmatter(text)?;
    let doc: MarkdownCouncilDoc =
        serde_yaml::from_str(frontmatter).map_err(|err| CouncilError::Pack(err.to_string()))?;
    let mut pack = CouncilPack::default();
    match doc.kind.as_str() {
        "member" => {
            pack.members.push(CouncilMember {
                name: normalize_name(doc.name.as_deref().ok_or_else(|| {
                    CouncilError::Pack("member markdown requires name".to_string())
                })?),
                persona: doc.persona.or(doc.name).unwrap_or_default(),
                domain: doc.domain.unwrap_or_default(),
                polarity: doc
                    .polarity
                    .unwrap_or_else(|| "independent reasoning".to_string()),
                expertise: doc.expertise,
                principles: doc.principles,
                style: doc.style,
                stance: doc.stance,
                llm: doc.llm,
                constraints: doc.constraints,
                target: None,
            })
        }
        "llm" => pack.llms.push(CouncilLlmProfile {
            name: doc
                .name
                .ok_or_else(|| CouncilError::Pack("llm markdown requires name".to_string()))?,
            lane: doc.lane.unwrap_or(AgentLane::Api),
            provider: doc.provider,
            model: doc.model,
            endpoint: doc.endpoint,
            workspace: doc.workspace,
            temperature: doc.temperature,
            max_tokens: doc.max_tokens,
            metadata: doc.metadata,
        }),
        "domain" => pack.domains.push(CouncilDomain {
            name: doc
                .name
                .ok_or_else(|| CouncilError::Pack("domain markdown requires name".to_string()))?,
            members: doc
                .members
                .into_iter()
                .map(|name| normalize_name(&name))
                .collect(),
        }),
        "template" => pack.prompt_templates.push(CouncilPromptTemplate {
            name: doc
                .name
                .unwrap_or_else(|| doc.round.clone().unwrap_or_else(|| "template".to_string())),
            round: doc.round.unwrap_or_else(|| "round".to_string()),
            scope: doc.scope.unwrap_or(PromptScope::Member),
            modes: doc.modes,
            template: doc.template.unwrap_or_else(|| body.trim().to_string()),
        }),
        other => {
            return Err(CouncilError::Pack(format!(
                "unknown markdown council doc kind: {other}"
            )))
        }
    }
    Ok(pack)
}

pub fn load_council_pack_from_path(path: impl AsRef<Path>) -> Result<CouncilPack, CouncilError> {
    let path = path.as_ref();
    if path.is_dir() {
        let mut entries = fs::read_dir(path)
            .map_err(|err| CouncilError::Pack(err.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| CouncilError::Pack(err.to_string()))?;
        entries.sort_by_key(|entry| entry.path());
        let mut pack = CouncilPack::default();
        for entry in entries {
            let item = entry.path();
            if item.is_dir() {
                merge_pack(&mut pack, load_council_pack_from_path(&item)?);
                continue;
            }
            match item.extension().and_then(|ext| ext.to_str()) {
                Some("json") => merge_pack(
                    &mut pack,
                    load_council_pack_from_json(&read_to_string(&item)?)?,
                ),
                Some("md") | Some("markdown") => merge_pack(
                    &mut pack,
                    load_council_pack_from_markdown(&read_to_string(&item)?)?,
                ),
                _ => {}
            }
        }
        Ok(pack)
    } else {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => load_council_pack_from_json(&read_to_string(path)?),
            Some("md") | Some("markdown") => {
                load_council_pack_from_markdown(&read_to_string(path)?)
            }
            _ => Err(CouncilError::Pack(
                "council pack path must be .json, .md, or a directory".to_string(),
            )),
        }
    }
}

pub fn persona_catalog() -> CouncilPersonaCatalog {
    persona_catalog_from_pack(&default_council_pack())
}

pub fn persona_catalog_from_pack(pack: &CouncilPack) -> CouncilPersonaCatalog {
    CouncilPersonaCatalog {
        built_ins: pack.members.clone(),
        triads: pack
            .domains
            .iter()
            .map(|domain| CouncilTriad {
                domain: domain.name.clone(),
                members: domain.members.clone(),
            })
            .collect(),
        llms: pack.llms.clone(),
        default_llm: pack.default_llm.clone(),
        custom_member_fields: pack.custom_member_fields.clone(),
    }
}

pub fn deliberate(req: CouncilRequest) -> Result<CouncilRun, CouncilError> {
    deliberate_with_pack(req, &default_council_pack())
}

pub fn deliberate_with_pack(
    req: CouncilRequest,
    pack: &CouncilPack,
) -> Result<CouncilRun, CouncilError> {
    if req.problem.trim().is_empty() {
        return Err(CouncilError::EmptyProblem);
    }
    if !(0.5..=1.0).contains(&req.consensus_threshold) {
        return Err(CouncilError::InvalidThreshold);
    }
    let domain = selected_domain(&req, pack)?;
    let members = resolve_members(&req, pack, &domain)?;
    let prompts = build_prompts(&req, pack, &members);
    let consensus = build_consensus(&req, &members);
    let status = if consensus.reached {
        "completed"
    } else if req.positions.is_empty() {
        "awaiting_positions"
    } else {
        "completed_no_consensus"
    };
    Ok(CouncilRun {
        council_id: Uuid::new_v4(),
        title: req.title,
        mode: req.mode,
        domain,
        problem: req.problem,
        status: status.to_string(),
        created_at: Utc::now(),
        members,
        prompts,
        positions: req.positions,
        consensus,
    })
}

pub fn render_council_report(run: &CouncilRun) -> String {
    let mut lines = vec![
        format!(
            "# Council Report: {}",
            run.title.as_deref().unwrap_or("Untitled")
        ),
        String::new(),
        format!("- Council id: `{}`", run.council_id),
        format!("- Status: `{}`", run.status),
        format!("- Domain: `{}`", run.domain),
        format!("- Created: `{}`", run.created_at),
        format!("- Consensus reached: `{}`", run.consensus.reached),
        format!("- Agreement ratio: `{}`", run.consensus.agreement_ratio),
        format!("- Threshold: `{}`", run.consensus.threshold),
        String::new(),
        "## Problem".to_string(),
        String::new(),
        run.problem.clone(),
        String::new(),
        "## Decision".to_string(),
        String::new(),
        run.consensus.position.clone(),
        String::new(),
    ];
    if !run.consensus.dissent.is_empty() {
        lines.extend(["## Dissent".to_string(), String::new()]);
        lines.extend(run.consensus.dissent.iter().map(|item| format!("- {item}")));
        lines.push(String::new());
    }
    if !run.consensus.unresolved_questions.is_empty() {
        lines.extend(["## Unresolved Questions".to_string(), String::new()]);
        lines.extend(
            run.consensus
                .unresolved_questions
                .iter()
                .map(|item| format!("- {item}")),
        );
        lines.push(String::new());
    }
    lines.extend(["## Recommended Next Actions".to_string(), String::new()]);
    lines.extend(
        run.consensus
            .recommended_next_actions
            .iter()
            .map(|item| format!("- {item}")),
    );
    lines.push(String::new());
    lines.join("\n")
}

fn split_frontmatter(text: &str) -> Result<(&str, &str), CouncilError> {
    let rest = text.strip_prefix("---\n").ok_or_else(|| {
        CouncilError::Pack("markdown council docs require YAML frontmatter".to_string())
    })?;
    let (frontmatter, body) = rest.split_once("\n---").ok_or_else(|| {
        CouncilError::Pack("markdown council docs require closing frontmatter".to_string())
    })?;
    Ok((frontmatter, body.trim_start_matches(['\n', '\r'])))
}

fn read_to_string(path: &Path) -> Result<String, CouncilError> {
    fs::read_to_string(path).map_err(|err| CouncilError::Pack(format!("{}: {err}", path.display())))
}

fn merge_pack(pack: &mut CouncilPack, other: CouncilPack) {
    pack.members.extend(other.members);
    pack.domains.extend(other.domains);
    pack.llms.extend(other.llms);
    if pack.default_llm.is_none() {
        pack.default_llm = other.default_llm;
    }
    pack.prompt_templates.extend(other.prompt_templates);
    if pack.custom_member_fields.is_empty() {
        pack.custom_member_fields = other.custom_member_fields;
    } else {
        pack.custom_member_fields.extend(other.custom_member_fields);
        pack.custom_member_fields.sort();
        pack.custom_member_fields.dedup();
    }
}

fn default_consensus_threshold() -> f32 {
    0.67
}

fn selected_domain(req: &CouncilRequest, pack: &CouncilPack) -> Result<String, CouncilError> {
    if let Some(domain) = &req.domain {
        return Ok(domain.clone());
    }
    pack.domains
        .first()
        .map(|domain| domain.name.clone())
        .ok_or(CouncilError::NoDomains)
}

fn resolve_members(
    req: &CouncilRequest,
    pack: &CouncilPack,
    domain: &str,
) -> Result<Vec<CouncilMember>, CouncilError> {
    let specs = match &req.members {
        Some(members) if !members.is_empty() => members.clone(),
        _ => default_specs(pack, domain, &req.mode)?,
    };
    if specs.is_empty() {
        return Err(CouncilError::NoMembers);
    }
    let mut out = Vec::with_capacity(specs.len());
    for (idx, spec) in specs.into_iter().enumerate() {
        let key = normalize_name(&spec.name);
        let base = pack.members.iter().find(|m| m.name == key);
        let explicit_llm = spec
            .llm
            .clone()
            .or_else(|| base.and_then(|m| m.llm.clone()));
        let fallback_llm = req.default_llm.clone().or_else(|| pack.default_llm.clone());
        let request_target = req.targets.get(idx % req.targets.len().max(1)).cloned();
        let target = spec
            .target
            .clone()
            .or_else(|| {
                explicit_llm
                    .as_deref()
                    .and_then(|name| llm_target(name, pack, req))
            })
            .or_else(|| base.and_then(|m| m.target.clone()))
            .or(request_target.clone())
            .or_else(|| {
                fallback_llm
                    .as_deref()
                    .and_then(|name| llm_target(name, pack, req))
            });
        let llm =
            explicit_llm.or_else(|| request_target.is_none().then_some(fallback_llm).flatten());
        out.push(CouncilMember {
            name: key,
            persona: spec
                .persona
                .clone()
                .or_else(|| base.map(|m| m.persona.clone()))
                .unwrap_or(spec.name.clone()),
            domain: spec
                .domain
                .clone()
                .or_else(|| base.map(|m| m.domain.clone()))
                .unwrap_or_else(|| domain.to_string()),
            polarity: spec
                .polarity
                .clone()
                .or_else(|| base.map(|m| m.polarity.clone()))
                .unwrap_or_else(|| "independent reasoning".to_string()),
            expertise: prefer_vec(spec.expertise, base.map(|m| m.expertise.clone())),
            principles: prefer_vec(spec.principles, base.map(|m| m.principles.clone())),
            style: spec
                .style
                .clone()
                .or_else(|| base.and_then(|m| m.style.clone())),
            stance: spec
                .stance
                .clone()
                .or_else(|| base.and_then(|m| m.stance.clone())),
            llm,
            constraints: prefer_vec(spec.constraints, base.map(|m| m.constraints.clone())),
            target,
        });
    }
    Ok(out)
}

fn llm_target(name: &str, pack: &CouncilPack, req: &CouncilRequest) -> Option<CouncilTarget> {
    req.llms
        .iter()
        .chain(pack.llms.iter())
        .find(|profile| profile.name == name)
        .map(|profile| CouncilTarget {
            lane: profile.lane.clone(),
            name: Some(profile.name.clone()),
            provider: profile.provider.clone(),
            workspace: profile.workspace.clone(),
            model: profile.model.clone(),
            endpoint: profile.endpoint.clone(),
            temperature: profile.temperature,
            max_tokens: profile.max_tokens,
            metadata: profile.metadata.clone(),
        })
}

fn default_specs(
    pack: &CouncilPack,
    domain: &str,
    mode: &CouncilMode,
) -> Result<Vec<CouncilMemberSpec>, CouncilError> {
    let mut names = pack
        .domains
        .iter()
        .find(|item| item.name == domain)
        .or_else(|| pack.domains.first())
        .ok_or(CouncilError::NoDomains)?
        .members
        .clone();
    match mode {
        CouncilMode::Duo => names.truncate(2),
        CouncilMode::Full => {
            names = pack
                .members
                .iter()
                .map(|member| member.name.clone())
                .collect()
        }
        CouncilMode::Quick => {}
    }
    Ok(names
        .into_iter()
        .map(|name| CouncilMemberSpec {
            name,
            persona: None,
            domain: None,
            polarity: None,
            expertise: vec![],
            principles: vec![],
            style: None,
            stance: None,
            llm: None,
            constraints: vec![],
            target: None,
        })
        .collect())
}

fn build_prompts(
    req: &CouncilRequest,
    pack: &CouncilPack,
    members: &[CouncilMember],
) -> Vec<CouncilRoundPrompt> {
    let mut prompts = Vec::new();
    for template in pack
        .prompt_templates
        .iter()
        .filter(|template| template_applies(template, &req.mode))
    {
        match template.scope {
            PromptScope::Member => {
                for member in members {
                    prompts.push(CouncilRoundPrompt {
                        member: member.name.clone(),
                        round: template.round.clone(),
                        prompt: render_template(&template.template, req, Some(member), members),
                    });
                }
            }
            PromptScope::Coordinator => prompts.push(CouncilRoundPrompt {
                member: "coordinator".to_string(),
                round: template.round.clone(),
                prompt: render_template(&template.template, req, None, members),
            }),
        }
    }
    prompts
}

fn template_applies(template: &CouncilPromptTemplate, mode: &CouncilMode) -> bool {
    template.modes.is_empty() || template.modes.iter().any(|item| item == mode)
}

fn render_template(
    template: &str,
    req: &CouncilRequest,
    member: Option<&CouncilMember>,
    members: &[CouncilMember],
) -> String {
    let mut out = template.to_string();
    let domain = req.domain.as_deref().unwrap_or("");
    let member_profile = member.map(persona_profile).unwrap_or_default();
    let member_summary = member_summary(members);
    let replacements = [
        ("{{problem}}", req.problem.as_str()),
        ("{{domain}}", domain),
        ("{{persona_profile}}", member_profile.as_str()),
        ("{{member_summary}}", member_summary.as_str()),
        (
            "{{member.name}}",
            member.map(|m| m.name.as_str()).unwrap_or(""),
        ),
        (
            "{{member.persona}}",
            member.map(|m| m.persona.as_str()).unwrap_or(""),
        ),
        (
            "{{member.domain}}",
            member.map(|m| m.domain.as_str()).unwrap_or(""),
        ),
        (
            "{{member.polarity}}",
            member.map(|m| m.polarity.as_str()).unwrap_or(""),
        ),
    ];
    for (needle, value) in replacements {
        out = out.replace(needle, value);
    }
    out = out.replace(
        "{{consensus_threshold}}",
        &req.consensus_threshold.to_string(),
    );
    out
}

fn build_consensus(req: &CouncilRequest, members: &[CouncilMember]) -> CouncilConsensus {
    let member_names: BTreeSet<_> = members.iter().map(|m| m.name.as_str()).collect();
    let usable_positions: Vec<_> = req
        .positions
        .iter()
        .filter(|p| member_names.contains(normalize_name(&p.member).as_str()))
        .collect();
    let yes_count = usable_positions
        .iter()
        .filter(|p| vote_is_yes(p.vote.as_deref()))
        .count();
    let agreement_ratio = round2(yes_count as f32 / members.len().max(1) as f32);
    let reached = !members.is_empty() && agreement_ratio >= req.consensus_threshold;
    let position = if usable_positions.is_empty() {
        "Awaiting member positions. Use the generated prompts to collect independent analyses, then resubmit with positions[].".to_string()
    } else if reached {
        synthesize_position(&usable_positions)
    } else {
        format!(
            "No consensus reached. {yes_count}/{} member(s) voted yes; threshold is {}.",
            members.len(),
            req.consensus_threshold
        )
    };
    let mut dissent = Vec::new();
    let mut unresolved = Vec::new();
    for p in &usable_positions {
        for item in &p.dissent {
            dissent.push(format!("{}: {}", normalize_name(&p.member), item));
        }
        if !vote_is_yes(p.vote.as_deref()) && p.dissent.is_empty() {
            dissent.push(format!(
                "{}: vote={}",
                normalize_name(&p.member),
                p.vote.clone().unwrap_or_else(|| "unspecified".to_string())
            ));
        }
        for item in &p.unresolved_questions {
            unresolved.push(format!("{}: {}", normalize_name(&p.member), item));
        }
    }
    let mut actions = vec![
        "Persist the council_id with the decision record before acting.".to_string(),
        "Resolve unresolved questions before irreversible or high-cost actions.".to_string(),
    ];
    if req.positions.is_empty() {
        actions.push(
            "Dispatch each generated prompt to its configured target and resubmit positions."
                .to_string(),
        );
    } else if !reached {
        actions
            .push("Run another council with a narrower problem or different personas.".to_string());
    }
    CouncilConsensus {
        reached,
        agreement_ratio,
        threshold: req.consensus_threshold,
        decision_rule: "yes_votes_over_selected_members".to_string(),
        position,
        dissent,
        unresolved_questions: unresolved,
        recommended_next_actions: actions,
    }
}

fn synthesize_position(positions: &[&CouncilMemberPosition]) -> String {
    let mut out = String::from("Consensus reached. Shared position:\n");
    for p in positions.iter().filter(|p| vote_is_yes(p.vote.as_deref())) {
        out.push_str("- ");
        out.push_str(&normalize_name(&p.member));
        out.push_str(": ");
        out.push_str(p.position.trim());
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn persona_profile(member: &CouncilMember) -> String {
    let mut lines = vec![
        format!("Member: {} ({})", member.persona, member.name),
        format!("Domain: {}", member.domain),
        format!("Polarity: {}", member.polarity),
    ];
    if !member.expertise.is_empty() {
        lines.push(format!("Expertise: {}", member.expertise.join("; ")));
    }
    if !member.principles.is_empty() {
        lines.push(format!(
            "Operating principles: {}",
            member.principles.join("; ")
        ));
    }
    if let Some(style) = &member.style {
        lines.push(format!("Communication style: {style}"));
    }
    if let Some(stance) = &member.stance {
        lines.push(format!("Default stance: {stance}"));
    }
    if !member.constraints.is_empty() {
        lines.push(format!("Constraints: {}", member.constraints.join("; ")));
    }
    lines.join("\n")
}

fn member_summary(members: &[CouncilMember]) -> String {
    members
        .iter()
        .map(|m| {
            let target = m
                .target
                .as_ref()
                .and_then(|t| {
                    t.name
                        .as_ref()
                        .or(t.provider.as_ref())
                        .or(t.workspace.as_ref())
                })
                .cloned()
                .unwrap_or_else(|| "none".to_string());
            let model = m
                .target
                .as_ref()
                .and_then(|t| t.model.as_ref())
                .cloned()
                .unwrap_or_else(|| "none".to_string());
            let llm = m.llm.as_deref().unwrap_or("none");
            format!(
                "- {}: {}; {}; {}; llm={}; target={}; model={}",
                m.name, m.persona, m.domain, m.polarity, llm, target, model
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_name(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace(['_', ' '], "-")
}

fn prefer_vec(primary: Vec<String>, fallback: Option<Vec<String>>) -> Vec<String> {
    if primary.is_empty() {
        fallback.unwrap_or_default()
    } else {
        primary
    }
}

fn vote_is_yes(vote: Option<&str>) -> bool {
    matches!(vote.map(|v| v.trim().to_ascii_lowercase()), Some(v) if matches!(v.as_str(), "yes" | "approve" | "agree" | "consensus"))
}

fn round2(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api_target() -> CouncilTarget {
        CouncilTarget {
            lane: AgentLane::Api,
            name: None,
            provider: Some("openrouter".to_string()),
            workspace: None,
            model: None,
            endpoint: None,
            temperature: None,
            max_tokens: None,
            metadata: Value::Null,
        }
    }

    #[test]
    fn catalog_loads_from_json_pack() {
        let pack = default_council_pack();
        assert_eq!(pack.members.len(), 18);
        assert!(pack
            .domains
            .iter()
            .any(|domain| domain.name == "living-research"));
        assert!(!pack.prompt_templates.is_empty());
    }

    #[test]
    fn markdown_frontmatter_loads_template() {
        let pack = load_council_pack_from_markdown("---\nkind: template\nname: test\nround: independent\nscope: member\nmodes: [quick]\n---\nHello {{member.name}}: {{problem}}\n").unwrap();
        assert_eq!(pack.prompt_templates[0].round, "independent");
        assert!(pack.prompt_templates[0].template.contains("{{problem}}"));
    }

    #[test]
    fn custom_personas_are_injected_into_dynamic_prompts() {
        let req = CouncilRequest {
            title: Some("Custom".to_string()),
            problem: "How should agents coordinate work?".to_string(),
            mode: CouncilMode::Duo,
            domain: Some("architecture".to_string()),
            llms: vec![],
            default_llm: None,
            targets: vec![api_target()],
            consensus_threshold: 0.67,
            positions: vec![],
            members: Some(vec![CouncilMemberSpec {
                name: "ops-steward".to_string(),
                persona: Some("Ops Steward".to_string()),
                domain: Some("deployment reliability".to_string()),
                polarity: Some("protects uptime and rollback paths".to_string()),
                expertise: vec!["incident response".to_string(), "release gates".to_string()],
                principles: vec!["prefer reversible changes".to_string()],
                style: Some("terse operational checklist".to_string()),
                stance: Some("skeptical until health checks pass".to_string()),
                llm: None,
                constraints: vec!["must preserve existing auth boundary".to_string()],
                target: None,
            }]),
        };
        let run = deliberate(req).unwrap();
        assert_eq!(run.status, "awaiting_positions");
        let joined = run
            .prompts
            .iter()
            .map(|p| p.prompt.as_str())
            .collect::<Vec<_>>()
            .join("\n---\n");
        assert!(joined.contains("Member: Ops Steward (ops-steward)"));
        assert!(joined.contains("Expertise: incident response; release gates"));
    }

    #[test]
    fn llm_profiles_resolve_to_member_targets() {
        let mut pack = default_council_pack();
        pack.default_llm = Some("fast-api".to_string());
        pack.llms.push(CouncilLlmProfile {
            name: "fast-api".to_string(),
            lane: AgentLane::Api,
            provider: Some("openrouter".to_string()),
            model: Some("neutral-test-model".to_string()),
            endpoint: Some("https://openrouter.ai/api/v1".to_string()),
            workspace: None,
            temperature: Some(0.2),
            max_tokens: Some(2048),
            metadata: Value::Null,
        });
        let run = deliberate_with_pack(
            CouncilRequest {
                title: None,
                problem: "Which LLM should this agent use?".to_string(),
                mode: CouncilMode::Duo,
                domain: Some("architecture".to_string()),
                members: None,
                llms: vec![],
                default_llm: None,
                targets: vec![],
                consensus_threshold: 0.67,
                positions: vec![],
            },
            &pack,
        )
        .unwrap();
        assert_eq!(run.members[0].llm.as_deref(), Some("fast-api"));
        assert_eq!(
            run.members[0]
                .target
                .as_ref()
                .and_then(|target| target.model.as_deref()),
            Some("neutral-test-model")
        );
    }

    #[test]
    fn positions_reach_threshold_consensus() {
        let req = CouncilRequest {
            title: None,
            problem: "Ship?".to_string(),
            mode: CouncilMode::Quick,
            domain: Some("architecture".to_string()),
            members: None,
            llms: vec![],
            default_llm: None,
            targets: vec![api_target()],
            consensus_threshold: 0.67,
            positions: vec![
                CouncilMemberPosition {
                    member: "aristotle".to_string(),
                    position: "Ship reversible scope.".to_string(),
                    vote: Some("yes".to_string()),
                    confidence: Some(0.8),
                    dissent: vec![],
                    unresolved_questions: vec![],
                },
                CouncilMemberPosition {
                    member: "ada".to_string(),
                    position: "Ship behind interface.".to_string(),
                    vote: Some("yes".to_string()),
                    confidence: Some(0.75),
                    dissent: vec![],
                    unresolved_questions: vec![],
                },
                CouncilMemberPosition {
                    member: "feynman".to_string(),
                    position: "Need a smoke test first.".to_string(),
                    vote: Some("no".to_string()),
                    confidence: Some(0.6),
                    dissent: vec!["No smoke test evidence.".to_string()],
                    unresolved_questions: vec!["What failed last run?".to_string()],
                },
            ],
        };
        let run = deliberate(req).unwrap();
        assert_eq!(run.status, "completed");
        assert!(run.consensus.reached);
        assert_eq!(run.consensus.agreement_ratio, 0.67);
        assert!(run.consensus.position.contains("Ship reversible scope"));
        assert_eq!(
            run.consensus.dissent,
            vec!["feynman: No smoke test evidence."]
        );
    }
}
