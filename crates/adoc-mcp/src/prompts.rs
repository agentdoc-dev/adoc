use rmcp::model::{
    GetPromptResult, JsonObject, ListPromptsResult, Prompt, PromptArgument, PromptMessage,
    PromptMessageRole,
};

#[derive(Debug, Clone, Copy)]
struct PromptSpec {
    name: &'static str,
    canonical_name: &'static str,
    description: &'static str,
    body: &'static str,
    arguments: fn() -> Vec<PromptArgument>,
}

const ANSWER_BODY: &str = r#"Use AgentDoc V2.2 retrieval before answering.

Workflow:
1. Call adoc_project_status with refresh "none".
2. If retrieval is not ready, call adoc_project_status with refresh "check" or "build".
3. Call adoc_search for candidate records; use adoc_why for exact Object ID records.
4. Use adoc_graph when relation context matters.
5. Answer with citations that include Object ID, kind, status, owner when present, evidence fields, caveats, diagnostics, and stale-artifact warnings.

Do not answer from private graph/search DTOs or uncited source text when a retrieval record is available."#;

const PATCH_BODY: &str = r#"Use AgentDoc V2.2 patch validation before proposing source changes.

Workflow:
1. Inspect readiness with adoc_project_status.
2. Retrieve the target Object ID with adoc_why or adoc_search.
3. Build a single-operation adoc.patch.v0 JSON proposal using replace_body, update_fields, create_object, supersede, or revoke; include reason and current base_hash when updating existing knowledge.
4. Validate the inline patch with adoc_patch_check.
5. Report validity, diagnostics, affected relations, diffs, and proof obligations.

Do not apply patches, rewrite AgentDoc Source, approve knowledge, or create hosted review state."#;

const STATUS_BODY: &str = r#"Inspect an AgentDoc V2.2 project before retrieval or patch validation.

Call adoc_project_status with the requested refresh value. Use refresh "none" for read-only inspection, "check" for validation diagnostics without writes, and "build" when artifacts must be created or refreshed. Explain config discovery, resolved paths, artifact load status, graph/search schema versions, object count, and readiness for retrieval, semantic search, and patch validation."#;

const DOGFOOD_BODY: &str = r#"Run the AgentDoc V2.2 billing pilot dogfood flow.

Use examples/billing-pilot as the project root. Inspect project status, refresh with check or build when needed, search for billing evidence, fetch exact records with adoc_why, traverse related context with adoc_graph, answer with Object ID citations, and validate any inline adoc.patch.v0 proposal with adoc_patch_check."#;

const PROMPTS: &[PromptSpec] = &[
    PromptSpec {
        name: "adoc_answer_with_citations_v0",
        canonical_name: "adoc_answer_with_citations_v0",
        description: "Answer an AgentDoc question with retrieval citations.",
        body: ANSWER_BODY,
        arguments: answer_arguments,
    },
    PromptSpec {
        name: "adoc_answer_with_citations",
        canonical_name: "adoc_answer_with_citations_v0",
        description: "Pinned alias for adoc_answer_with_citations_v0.",
        body: ANSWER_BODY,
        arguments: answer_arguments,
    },
    PromptSpec {
        name: "adoc_propose_patch_v0",
        canonical_name: "adoc_propose_patch_v0",
        description: "Propose and validate an AgentDoc patch.",
        body: PATCH_BODY,
        arguments: patch_arguments,
    },
    PromptSpec {
        name: "adoc_propose_patch",
        canonical_name: "adoc_propose_patch_v0",
        description: "Pinned alias for adoc_propose_patch_v0.",
        body: PATCH_BODY,
        arguments: patch_arguments,
    },
    PromptSpec {
        name: "adoc_inspect_project_status_v0",
        canonical_name: "adoc_inspect_project_status_v0",
        description: "Inspect AgentDoc project readiness.",
        body: STATUS_BODY,
        arguments: status_arguments,
    },
    PromptSpec {
        name: "adoc_inspect_project_status",
        canonical_name: "adoc_inspect_project_status_v0",
        description: "Pinned alias for adoc_inspect_project_status_v0.",
        body: STATUS_BODY,
        arguments: status_arguments,
    },
    PromptSpec {
        name: "adoc_dogfood_billing_pilot_v0",
        canonical_name: "adoc_dogfood_billing_pilot_v0",
        description: "Run the billing pilot dogfood workflow.",
        body: DOGFOOD_BODY,
        arguments: dogfood_arguments,
    },
    PromptSpec {
        name: "adoc_dogfood_billing_pilot",
        canonical_name: "adoc_dogfood_billing_pilot_v0",
        description: "Pinned alias for adoc_dogfood_billing_pilot_v0.",
        body: DOGFOOD_BODY,
        arguments: dogfood_arguments,
    },
];

pub fn list() -> Vec<Prompt> {
    PROMPTS
        .iter()
        .map(|spec| Prompt::new(spec.name, Some(spec.description), Some((spec.arguments)())))
        .collect()
}

pub fn list_result() -> ListPromptsResult {
    ListPromptsResult::with_all_items(list())
}

pub fn get(name: &str, arguments: Option<JsonObject>) -> Option<GetPromptResult> {
    let spec = PROMPTS.iter().find(|spec| spec.name == name)?;
    let canonical = PROMPTS
        .iter()
        .find(|candidate| candidate.name == spec.canonical_name)
        .unwrap_or(spec);
    let mut text = canonical.body.to_string();
    if let Some(arguments) = arguments.filter(|arguments| !arguments.is_empty()) {
        text.push_str("\n\nProvided arguments:\n");
        for (key, value) in arguments {
            text.push_str("- ");
            text.push_str(&key);
            text.push_str(": ");
            text.push_str(&value.to_string());
            text.push('\n');
        }
    }

    Some(
        GetPromptResult::new(vec![PromptMessage::new_text(PromptMessageRole::User, text)])
            .with_description(canonical.description),
    )
}

fn arg(name: &str, description: &str, required: bool) -> PromptArgument {
    PromptArgument::new(name)
        .with_description(description)
        .with_required(required)
}

fn answer_arguments() -> Vec<PromptArgument> {
    vec![
        arg(
            "query",
            "User question to answer from AgentDoc retrieval records.",
            true,
        ),
        arg(
            "retrieval_mode",
            "String; allowed values: hybrid, semantic, lexical.",
            false,
        ),
        arg(
            "project_root",
            "Optional AgentDoc project root path.",
            false,
        ),
        arg("top", "String integer result count for adoc_search.", false),
    ]
}

fn patch_arguments() -> Vec<PromptArgument> {
    vec![
        arg("target_object_id", "Object ID to change or create.", true),
        arg(
            "change_intent",
            "Natural-language description of the proposed change.",
            true,
        ),
        arg(
            "patch_operation",
            "String; allowed values: replace_body, update_fields, create_object, supersede, revoke.",
            false,
        ),
        arg(
            "project_root",
            "Optional AgentDoc project root path.",
            false,
        ),
    ]
}

fn status_arguments() -> Vec<PromptArgument> {
    vec![
        arg(
            "refresh",
            "String; allowed values: none, check, build.",
            false,
        ),
        arg(
            "no_embeddings",
            "String boolean; allowed values: true, false.",
            false,
        ),
        arg(
            "project_root",
            "Optional AgentDoc project root path.",
            false,
        ),
    ]
}

fn dogfood_arguments() -> Vec<PromptArgument> {
    vec![
        arg(
            "focus",
            "Billing topic, Object ID, or patch intent to dogfood.",
            false,
        ),
        arg(
            "refresh",
            "String; allowed values: none, check, build.",
            false,
        ),
    ]
}
