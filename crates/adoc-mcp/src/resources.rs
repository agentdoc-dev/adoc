use rmcp::model::{ReadResourceResult, Resource, ResourceContents};

#[derive(Debug, Clone, Copy)]
struct AgentResource {
    uri: &'static str,
    name: &'static str,
    title: &'static str,
    description: &'static str,
    mime_type: &'static str,
    contents: &'static str,
}

const MARKDOWN: &str = "text/markdown";
const JSON_SCHEMA: &str = "application/schema+json";

const RESOURCES: &[AgentResource] = &[
    AgentResource {
        uri: "adoc://agent/v0/usage-contract",
        name: "agent-usage-contract",
        title: "Agent Usage Contract",
        description: "V2.2 stable AgentDoc agent usage rules.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/usage-contract.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/tool-guide",
        name: "agent-tool-guide",
        title: "Agent Tool Guide",
        description: "Recommended V2.2 MCP tool order for AgentDoc.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/tool-guide.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/answer-contract",
        name: "agent-answer-contract",
        title: "Agent Answer Contract",
        description: "Citation requirements for AgentDoc answers.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/answer-contract.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/agent-instruction-guide",
        name: "agent-instruction-guide",
        title: "Agent Instruction Guide",
        description: "V5 agent_instruction objects are authored knowledge, never runtime ACLs.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/agent-instruction-guide.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/contradiction-guide",
        name: "agent-contradiction-guide",
        title: "Contradiction Guide",
        description: "V5.6 contradiction objects are manually authored cross-references linking conflicting claims.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/contradiction-guide.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/source-guide",
        name: "agent-source-guide",
        title: "Source Guide",
        description: "V5.7 source objects are reusable evidence pointers referencing external artefacts by path or URL.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/source-guide.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/api-guide",
        name: "agent-api-guide",
        title: "API Guide",
        description: "V6.5.1 api objects are typed API contracts; verified apis require schema evidence.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/api-guide.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/observation-guide",
        name: "agent-observation-guide",
        title: "Observation Guide",
        description: "V6.5.2 observation objects record findings from support, analytics, research, and ops.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/observation-guide.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/question-guide",
        name: "agent-question-guide",
        title: "Question Guide",
        description: "V6.5.3 question objects are tracked open questions; answered questions name their resolving claim/decision.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/question-guide.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/task-guide",
        name: "agent-task-guide",
        title: "Task Guide",
        description: "V6.5.4 task objects are documentation action items; open tasks with a past due date warn task.overdue.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/task-guide.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/patch-contract",
        name: "agent-patch-contract",
        title: "Agent Patch Contract",
        description: "Read-only AgentDoc patch proposal rules.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/patch-contract.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/patch-apply-guide",
        name: "agent-patch-apply-guide",
        title: "Patch Apply Guide",
        description: "V6.4 gated apply loop: propose, check, apply, re-check, cite the post-check.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/patch-apply-guide.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/project-status-guide",
        name: "agent-project-status-guide",
        title: "Project Status Guide",
        description: "How to interpret adoc.project.status.v0.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/project-status-guide.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/dogfood-billing-pilot",
        name: "agent-dogfood-billing-pilot",
        title: "Billing Pilot Dogfood",
        description: "V2.2 dogfood flow for examples/billing-pilot.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/dogfood-billing-pilot.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/review-workflow",
        name: "agent-review-workflow",
        title: "Review Workflow",
        description: "V3.6 PR-review workflow over adoc_diff and adoc_review.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/review-workflow.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/compat-guide",
        name: "agent-compat-guide",
        title: "Markdown Compatibility Guide",
        description: "V4 Markdown compatibility mode: how .md sources appear in the graph and what is citable.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/compat-guide.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/retrieval",
        name: "schema-retrieval",
        title: "Retrieval Schema Reference",
        description: "Markdown reference for adoc.retrieval.v1.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/schema/retrieval.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/graph-traversal",
        name: "schema-graph-traversal",
        title: "Graph Traversal Schema Reference",
        description: "Markdown reference for adoc.graph.traversal.v0.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/schema/graph-traversal.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/patch",
        name: "schema-patch",
        title: "Patch Schema Reference",
        description: "Markdown reference for adoc.patch.v0 and adoc.patch.check.v0.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/schema/patch.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/project-status",
        name: "schema-project-status",
        title: "Project Status Schema Reference",
        description: "Markdown reference for adoc.project.status.v0.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/schema/project-status.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/mcp-command",
        name: "schema-mcp-command",
        title: "MCP Command Schema Reference",
        description: "Markdown reference for adoc.mcp.command.v0.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/schema/mcp-command.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/diff",
        name: "schema-diff",
        title: "Diff Schema Reference",
        description: "Markdown reference for adoc.diff.v0.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/schema/diff.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/review",
        name: "schema-review",
        title: "Review Schema Reference",
        description: "Markdown reference for adoc.review.v0.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/schema/review.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/stale",
        name: "schema-stale",
        title: "Stale Query Schema Reference",
        description: "Markdown reference for adoc.stale.v0.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/schema/stale.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/contradictions",
        name: "schema-contradictions",
        title: "Contradictions Query Schema Reference",
        description: "Markdown reference for adoc.contradictions.v0.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/schema/contradictions.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/impacted",
        name: "schema-impacted",
        title: "Impacted-By Query Schema Reference",
        description: "Markdown reference for adoc.impacted.v0.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/schema/impacted.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/migrate-report",
        name: "schema-migrate-report",
        title: "Migration Report Schema Reference",
        description: "Markdown reference for adoc.migrate.report.v0.",
        mime_type: MARKDOWN,
        contents: include_str!("../../../docs/agent/v0/schema/migrate-report.md"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/retrieval-envelope.json",
        name: "schema-retrieval-envelope-json",
        title: "Retrieval Envelope JSON Schema",
        description: "JSON Schema for adoc.retrieval.v1.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/retrieval-envelope.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/retrieval-envelope.v0.json",
        name: "schema-retrieval-envelope-v0-json",
        title: "Retrieval Envelope JSON Schema (legacy v0)",
        description: "JSON Schema for the legacy adoc.retrieval.v0 envelope; superseded by adoc.retrieval.v1 (ADR-0040) but kept published.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/retrieval-envelope.v0.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/graph-traversal-envelope.json",
        name: "schema-graph-traversal-envelope-json",
        title: "Graph Traversal Envelope JSON Schema",
        description: "JSON Schema for adoc.graph.traversal.v0.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/graph-traversal-envelope.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/patch-input.json",
        name: "schema-patch-input-json",
        title: "Patch Input JSON Schema",
        description: "JSON Schema for adoc.patch.v0.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/patch-input.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/patch-check.json",
        name: "schema-patch-check-json",
        title: "Patch Check JSON Schema",
        description: "JSON Schema for adoc.patch.check.v0.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/patch-check.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/project-status.json",
        name: "schema-project-status-json",
        title: "Project Status JSON Schema",
        description: "JSON Schema for adoc.project.status.v0.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/project-status.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/mcp-command.json",
        name: "schema-mcp-command-json",
        title: "MCP Command JSON Schema",
        description: "JSON Schema for adoc.mcp.command.v0.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/mcp-command.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/adoc.diff.v0.schema.json",
        name: "schema-adoc-diff-v0-json",
        title: "Object Diff JSON Schema",
        description: "JSON Schema for adoc.diff.v0.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/adoc.diff.v0.schema.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/adoc.review.v0.schema.json",
        name: "schema-adoc-review-v0-json",
        title: "Review Report JSON Schema",
        description: "JSON Schema for adoc.review.v0.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/adoc.review.v0.schema.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/adoc.stale.v0.schema.json",
        name: "schema-adoc-stale-v0-json",
        title: "Stale Query JSON Schema",
        description: "JSON Schema for adoc.stale.v0.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/adoc.stale.v0.schema.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/adoc.contradictions.v0.schema.json",
        name: "schema-adoc-contradictions-v0-json",
        title: "Contradictions Query JSON Schema",
        description: "JSON Schema for adoc.contradictions.v0.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/adoc.contradictions.v0.schema.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/adoc.impacted.v0.schema.json",
        name: "schema-adoc-impacted-v0-json",
        title: "Impacted-By Query JSON Schema",
        description: "JSON Schema for adoc.impacted.v0.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/adoc.impacted.v0.schema.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/adoc.patch.apply.v0.schema.json",
        name: "schema-adoc-patch-apply-v0-json",
        title: "Patch Apply JSON Schema",
        description: "JSON Schema for adoc.patch.apply.v0.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/adoc.patch.apply.v0.schema.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/adoc.migrate.report.v0.schema.json",
        name: "schema-adoc-migrate-report-v0-json",
        title: "Migration Report JSON Schema",
        description: "JSON Schema for adoc.migrate.report.v0.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/adoc.migrate.report.v0.schema.json"),
    },
    AgentResource {
        uri: "adoc://agent/v0/schema/search-artifact.json",
        name: "schema-search-artifact-json",
        title: "Search Artifact JSON Schema",
        description: "JSON Schema for adoc.search.v1, the dist/docs.search.json wire shape. The artifact itself is a build output, not an MCP resource.",
        mime_type: JSON_SCHEMA,
        contents: include_str!("../../../docs/agent/v0/schema/search-artifact.json"),
    },
];

pub fn list() -> Vec<Resource> {
    RESOURCES
        .iter()
        .map(|resource| {
            Resource::new(resource.uri, resource.name)
                .with_title(resource.title)
                .with_description(resource.description)
                .with_mime_type(resource.mime_type)
                .with_size(resource.contents.len() as u64)
        })
        .collect()
}

pub fn read(uri: &str) -> Option<ReadResourceResult> {
    RESOURCES
        .iter()
        .find(|resource| resource.uri == uri)
        .map(|resource| {
            ReadResourceResult::new(vec![
                ResourceContents::text(resource.contents, resource.uri)
                    .with_mime_type(resource.mime_type),
            ])
        })
}
