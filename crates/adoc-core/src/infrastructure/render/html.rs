use std::collections::HashSet;

use chrono::NaiveDate;

use crate::domain::ast::{BlockAst, ColumnAlignment, ListKind, UnknownExtensionKind, WorkspaceAst};
use crate::domain::graph::GraphRelationKind;
use crate::domain::inline::InlineSegment;
use crate::domain::knowledge_object::{
    KnowledgeObject, Relations,
    agent_instruction::AgentInstruction,
    claim::Evidence,
    contradiction::Contradiction,
    policy::Policy,
    procedure::ordered_step_marker_len,
    projection::{KnowledgeObjectMetadata, MetadataField},
};
use crate::domain::url_safety::verdict;
use crate::infrastructure::artifact::graph_json::derive_effective_status;

/// CSS class used to wrap **Quarantined HTML** content (block and inline) in
/// the rendered output. ADR-0023's renderer-as-security-boundary rule means
/// this class is the user-visible artifact of compat-mode raw-HTML handling;
/// authoring it here is the single source of truth. Tests pin the literal
/// string in their HTML assertions — see `crates/adoc-cli/tests/markdown_pilot.rs`.
const QUARANTINED_HTML_CLASS: &str = "quarantined-html";

/// CSS class used to wrap an inline image whose `src` URL failed the safety
/// verdict. The alt text is rendered inside the span so the document remains
/// readable; the class name is pinned by integration tests — output must not
/// change.
const QUARANTINED_IMAGE_CLASS: &str = "quarantined-image";

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct HtmlRenderer;

impl HtmlRenderer {
    /// Render the workspace HTML with the wall-clock date for lifecycle
    /// derivation. Equivalent to [`Self::render_workspace_for_date`] with
    /// `today = None`.
    #[allow(dead_code)]
    pub(crate) fn render_workspace(&self, workspace: &WorkspaceAst) -> String {
        self.render_workspace_for_date(workspace, None)
    }

    /// Render workspace HTML with a pinned `today` date so that the derived
    /// `effective_status` badge can appear in tests without relying on the wall
    /// clock. When `today` is `None` the badge is never emitted (consistent
    /// with the pre-V5.10 behaviour).
    pub(crate) fn render_workspace_for_date(
        &self,
        workspace: &WorkspaceAst,
        today: Option<NaiveDate>,
    ) -> String {
        self.render_pages_for_date(&workspace.pages, today)
    }

    #[cfg(test)]
    fn render(&self, pages: &[crate::domain::ast::PageAst]) -> String {
        self.render_pages_for_date(pages, None)
    }

    fn render_pages_for_date(
        &self,
        pages: &[crate::domain::ast::PageAst],
        today: Option<NaiveDate>,
    ) -> String {
        // V5.10 TB4: build the set of claim ids that are effectively contradicted
        // by at least one unresolved contradiction.  This is computed once at the
        // page-list level so we don't have to thread contradictions down into every
        // per-object render function.
        let contradicted_claim_ids: HashSet<String> = build_contradicted_claim_ids(pages);

        let mut html = String::from(
            "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>AgentDoc</title>\n</head>\n<body>\n",
        );

        for page in pages {
            html.push_str("<article data-page-id=\"");
            html.push_str(&escape_html(page.id.as_str()));
            html.push_str("\">\n");

            for block in &page.blocks {
                render_block(block, today, &contradicted_claim_ids, &mut html);
            }

            html.push_str("</article>\n");
        }

        html.push_str("</body>\n</html>\n");
        html
    }
}

/// Build the set of claim IDs that are effectively `contradicted` — i.e. they
/// are referenced by at least one unresolved contradiction in the workspace.
///
/// This is computed once per render call at the top level so the badge logic
/// does not need cross-page look-ups inside each per-object render function.
fn build_contradicted_claim_ids(pages: &[crate::domain::ast::PageAst]) -> HashSet<String> {
    let mut set = HashSet::new();
    for page in pages {
        for block in &page.blocks {
            let BlockAst::KnowledgeObject(ko) = block else {
                continue;
            };
            let KnowledgeObject::Contradiction(contradiction) = ko.as_ref() else {
                continue;
            };
            if !contradiction.status().is_active() {
                continue;
            }
            for claim_id in contradiction.claims().as_slice() {
                set.insert(claim_id.as_str().to_string());
            }
        }
    }
    set
}

fn render_block(
    block: &BlockAst,
    today: Option<NaiveDate>,
    contradicted_claim_ids: &HashSet<String>,
    html: &mut String,
) {
    match block {
        BlockAst::Heading(heading) => {
            let level = heading.level.clamp(1, 6);
            html.push_str(&format!("<h{level}>"));
            render_inlines(&heading.inlines, html);
            html.push_str(&format!("</h{level}>\n"));
        }
        BlockAst::Paragraph(paragraph) => {
            html.push_str("<p>");
            render_inlines(&paragraph.inlines, html);
            html.push_str("</p>\n");
        }
        BlockAst::List(list) => {
            let tag = match list.kind {
                ListKind::Ordered => "ol",
                ListKind::Unordered => "ul",
            };
            // V4.2: when any item carries a task_state, the list is a GFM
            // task list. Mark the parent so CSS can hide bullets if desired.
            let is_task_list = list.items.iter().any(|item| item.task_state.is_some());
            html.push('<');
            html.push_str(tag);
            if is_task_list {
                html.push_str(" class=\"adoc-task-list\"");
            }
            html.push_str(">\n");
            for item in &list.items {
                html.push_str("<li>");
                if let Some(checked) = item.task_state {
                    html.push_str("<input type=\"checkbox\" disabled");
                    if checked {
                        html.push_str(" checked");
                    }
                    html.push_str(" /> ");
                }
                render_inlines(&item.inlines, html);
                for child in &item.content {
                    render_block(child, today, contradicted_claim_ids, html);
                }
                html.push_str("</li>\n");
            }
            html.push_str("</");
            html.push_str(tag);
            html.push_str(">\n");
        }
        BlockAst::CodeBlock(code_block) => {
            html.push_str("<pre><code");
            if let Some(language) = &code_block.language {
                html.push_str(" class=\"language-");
                html.push_str(&escape_html(language));
                html.push('"');
            }
            html.push('>');
            html.push_str(&escape_html(&code_block.code));
            html.push_str("</code></pre>\n");
        }
        BlockAst::KnowledgeObject(ko) => {
            render_knowledge_object(ko, today, contradicted_claim_ids, html);
        }
        BlockAst::KnowledgeObjectPending(_) => {
            unreachable!("resolver must replace pending knowledge objects before rendering")
        }
        // V4 Compatibility Mode: render raw HTML from Markdown source as
        // escaped text inside a quarantine block. The browser never
        // interprets the original markup; the reader sees it as code.
        BlockAst::QuarantinedHtml(quarantined_html) => {
            html.push_str("<pre class=\"");
            html.push_str(QUARANTINED_HTML_CLASS);
            html.push_str("\">");
            html.push_str(&escape_html(&quarantined_html.source_text));
            html.push_str("</pre>\n");
        }
        // V4 Compatibility Mode: a thematic break is valid Markdown — render
        // it as a real horizontal rule element, not a quarantine block.
        BlockAst::ThematicBreak(_) => {
            html.push_str("<hr />\n");
        }
        BlockAst::Table(table) => render_table(table, html),
        BlockAst::FootnoteDefinition(footnote) => {
            html.push_str("<aside class=\"adoc-footnote\" id=\"fn-");
            html.push_str(&escape_html(&footnote.label));
            html.push_str("\">\n");
            for child in &footnote.content {
                render_block(child, today, contradicted_claim_ids, html);
            }
            html.push_str("<a class=\"adoc-footnote-backref\" href=\"#fnref-");
            html.push_str(&escape_html(&footnote.label));
            html.push_str("\">&#8617;</a>\n</aside>\n");
        }
        BlockAst::UnknownExtension(unknown) => {
            html.push_str("<pre class=\"adoc-unknown-extension\" data-kind=\"");
            html.push_str(unknown_extension_kind_token(unknown.kind));
            html.push_str("\"><code>");
            html.push_str(&escape_html(&unknown.source_text));
            html.push_str("</code></pre>\n");
        }
    }
}

fn render_table(table: &crate::domain::ast::TableAst, html: &mut String) {
    html.push_str("<table class=\"adoc-table\">\n");
    if !table.header.is_empty() {
        html.push_str("<thead><tr>");
        for (index, cell) in table.header.iter().enumerate() {
            html.push_str("<th");
            if let Some(class) = column_alignment_class(table.alignments.get(index).copied()) {
                html.push_str(" class=\"");
                html.push_str(class);
                html.push('"');
            }
            html.push('>');
            render_inlines(&cell.inlines, html);
            html.push_str("</th>");
        }
        html.push_str("</tr></thead>\n");
    }
    if !table.rows.is_empty() {
        html.push_str("<tbody>\n");
        for row in &table.rows {
            html.push_str("<tr>");
            for (index, cell) in row.iter().enumerate() {
                html.push_str("<td");
                if let Some(class) = column_alignment_class(table.alignments.get(index).copied()) {
                    html.push_str(" class=\"");
                    html.push_str(class);
                    html.push('"');
                }
                html.push('>');
                render_inlines(&cell.inlines, html);
                html.push_str("</td>");
            }
            html.push_str("</tr>\n");
        }
        html.push_str("</tbody>\n");
    }
    html.push_str("</table>\n");
}

fn column_alignment_class(alignment: Option<ColumnAlignment>) -> Option<&'static str> {
    match alignment? {
        ColumnAlignment::Default => None,
        ColumnAlignment::Left => Some("adoc-table-cell-left"),
        ColumnAlignment::Center => Some("adoc-table-cell-center"),
        ColumnAlignment::Right => Some("adoc-table-cell-right"),
    }
}

fn unknown_extension_kind_token(kind: UnknownExtensionKind) -> &'static str {
    match kind {
        UnknownExtensionKind::MdxComponent => "mdx-component",
        UnknownExtensionKind::PandocDirective => "pandoc-directive",
        UnknownExtensionKind::AttributeBlock => "attribute-block",
        UnknownExtensionKind::MathFence => "math-fence",
    }
}

fn render_knowledge_object(
    knowledge_object: &KnowledgeObject,
    today: Option<NaiveDate>,
    contradicted_claim_ids: &HashSet<String>,
    html: &mut String,
) {
    let metadata = knowledge_object.metadata_projection();

    // V5.10: derive effective_status for badge rendering.
    // Precedence: stale > contradicted (stale is the stronger lifecycle signal).
    let status = metadata
        .discriminant()
        .map(|d| d.value_as_str().to_string());
    let stale_label = today
        .and_then(|date| derive_effective_status(&status, knowledge_object, date))
        .map(|(s, _)| s);

    // V5.10 TB4: a claim is effectively contradicted if it is referenced by
    // any unresolved contradiction in the workspace.
    let contradicted_label = if stale_label.is_none() {
        // Only apply contradicted badge when stale is not set (stale wins).
        if knowledge_object.kind().as_str() == "claim" {
            let claim_id = knowledge_object.id().as_str();
            if contradicted_claim_ids.contains(claim_id) {
                Some("contradicted".to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let effective_status_label = stale_label.or(contradicted_label);

    match knowledge_object {
        KnowledgeObject::Claim(_) => {
            render_claim(knowledge_object, &metadata, html);
        }
        KnowledgeObject::Decision(_) => {
            render_decision(knowledge_object, &metadata, html);
        }
        KnowledgeObject::Glossary(_) => {
            render_glossary(knowledge_object, &metadata, html);
        }
        KnowledgeObject::Warning(_) => {
            render_warning(knowledge_object, &metadata, html);
        }
        KnowledgeObject::Constraint(_) => {
            render_constraint(knowledge_object, &metadata, html);
        }
        KnowledgeObject::Policy(_) => {
            render_policy(knowledge_object, &metadata, html);
        }
        KnowledgeObject::Procedure(_) => {
            render_procedure(knowledge_object, &metadata, html);
        }
        KnowledgeObject::Example(_) => {
            render_example(knowledge_object, &metadata, html);
        }
        KnowledgeObject::AgentInstruction(_) => {
            render_agent_instruction(knowledge_object, &metadata, html);
        }
        KnowledgeObject::Contradiction(_) => {
            render_contradiction(knowledge_object, &metadata, html);
        }
        KnowledgeObject::Source(_) => {
            render_source(knowledge_object, &metadata, html);
        }
        KnowledgeObject::Api(_) => {
            render_api(knowledge_object, &metadata, html);
        }
        KnowledgeObject::Observation(_) => {
            render_observation(knowledge_object, &metadata, html);
        }
        KnowledgeObject::Question(_) => {
            render_question(knowledge_object, &metadata, html);
        }
    }

    // V5.10: append effective_status badge after the section close tag if set.
    // The badge is injected before the closing `</section>\n` of the preceding
    // section. This avoids threading date/contradiction context into every
    // per-kind render function.
    if let Some(label) = effective_status_label {
        // Replace the last `</section>\n` with badge + `</section>\n`.
        if let Some(pos) = html.rfind("</section>\n") {
            let badge = format!(
                "<span class=\"ko__effective-status ko__effective-status--{label}\">{label}</span>\n",
            );
            html.insert_str(pos, &badge);
        }
    }
}

fn render_glossary(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    render_object_section_open(knowledge_object, "glossary", html);
    render_object_header(knowledge_object, status_badge(metadata), html);
    render_object_body(knowledge_object, html);
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_warning(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    let severity = required_severity(metadata);
    let class = format!("warning warning--{severity}");

    render_object_section_open(knowledge_object, &class, html);
    render_object_header(knowledge_object, Some(("severity", severity)), html);
    render_object_body(knowledge_object, html);
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_constraint(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    let severity = required_severity(metadata);
    let class = format!("constraint constraint--{severity}");

    render_object_section_open(knowledge_object, &class, html);
    render_object_header(knowledge_object, Some(("severity", severity)), html);
    render_object_body(knowledge_object, html);
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_contradiction(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    // For contradiction the discriminant is the lifecycle status; severity
    // lives in its dedicated ADR-0039 slot.
    let status_str = metadata
        .discriminant()
        .map(|d| d.value_as_str())
        .unwrap_or("unresolved");
    let severity_str = required_severity(metadata);
    let class =
        format!("contradiction contradiction--{status_str} contradiction--severity-{severity_str}");

    render_object_section_open(knowledge_object, &class, html);

    // Severity badge.
    html.push_str("<span class=\"contradiction__severity-badge\">");
    html.push_str(&escape_html(severity_str));
    html.push_str("</span>\n");

    render_object_header(knowledge_object, status_badge(metadata), html);

    // Status line.
    html.push_str("<p class=\"contradiction__status\">status: ");
    html.push_str(&escape_html(status_str));
    html.push_str("</p>\n");

    // Linked list of conflicting claim IDs.
    let KnowledgeObject::Contradiction(contradiction) = knowledge_object else {
        unreachable!("render_contradiction called with non-contradiction object");
    };
    render_contradiction_claims(contradiction, html);

    render_object_body(knowledge_object, html);
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_source(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    let KnowledgeObject::Source(source) = knowledge_object else {
        unreachable!("render_source called with non-source object");
    };

    let kind_str = source.kind().as_str();
    let class = format!("source source--{kind_str}");

    render_object_section_open(knowledge_object, &class, html);

    // Evidence kind badge.
    html.push_str("<span class=\"source__kind-badge\">");
    html.push_str(&escape_html(kind_str));
    html.push_str("</span>\n");

    render_object_header(knowledge_object, None, html);

    // Path or URL metadata line.
    html.push_str("<div class=\"source__target\">\n<dl>\n");
    if let Some(path) = source.path() {
        html.push_str("<dt>path</dt><dd><code>");
        html.push_str(&escape_html(path.as_str()));
        html.push_str("</code></dd>\n");
    } else if let Some(url) = source.url() {
        // The `Url` type only allows http, https, and mailto — always safe.
        html.push_str("<dt>url</dt><dd><a href=\"");
        html.push_str(&escape_html(url.as_str()));
        html.push_str("\">");
        html.push_str(&escape_html(url.as_str()));
        html.push_str("</a></dd>\n");
    }
    html.push_str("</dl>\n</div>\n");

    render_object_body(knowledge_object, html);
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_api(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    let KnowledgeObject::Api(api) = knowledge_object else {
        unreachable!("render_api called with non-api object");
    };

    render_object_section_open(knowledge_object, "api", html);
    render_object_header(knowledge_object, status_badge(metadata), html);

    // Endpoint signature: method badge (or interface type) plus path/symbol
    // in code style, above the prose body (PRD §13.7).
    html.push_str("<div class=\"api__signature\">");
    if let Some(method) = api.method() {
        html.push_str("<span class=\"api__method\">");
        html.push_str(method.as_str());
        html.push_str("</span>");
    } else if let Some(interface_type) = api.interface_type() {
        html.push_str("<span class=\"api__interface-type\">");
        html.push_str(&escape_html(interface_type));
        html.push_str("</span>");
    }
    if let Some(path) = api.path() {
        html.push_str("<code class=\"api__path\">");
        html.push_str(&escape_html(path));
        html.push_str("</code>");
    } else if let Some(symbol) = api.symbol() {
        html.push_str("<code class=\"api__symbol\">");
        html.push_str(&escape_html(symbol));
        html.push_str("</code>");
    }
    html.push_str("</div>\n");

    render_object_body(knowledge_object, html);
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_observation(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    let KnowledgeObject::Observation(observation) = knowledge_object else {
        unreachable!("render_observation called with non-observation object");
    };

    render_object_section_open(knowledge_object, "observation", html);
    render_object_header(knowledge_object, status_badge(metadata), html);

    // Sample size and observed date as metadata chips above the prose body
    // (PRD §13.9).
    if observation.sample_size().is_some() || observation.observed_at().is_some() {
        html.push_str("<div class=\"observation__chips\">");
        if let Some(sample_size) = observation.sample_size() {
            html.push_str("<span class=\"observation__sample-size\">n=");
            html.push_str(&escape_html(sample_size.as_str()));
            html.push_str("</span>");
        }
        if let Some(observed_at) = observation.observed_at() {
            html.push_str("<span class=\"observation__observed-at\">");
            html.push_str(&escape_html(observed_at.as_str()));
            html.push_str("</span>");
        }
        html.push_str("</div>\n");
    }

    render_object_body(knowledge_object, html);
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_question(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    let KnowledgeObject::Question(question) = knowledge_object else {
        unreachable!("render_question called with non-question object");
    };

    render_object_section_open(knowledge_object, "question", html);
    render_object_header(knowledge_object, status_badge(metadata), html);

    // Open questions carry a prominent badge; answered ones link to the
    // resolving claim/decision (PRD §13.10).
    if let Some(resolved_by) = question.resolved_by() {
        html.push_str(
            "<div class=\"question__resolved-by\">Answered by <a class=\"object-ref\" href=\"#",
        );
        html.push_str(&escape_html(resolved_by.as_str()));
        html.push_str("\">");
        html.push_str(&escape_html(resolved_by.as_str()));
        html.push_str("</a></div>\n");
    } else {
        html.push_str("<div class=\"question__open-badge\">Open</div>\n");
    }

    render_object_body(knowledge_object, html);
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_contradiction_claims(contradiction: &Contradiction, html: &mut String) {
    html.push_str("<ul class=\"contradiction__claims\">\n");
    for claim_id in contradiction.claims().as_slice() {
        html.push_str("<li><a href=\"#");
        html.push_str(&escape_html(claim_id.as_str()));
        html.push_str("\">");
        html.push_str(&escape_html(claim_id.as_str()));
        html.push_str("</a></li>\n");
    }
    html.push_str("</ul>\n");
}

fn render_procedure(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    render_object_section_open(knowledge_object, "procedure", html);
    render_object_header(knowledge_object, status_badge(metadata), html);
    render_procedure_body(knowledge_object, html);
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_policy(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    render_object_section_open(knowledge_object, "policy", html);
    render_object_header(knowledge_object, status_badge(metadata), html);
    render_object_body(knowledge_object, html);

    // Approval block: list effective_at, each approver, and optional review_interval.
    let KnowledgeObject::Policy(policy) = knowledge_object else {
        unreachable!("render_policy called with non-policy object");
    };
    render_policy_approval(policy, html);

    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_policy_approval(policy: &Policy, html: &mut String) {
    html.push_str("<div class=\"policy__approval\">\n<dl>\n");
    html.push_str("<div class=\"policy__approval-item\"><dt>effective_at</dt><dd>");
    html.push_str(&escape_html(policy.effective_at().as_str()));
    html.push_str("</dd></div>\n");
    for approver in policy.approved_by().as_slice() {
        html.push_str("<div class=\"policy__approval-item\"><dt>approved_by</dt><dd>");
        html.push_str(&escape_html(approver.as_str()));
        html.push_str("</dd></div>\n");
    }
    if let Some(ri) = policy.review_interval() {
        html.push_str("<div class=\"policy__approval-item\"><dt>review_interval</dt><dd>");
        html.push_str(&escape_html(ri.as_str()));
        html.push_str("</dd></div>\n");
    }
    html.push_str("</dl>\n</div>\n");
}

fn render_agent_instruction(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    render_object_section_open(knowledge_object, "agent_instruction", html);

    // ADR-0025: mandatory "NOT runtime ACL" banner — exact text is non-negotiable.
    html.push_str("<div class=\"agent_instruction__banner\"><p>Agent Instruction. Authored knowledge, NOT runtime ACL. See <a href=\"adoc://agent/v0/agent-instruction-guide\">agent-instruction-guide</a>.</p></div>\n");

    let trust_badge = metadata.trust().map(|trust| ("trust", trust.as_str()));
    render_object_header(knowledge_object, trust_badge, html);

    let KnowledgeObject::AgentInstruction(ai) = knowledge_object else {
        unreachable!("render_agent_instruction called with non-agent_instruction object");
    };
    render_agent_instruction_fields(ai, html);

    render_object_body(knowledge_object, html);
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_agent_instruction_fields(ai: &AgentInstruction, html: &mut String) {
    html.push_str("<div class=\"agent_instruction__fields\">\n<dl>\n");
    html.push_str("<div class=\"agent_instruction__field-item\"><dt>scope</dt><dd>");
    html.push_str(&escape_html(ai.scope().as_str()));
    html.push_str("</dd></div>\n");
    html.push_str("<div class=\"agent_instruction__field-item\"><dt>trust</dt><dd><span class=\"agent_instruction__trust\">");
    html.push_str(&escape_html(ai.trust().as_str()));
    html.push_str("</span></dd></div>\n");
    for action in ai.action_set().allowed() {
        html.push_str("<div class=\"agent_instruction__field-item\"><dt>allowed_actions</dt><dd>");
        html.push_str(&escape_html(action.as_str()));
        html.push_str("</dd></div>\n");
    }
    for action in ai.action_set().forbidden() {
        html.push_str(
            "<div class=\"agent_instruction__field-item\"><dt>forbidden_actions</dt><dd>",
        );
        html.push_str(&escape_html(action.as_str()));
        html.push_str("</dd></div>\n");
    }
    html.push_str("</dl>\n</div>\n");
}

/// Render a procedure body as numbered steps. The aggregate guarantees the
/// body begins with an ordered list (V5.2 strict rule); each ordered-list line
/// becomes an `<li>` inside a single `<ol>`, and any non-list prose lines
/// render as paragraphs. The graph stores the body as canonical prose — visual
/// ordering lives here, in the renderer (V5-DESIGN §V5.2).
fn render_procedure_body(knowledge_object: &KnowledgeObject, html: &mut String) {
    html.push_str("<div class=\"procedure__body\">\n");

    let lines = split_inline_lines(knowledge_object.body().inlines());
    let mut in_list = false;
    for line in &lines {
        if line.is_empty() {
            continue;
        }
        match ordered_step_marker_on_line(line) {
            Some(marker_len) => {
                if !in_list {
                    html.push_str("<ol>\n");
                    in_list = true;
                }
                html.push_str("<li>");
                render_ordered_step_item(line, marker_len, html);
                html.push_str("</li>\n");
            }
            None => {
                if in_list {
                    html.push_str("</ol>\n");
                    in_list = false;
                }
                html.push_str("<p>");
                render_inlines(line, html);
                html.push_str("</p>\n");
            }
        }
    }
    if in_list {
        html.push_str("</ol>\n");
    }

    html.push_str("</div>\n");
}

/// Split body inlines into per-line groups. A line break is a newline inside a
/// `Text` segment (`InlineSegment::Text("\n")` produced by the parser, or an
/// embedded `\n` in a single-segment body). Inline formatting (code, links,
/// object refs) within a line is preserved.
fn split_inline_lines(inlines: &[InlineSegment]) -> Vec<Vec<InlineSegment>> {
    let mut lines: Vec<Vec<InlineSegment>> = vec![Vec::new()];
    for segment in inlines {
        let InlineSegment::Text(text) = segment else {
            lines
                .last_mut()
                .expect("lines is seeded with one group")
                .push(segment.clone());
            continue;
        };
        let mut parts = text.split('\n');
        if let Some(first) = parts.next()
            && !first.is_empty()
        {
            lines
                .last_mut()
                .expect("lines is seeded with one group")
                .push(InlineSegment::Text(first.to_string()));
        }
        for part in parts {
            lines.push(Vec::new());
            if !part.is_empty() {
                lines
                    .last_mut()
                    .expect("a group was just pushed")
                    .push(InlineSegment::Text(part.to_string()));
            }
        }
    }
    lines
}

fn ordered_step_marker_on_line(line: &[InlineSegment]) -> Option<usize> {
    match line.first() {
        Some(InlineSegment::Text(text)) => ordered_step_marker_len(text),
        _ => None,
    }
}

fn render_ordered_step_item(line: &[InlineSegment], marker_len: usize, html: &mut String) {
    let Some((InlineSegment::Text(text), rest)) = line.split_first() else {
        render_inlines(line, html);
        return;
    };
    let stripped = &text[marker_len..];
    if !stripped.is_empty() {
        html.push_str(&escape_html(stripped));
    }
    render_inlines(rest, html);
}

fn render_example(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    render_object_section_open(knowledge_object, "example", html);
    render_object_header(knowledge_object, status_badge(metadata), html);

    // Render optional checks/sandbox metadata above the code body.
    let KnowledgeObject::Example(example) = knowledge_object else {
        unreachable!("render_example called with non-example object");
    };
    if example.checks().is_some() || example.sandbox().is_some() {
        html.push_str("<div class=\"example__meta\">\n<dl>\n");
        if let Some(checks) = example.checks() {
            html.push_str("<dt>checks</dt><dd>");
            html.push_str(&escape_html(checks));
            html.push_str("<span class=\"example__caveat\"> Not executed by adoc</span>");
            html.push_str("</dd>\n");
        }
        if let Some(sandbox) = example.sandbox() {
            html.push_str("<dt>sandbox</dt><dd>");
            html.push_str(&escape_html(sandbox.as_str()));
            html.push_str("</dd>\n");
        }
        html.push_str("</dl>\n</div>\n");
    }

    // Render body as a fenced code block using the declared lang or format.
    let body_source = knowledge_object.body().to_source();
    if let Some(lang) = example.lang() {
        html.push_str("<pre><code class=\"language-");
        html.push_str(&escape_html(lang.as_str()));
        html.push_str("\">");
        html.push_str(&escape_html(&body_source));
        html.push_str("</code></pre>\n");
    } else if let Some(format) = example.format() {
        html.push_str("<pre><code class=\"format-");
        html.push_str(&escape_html(format));
        html.push_str("\">");
        html.push_str(&escape_html(&body_source));
        html.push_str("</code></pre>\n");
    } else {
        html.push_str("<pre><code>");
        html.push_str(&escape_html(&body_source));
        html.push_str("</code></pre>\n");
    }

    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn required_severity<'a>(metadata: &KnowledgeObjectMetadata<'a>) -> &'a str {
    metadata
        .severity()
        .map(|severity| severity.as_str())
        .expect("severity-bearing kind metadata projection must include severity")
}

fn render_decision(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    let class = if accepted_decision_field(metadata).is_some() {
        "decision decision--accepted"
    } else {
        "decision"
    };
    render_object_section_open(knowledge_object, class, html);
    render_object_header(knowledge_object, status_badge(metadata), html);
    render_object_body(knowledge_object, html);
    if let Some(decided_by) = accepted_decision_field(metadata) {
        html.push_str("<div class=\"decision__verdict\"><dl>");
        html.push_str("<div class=\"decision__verdict-item\"><dt>");
        html.push_str(decided_by.key());
        html.push_str("</dt><dd>");
        html.push_str(&escape_html(decided_by.value_as_str()));
        html.push_str("</dd></div>");
        html.push_str("</dl></div>\n");
    }
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_claim(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    let class = if has_claim_verification(metadata) {
        "claim claim--verified"
    } else {
        "claim"
    };
    render_object_section_open(knowledge_object, class, html);
    render_object_header(knowledge_object, status_badge(metadata), html);
    render_object_body(knowledge_object, html);

    if let (Some(owner), Some(verified_at)) = (
        claim_owner_field(metadata),
        claim_verified_at_field(metadata),
    ) {
        html.push_str("<div class=\"claim__verification\">\n");
        html.push_str("<dl>\n");
        html.push_str("<div class=\"claim__verification-item\"><dt>");
        html.push_str(owner.key());
        html.push_str("</dt><dd>");
        html.push_str(&escape_html(owner.value_as_str()));
        html.push_str("</dd></div>\n");
        html.push_str("<div class=\"claim__verification-item\"><dt>");
        html.push_str(verified_at.key());
        html.push_str("</dt><dd>");
        html.push_str(&escape_html(verified_at.value_as_str()));
        html.push_str("</dd></div>\n");
        html.push_str("</dl>\n");
        html.push_str("</div>\n");

        html.push_str("<div class=\"claim__evidence\">\n");
        html.push_str("<dl>\n");
        for evidence in claim_evidence_fields(metadata) {
            render_evidence(evidence, html);
        }
        html.push_str("</dl>\n");
        html.push_str("</div>\n");
    }

    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn has_claim_verification(metadata: &KnowledgeObjectMetadata<'_>) -> bool {
    claim_owner_field(metadata).is_some()
}

fn claim_owner_field<'a>(
    metadata: &'a KnowledgeObjectMetadata<'a>,
) -> Option<&'a MetadataField<'a>> {
    metadata
        .fields()
        .iter()
        .find(|field| matches!(field, MetadataField::Owner(_)))
}

fn claim_verified_at_field<'a>(
    metadata: &'a KnowledgeObjectMetadata<'a>,
) -> Option<&'a MetadataField<'a>> {
    metadata
        .fields()
        .iter()
        .find(|field| matches!(field, MetadataField::VerifiedAt(_)))
}

fn claim_evidence_fields<'a>(
    metadata: &'a KnowledgeObjectMetadata<'a>,
) -> impl Iterator<Item = &'a Evidence> {
    // V5.8: evidence is in the typed `evidence()` slice, not in `fields()`.
    metadata.evidence().iter().copied()
}

fn accepted_decision_field<'a>(
    metadata: &'a KnowledgeObjectMetadata<'a>,
) -> Option<&'a MetadataField<'a>> {
    metadata
        .fields()
        .iter()
        .find(|field| matches!(field, MetadataField::DecidedBy(_)))
}

fn render_evidence(evidence: &Evidence, html: &mut String) {
    // V5.8: ObjectRef entries are cross-object links; render as a reference.
    // Inline entries carry an EvidenceKind + value text.
    if let Some(ref_id) = evidence.target_id() {
        html.push_str("<div class=\"claim__evidence-item claim__evidence-item--object-ref\">");
        html.push_str("<dt>evidence_ref</dt><dd>");
        html.push_str(ref_id.as_str());
        html.push_str("</dd></div>\n");
    } else if let (Some(kind), Some(value)) = (evidence.kind(), evidence.value()) {
        // EvidenceKind as_str() is the display key (e.g. "source_code").
        // The CSS modifier converts underscores to dashes for the BEM class.
        let kind_str = kind.as_str();
        let modifier = kind_str.replace('_', "-");
        html.push_str("<div class=\"claim__evidence-item claim__evidence-item--");
        html.push_str(&modifier);
        html.push_str("\"><dt>");
        html.push_str(kind_str);
        html.push_str("</dt><dd>");
        html.push_str(&escape_html(value.as_str()));
        html.push_str("</dd></div>\n");
    }
}

fn render_object_section_open(
    knowledge_object: &KnowledgeObject,
    class_name: &str,
    html: &mut String,
) {
    html.push_str("<section class=\"");
    html.push_str(class_name);
    html.push_str("\" id=\"");
    html.push_str(&escape_html(knowledge_object.id().as_str()));
    html.push_str("\">\n");
}

/// Header badge: `(field_name, value)` — `("status", …)` for lifecycle
/// discriminants, `("severity", …)`/`("trust", …)` for the dedicated
/// ADR-0039 carriers.
fn render_object_header(
    knowledge_object: &KnowledgeObject,
    badge: Option<(&str, &str)>,
    html: &mut String,
) {
    let kind = knowledge_object.kind().as_str();

    html.push_str("<header class=\"");
    html.push_str(kind);
    html.push_str("__header\">");
    html.push_str("<span class=\"");
    html.push_str(kind);
    html.push_str("__kind\">");
    html.push_str(kind);
    html.push_str("</span>");
    html.push_str("<code class=\"");
    html.push_str(kind);
    html.push_str("__id\">");
    html.push_str(&escape_html(knowledge_object.id().as_str()));
    html.push_str("</code>");

    if let Some((field_name, value)) = badge {
        html.push_str("<span class=\"");
        html.push_str(kind);
        html.push_str("__");
        html.push_str(field_name);
        html.push_str("\">");
        html.push_str(&escape_html(value));
        html.push_str("</span>");
    }

    html.push_str("</header>\n");
}

fn status_badge<'a>(metadata: &KnowledgeObjectMetadata<'a>) -> Option<(&'static str, &'a str)> {
    metadata
        .discriminant()
        .map(|discriminant| ("status", discriminant.value_as_str()))
}

fn render_object_body(knowledge_object: &KnowledgeObject, html: &mut String) {
    let kind = knowledge_object.kind().as_str();

    html.push_str("<div class=\"");
    html.push_str(kind);
    html.push_str("__body\"><p>");
    render_inlines(knowledge_object.body().inlines(), html);
    html.push_str("</p></div>\n");
}

fn render_object_metadata(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    if !has_stored_metadata_fields(metadata) && knowledge_object.relations().is_empty() {
        return;
    }

    let kind = knowledge_object.kind().as_str();
    html.push_str("<footer class=\"");
    html.push_str(kind);
    html.push_str("__metadata\">\n");
    render_metadata_fields(metadata, html);
    render_relations(kind, knowledge_object.relations(), html);
    html.push_str("</footer>\n");
}

fn has_stored_metadata_fields(metadata: &KnowledgeObjectMetadata<'_>) -> bool {
    metadata
        .fields()
        .iter()
        .any(|field| matches!(field, MetadataField::Stored { .. }))
}

fn render_metadata_fields(metadata: &KnowledgeObjectMetadata<'_>, html: &mut String) {
    let mut rendered_any = false;
    for field in metadata.fields() {
        let MetadataField::Stored { key, value } = field else {
            continue;
        };

        if !rendered_any {
            html.push_str("<dl>\n");
            rendered_any = true;
        }
        html.push_str("<dt>");
        html.push_str(&escape_html(key));
        html.push_str("</dt><dd>");
        html.push_str(&escape_html(value));
        html.push_str("</dd>\n");
    }
    if rendered_any {
        html.push_str("</dl>\n");
    }
}

fn render_relations(kind: &str, relations: &Relations, html: &mut String) {
    if relations.is_empty() {
        return;
    }

    html.push_str("<section class=\"");
    html.push_str(kind);
    html.push_str("__relations\"><dl>\n");
    for relation in GraphRelationKind::ALL {
        render_relation_group(relation, relations.targets(relation), html);
    }
    html.push_str("</dl></section>\n");
}

fn render_relation_group(
    relation: GraphRelationKind,
    targets: &[crate::domain::knowledge_object::RelationTarget],
    html: &mut String,
) {
    if targets.is_empty() {
        return;
    }

    html.push_str("<dt>");
    html.push_str(relation.as_str());
    html.push_str("</dt><dd>");
    for (index, target) in targets.iter().enumerate() {
        if index > 0 {
            html.push_str(", ");
        }
        render_object_ref_anchor(target.id().as_str(), html);
    }
    html.push_str("</dd>\n");
}

fn render_object_ref_anchor(id: &str, html: &mut String) {
    html.push_str("<a class=\"object-ref\" href=\"#");
    html.push_str(&escape_html(id));
    html.push_str("\">");
    html.push_str(&escape_html(id));
    html.push_str("</a>");
}

fn render_inlines(segments: &[InlineSegment], html: &mut String) {
    for segment in segments {
        render_inline(segment, html);
    }
}

fn render_inline(segment: &InlineSegment, html: &mut String) {
    match segment {
        InlineSegment::Text(text) => {
            html.push_str(&escape_html(text));
        }
        InlineSegment::Code(code) => {
            html.push_str("<code>");
            html.push_str(&escape_html(code));
            html.push_str("</code>");
        }
        InlineSegment::Emphasis(inner) => {
            html.push_str("<em>");
            render_inlines(inner, html);
            html.push_str("</em>");
        }
        InlineSegment::Strong(inner) => {
            html.push_str("<strong>");
            render_inlines(inner, html);
            html.push_str("</strong>");
        }
        InlineSegment::Link { text, url, .. } => {
            // V4 Compatibility Mode allows Markdown sources to reach the
            // renderer with link URLs the compat validator already flagged.
            // Drop the `href` on unsafe schemes so rendered HTML can never
            // execute the link; the inline text is still rendered.
            if verdict(url).is_safe() {
                html.push_str("<a href=\"");
                html.push_str(&escape_html(url));
                html.push_str("\">");
                render_inlines(text, html);
                html.push_str("</a>");
            } else {
                render_inlines(text, html);
            }
        }
        InlineSegment::ObjectReference { id, .. } => {
            render_object_ref_anchor(id.as_str(), html);
        }
        InlineSegment::ObjectReferencePending { .. } => {
            unreachable!("object references must resolve before rendering")
        }
        InlineSegment::Image { alt, url, .. } => {
            let alt_text = crate::domain::inline::plain_text(alt);
            if verdict(url).is_safe() {
                html.push_str("<img src=\"");
                html.push_str(&escape_html(url));
                html.push_str("\" alt=\"");
                html.push_str(&escape_html(&alt_text));
                html.push_str("\" />");
            } else {
                html.push_str("<span class=\"");
                html.push_str(QUARANTINED_IMAGE_CLASS);
                html.push_str("\">");
                html.push_str(&escape_html(&alt_text));
                html.push_str("</span>");
            }
        }
        InlineSegment::QuarantinedHtml { source_text, .. } => {
            html.push_str("<code class=\"");
            html.push_str(QUARANTINED_HTML_CLASS);
            html.push_str("\">");
            html.push_str(&escape_html(source_text));
            html.push_str("</code>");
        }
        InlineSegment::Strikethrough(inner) => {
            html.push_str("<del>");
            render_inlines(inner, html);
            html.push_str("</del>");
        }
        InlineSegment::FootnoteReference { label, .. } => {
            html.push_str("<sup class=\"adoc-footnote-ref\" id=\"fnref-");
            html.push_str(&escape_html(label));
            html.push_str("\"><a href=\"#fn-");
            html.push_str(&escape_html(label));
            html.push_str("\">[");
            html.push_str(&escape_html(label));
            html.push_str("]</a></sup>");
        }
        InlineSegment::UnknownExtension {
            source_text, kind, ..
        } => {
            html.push_str("<code class=\"adoc-unknown-extension\" data-kind=\"");
            html.push_str(unknown_extension_kind_token(*kind));
            html.push_str("\">");
            html.push_str(&escape_html(source_text));
            html.push_str("</code>");
        }
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};

    fn render(segments: &[InlineSegment]) -> String {
        let mut html = String::new();
        render_inlines(segments, &mut html);
        html
    }

    fn dummy_span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("guide.adoc"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
        }
    }

    #[test]
    fn render_inlines_emits_text_with_html_escaping() {
        let html = render(&[InlineSegment::Text("AT&T <ok>".to_string())]);
        assert_eq!(html, "AT&amp;T &lt;ok&gt;");
    }

    #[test]
    fn render_inlines_emits_code_tag_with_escaped_body() {
        let html = render(&[InlineSegment::Code("Vec<String>".to_string())]);
        assert_eq!(html, "<code>Vec&lt;String&gt;</code>");
    }

    #[test]
    fn render_inlines_emits_em_tag_around_inner_segments() {
        let html = render(&[InlineSegment::Emphasis(vec![InlineSegment::Text(
            "italic".to_string(),
        )])]);
        assert_eq!(html, "<em>italic</em>");
    }

    #[test]
    fn render_inlines_emits_strong_tag_around_inner_segments() {
        let html = render(&[InlineSegment::Strong(vec![InlineSegment::Text(
            "bold".to_string(),
        )])]);
        assert_eq!(html, "<strong>bold</strong>");
    }

    #[test]
    fn render_inlines_emits_anchor_with_escaped_href_attribute() {
        let html = render(&[InlineSegment::Link {
            text: vec![InlineSegment::Text("docs".to_string())],
            url: "https://example.test/?q=\"a&b\"".to_string(),
            span: dummy_span(),
        }]);
        assert_eq!(
            html,
            "<a href=\"https://example.test/?q=&quot;a&amp;b&quot;\">docs</a>"
        );
    }

    #[test]
    fn render_html_flows_inlines_through_heading_paragraph_and_list_item() {
        use crate::domain::ast::{HeadingAst, ListAst, ListItem, ListKind, PageAst, ParagraphAst};
        use crate::domain::identity::PageId;

        let span = dummy_span;

        let page = PageAst {
            id: PageId::from_string("team.guide").expect("test page id is valid"),
            title: Some("Title".to_string()),
            source_path: PathBuf::from("guide.adoc"),
            blocks: vec![
                BlockAst::Heading(HeadingAst {
                    level: 1,
                    inlines: vec![
                        InlineSegment::Text("Title with ".to_string()),
                        InlineSegment::Strong(vec![InlineSegment::Text("bold".to_string())]),
                    ],
                    span: span(),
                }),
                BlockAst::Paragraph(ParagraphAst {
                    inlines: vec![
                        InlineSegment::Text("First ".to_string()),
                        InlineSegment::Emphasis(vec![InlineSegment::Text("emphasis".to_string())]),
                        InlineSegment::Text(" then ".to_string()),
                        InlineSegment::Code("ident".to_string()),
                        InlineSegment::Text(".".to_string()),
                    ],
                    span: span(),
                }),
                BlockAst::List(ListAst {
                    kind: ListKind::Unordered,
                    items: vec![ListItem {
                        inlines: vec![
                            InlineSegment::Text("Run ".to_string()),
                            InlineSegment::Code("adoc check".to_string()),
                        ],
                        span: span(),
                        task_state: None,
                        content: Vec::new(),
                    }],
                    span: span(),
                }),
            ],
        };

        let html = HtmlRenderer.render(&[page]);

        assert!(html.contains("<h1>Title with <strong>bold</strong></h1>"));
        assert!(html.contains("<p>First <em>emphasis</em> then <code>ident</code>.</p>"));
        assert!(html.contains("<li>Run <code>adoc check</code></li>"));
    }

    #[test]
    fn render_inlines_recursively_renders_link_label() {
        let html = render(&[InlineSegment::Link {
            text: vec![
                InlineSegment::Text("see ".to_string()),
                InlineSegment::Code("adoc".to_string()),
            ],
            url: "https://example.test".to_string(),
            span: dummy_span(),
        }]);
        assert_eq!(
            html,
            "<a href=\"https://example.test\">see <code>adoc</code></a>"
        );
    }

    fn make_claim(
        id: &str,
        status: &str,
        body: &str,
        fields: std::collections::BTreeMap<String, String>,
        span: SourceSpan,
    ) -> BlockAst {
        use crate::domain::knowledge_object::{KnowledgeObject, claim::Claim};
        let claim = Claim::try_new(id, Some(status), body, fields, None, span)
            .expect("test claim must be valid");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim)))
    }

    fn make_verified_claim(
        id: &str,
        body: &str,
        fields: std::collections::BTreeMap<String, String>,
        span: SourceSpan,
    ) -> BlockAst {
        use crate::domain::knowledge_object::{
            KnowledgeObject,
            claim::{
                Claim, Evidence, OWNER_FIELD, Owner, REVIEWED_BY_FIELD, SOURCE_FIELD, TEST_FIELD,
                VERIFIED_AT_FIELD, Verification, VerifiedAt,
            },
        };
        let verification = Verification::new(
            Owner::try_new(fields.get(OWNER_FIELD).expect("owner")).expect("owner"),
            VerifiedAt::try_new(fields.get(VERIFIED_AT_FIELD).expect("verified_at"))
                .expect("verified_at"),
            vec![
                Evidence::from_field(SOURCE_FIELD, fields.get(SOURCE_FIELD).expect("source"))
                    .expect("source"),
            ],
        );
        let mut storage_fields = fields;
        storage_fields.remove(OWNER_FIELD);
        storage_fields.remove(VERIFIED_AT_FIELD);
        storage_fields.remove(SOURCE_FIELD);
        storage_fields.remove(TEST_FIELD);
        storage_fields.remove(REVIEWED_BY_FIELD);
        let claim = Claim::try_new(
            id,
            Some("verified"),
            body,
            storage_fields,
            Some(verification),
            span,
        )
        .expect("test claim must be valid");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim)))
    }

    fn make_accepted_decision(
        id: &str,
        body: &str,
        fields: std::collections::BTreeMap<String, String>,
        span: SourceSpan,
    ) -> BlockAst {
        use crate::domain::knowledge_object::{
            KnowledgeObject,
            decision::{AcceptedVerdict, DecidedBy, Decision},
        };
        let verdict = AcceptedVerdict::new(
            DecidedBy::try_new("architecture").expect("decided_by"),
            Vec::new(),
        );
        let decision = Decision::try_new(id, Some("accepted"), body, fields, Some(verdict), span)
            .expect("test decision must be valid");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Decision(decision)))
    }

    fn make_warning(
        id: &str,
        severity: &str,
        body: &str,
        fields: std::collections::BTreeMap<String, String>,
        span: SourceSpan,
    ) -> BlockAst {
        use crate::domain::knowledge_object::{KnowledgeObject, warning::Warning};
        let warning = Warning::try_new(id, Some(severity), body, fields, span)
            .expect("test warning must be valid");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Warning(warning)))
    }

    fn make_glossary(
        id: &str,
        body: &str,
        fields: std::collections::BTreeMap<String, String>,
        span: SourceSpan,
    ) -> BlockAst {
        use crate::domain::knowledge_object::{KnowledgeObject, glossary::Glossary};
        let glossary =
            Glossary::try_new(id, body, fields, span).expect("test glossary must be valid");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Glossary(glossary)))
    }

    fn make_procedure(
        id: &str,
        status: &str,
        body: &str,
        fields: std::collections::BTreeMap<String, String>,
        span: SourceSpan,
    ) -> BlockAst {
        use crate::domain::knowledge_object::{KnowledgeObject, procedure::Procedure};
        let procedure = Procedure::try_new(id, Some(status), body, fields, None, span)
            .expect("test procedure must be valid");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Procedure(procedure)))
    }

    #[allow(clippy::too_many_arguments)]
    fn make_example(
        id: &str,
        status: Option<&str>,
        lang: Option<&str>,
        body: &str,
        checks: Option<&str>,
        sandbox: Option<&str>,
        fields: std::collections::BTreeMap<String, String>,
        span: SourceSpan,
    ) -> BlockAst {
        use crate::domain::knowledge_object::{KnowledgeObject, example::Example};
        let example = Example::try_new(id, status, lang, None, body, checks, sandbox, fields, span)
            .expect("test example must be valid");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Example(example)))
    }

    #[test]
    fn procedure_renders_four_numbered_steps_as_ordered_list_in_source_order() {
        let block = make_procedure(
            "auth.key.rotate",
            "draft",
            "1. Open the console.\n2. Rotate the key.\n3. Redeploy.\n4. Verify health.",
            std::collections::BTreeMap::new(),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        assert!(
            html.contains("<section class=\"procedure\" id=\"auth.key.rotate\">"),
            "missing procedure section: {html}"
        );
        assert!(
            html.contains("<span class=\"procedure__status\">draft</span>"),
            "missing status badge: {html}"
        );

        let body_start = html.find("<div class=\"procedure__body\">").expect("body");
        let body = &html[body_start..];
        let ol = body.find("<ol>").expect("missing <ol>");
        let close_ol = body.find("</ol>").expect("missing </ol>");
        let items = body[ol..close_ol].matches("<li>").count();
        assert_eq!(items, 4, "expected four steps, got {items}: {html}");

        // Steps render in source order with their numeric markers stripped.
        let one = body.find("<li>Open the console.</li>").expect("step 1");
        let two = body.find("<li>Rotate the key.</li>").expect("step 2");
        let three = body.find("<li>Redeploy.</li>").expect("step 3");
        let four = body.find("<li>Verify health.</li>").expect("step 4");
        assert!(
            one < two && two < three && three < four,
            "wrong order: {html}"
        );
        assert!(
            !body.contains("1. Open"),
            "ordered-list marker must be stripped: {html}"
        );
    }

    #[test]
    fn example_renders_lang_code_block_with_status_and_checks() {
        let block = make_example(
            "auth.credits.example",
            Some("draft"),
            Some("ts"),
            "const x = 1 + 1;",
            Some("npm test"),
            Some("node-test"),
            std::collections::BTreeMap::new(),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        assert!(
            html.contains("<section class=\"example\" id=\"auth.credits.example\">"),
            "missing example section: {html}"
        );
        assert!(
            html.contains("<span class=\"example__status\">draft</span>"),
            "missing status badge: {html}"
        );
        assert!(
            html.contains("<pre><code class=\"language-ts\">const x = 1 + 1;</code></pre>"),
            "missing code block with lang: {html}"
        );
        assert!(
            html.contains("<span class=\"example__caveat\"> Not executed by adoc</span>"),
            "missing caveat text: {html}"
        );
        assert!(
            html.contains("<dt>checks</dt>"),
            "missing checks field: {html}"
        );
        assert!(
            html.contains("<dt>sandbox</dt><dd>node-test</dd>"),
            "missing sandbox field: {html}"
        );
    }

    #[test]
    fn example_without_status_renders_no_status_badge() {
        let block = make_example(
            "auth.credits.example",
            None,
            Some("ts"),
            "const x = 1 + 1;",
            None,
            None,
            std::collections::BTreeMap::new(),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        assert!(
            !html.contains("example__status"),
            "unexpected status badge: {html}"
        );
        assert!(
            html.contains("<pre><code class=\"language-ts\">"),
            "missing lang code block: {html}"
        );
    }

    #[test]
    fn claim_renders_to_section_with_kind_id_status_body() {
        let block = make_claim(
            "billing.credits",
            "plain",
            "body content",
            std::collections::BTreeMap::new(),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        assert!(
            html.contains("<section class=\"claim\" id=\"billing.credits\">"),
            "missing section open tag: {html}"
        );
        assert!(
            html.contains("<span class=\"claim__kind\">claim</span>"),
            "missing kind span: {html}"
        );
        assert!(
            html.contains("<code class=\"claim__id\">billing.credits</code>"),
            "missing id code: {html}"
        );
        assert!(
            html.contains("<span class=\"claim__status\">plain</span>"),
            "missing status span: {html}"
        );
        assert!(
            html.contains("<p>body content</p>"),
            "missing body paragraph: {html}"
        );
    }

    #[test]
    fn claim_omits_metadata_footer_when_fields_are_empty() {
        let block = make_claim(
            "billing.credits",
            "plain",
            "body content",
            std::collections::BTreeMap::new(),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        assert!(
            !html.contains("<footer class=\"claim__metadata\">"),
            "unexpected metadata footer: {html}"
        );
    }

    #[test]
    fn claim_renders_metadata_footer_when_fields_are_present() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("source".to_string(), "ledger".to_string());
        fields.insert("owner".to_string(), "".to_string());
        let block = make_claim(
            "billing.credits",
            "plain",
            "body content",
            fields,
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        assert!(
            html.contains("<footer class=\"claim__metadata\">\n<dl>\n"),
            "missing metadata footer: {html}"
        );
        assert!(
            html.contains("<dt>source</dt><dd>ledger</dd>"),
            "missing populated field: {html}"
        );
        assert!(
            html.contains("<dt>owner</dt><dd></dd>"),
            "missing empty value field: {html}"
        );
    }

    #[test]
    fn claim_renders_metadata_fields_in_sorted_key_order() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("zeta".to_string(), "last".to_string());
        fields.insert("alpha".to_string(), "first".to_string());
        fields.insert("middle".to_string(), "second".to_string());
        let block = make_claim(
            "billing.credits",
            "plain",
            "body content",
            fields,
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        let alpha = html.find("<dt>alpha</dt>").expect("missing alpha field");
        let middle = html.find("<dt>middle</dt>").expect("missing middle field");
        let zeta = html.find("<dt>zeta</dt>").expect("missing zeta field");

        assert!(
            alpha < middle && middle < zeta,
            "fields not sorted by key: {html}"
        );
    }

    #[test]
    fn claim_html_escapes_metadata_keys_and_values() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "a <key> & \"id\"".to_string(),
            "value 'x' & <y>".to_string(),
        );
        let block = make_claim(
            "billing.credits",
            "plain",
            "body content",
            fields,
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        assert!(
            html.contains(
                "<dt>a &lt;key&gt; &amp; &quot;id&quot;</dt><dd>value &#39;x&#39; &amp; &lt;y&gt;</dd>"
            ),
            "metadata key or value not escaped: {html}"
        );
    }

    #[test]
    fn claim_html_escapes_id_status_and_body() {
        // Valid id; dangerous chars in status and body
        let block = make_claim(
            "team.guide",
            "<script>alert(1)</script>",
            "a < b && b > c",
            std::collections::BTreeMap::new(),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        // Status tag content must be escaped
        assert!(
            html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
            "status not escaped: {html}"
        );
        // Body must be escaped
        assert!(
            html.contains("a &lt; b &amp;&amp; b &gt; c"),
            "body not escaped: {html}"
        );
        // No raw angle brackets in output
        assert!(
            !html.contains("<script>"),
            "raw script tag leaked into output: {html}"
        );
    }

    #[test]
    fn verified_claim_renders_verification_and_evidence_sections() {
        let fields = std::collections::BTreeMap::from([
            ("owner".to_string(), "team-billing".to_string()),
            ("verified_at".to_string(), "2026-05-05".to_string()),
            ("source".to_string(), "payments ledger".to_string()),
        ]);
        let block = make_verified_claim("billing.credits", "body content", fields, dummy_span());
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        assert!(
            html.contains("<section class=\"claim claim--verified\" id=\"billing.credits\">"),
            "missing verified modifier: {html}"
        );
        assert!(
            html.contains("<div class=\"claim__verification\">"),
            "missing verification section: {html}"
        );
        assert!(
            html.contains(
                "<div class=\"claim__verification-item\"><dt>owner</dt><dd>team-billing</dd></div>"
            ),
            "missing owner: {html}"
        );
        assert!(
            html.contains(
                "<div class=\"claim__verification-item\"><dt>verified_at</dt><dd>2026-05-05</dd></div>"
            ),
            "missing verified_at: {html}"
        );
        assert!(
            html.contains("<div class=\"claim__evidence\">"),
            "missing evidence section: {html}"
        );
        // V5.8: evidence kind is now "source_code" (EvidenceKind::SourceCode).
        assert!(
            html.contains(
                "<div class=\"claim__evidence-item claim__evidence-item--source-code\"><dt>source_code</dt><dd>payments ledger</dd></div>"
            ),
            "missing source evidence: {html}"
        );
    }

    #[test]
    fn accepted_decision_renders_modifier_and_verdict_before_metadata() {
        let fields =
            std::collections::BTreeMap::from([("audience".to_string(), "support".to_string())]);
        let block = make_accepted_decision(
            "billing.policy",
            "Use the existing billing policy.",
            fields,
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        assert!(
            html.contains("<section class=\"decision decision--accepted\" id=\"billing.policy\">"),
            "missing accepted modifier: {html}"
        );
        assert!(
            html.contains(
                "<div class=\"decision__verdict\"><dl><div class=\"decision__verdict-item\"><dt>decided_by</dt><dd>architecture</dd></div></dl></div>"
            ),
            "missing verdict block: {html}"
        );
        let body = html.find("<div class=\"decision__body\">").expect("body");
        let verdict = html
            .find("<div class=\"decision__verdict\">")
            .expect("verdict");
        let metadata = html
            .find("<footer class=\"decision__metadata\">")
            .expect("metadata");
        assert!(body < verdict && verdict < metadata, "wrong order: {html}");
        assert!(
            !html.contains("<footer class=\"decision__metadata\">\n<dl>\n<dt>decided_by</dt>"),
            "decided_by must not render as generic metadata: {html}"
        );
    }

    #[test]
    fn warning_renders_to_section_with_kind_id_severity_body() {
        let block = make_warning(
            "auth.session.clock-skew",
            "high",
            "Session clocks can drift.",
            std::collections::BTreeMap::new(),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        assert!(
            html.contains(
                "<section class=\"warning warning--high\" id=\"auth.session.clock-skew\">"
            ),
            "missing warning section: {html}"
        );
        assert!(
            html.contains("<span class=\"warning__kind\">warning</span>"),
            "missing kind span: {html}"
        );
        assert!(
            html.contains("<code class=\"warning__id\">auth.session.clock-skew</code>"),
            "missing id code: {html}"
        );
        assert!(
            html.contains("<span class=\"warning__severity\">high</span>"),
            "missing severity span: {html}"
        );
        assert!(
            html.contains("<div class=\"warning__body\"><p>Session clocks can drift.</p></div>"),
            "missing warning body: {html}"
        );
    }

    #[test]
    fn warning_renders_metadata_footer_only_when_fields_are_present() {
        let block = make_warning(
            "auth.session.clock-skew",
            "medium",
            "Session clocks can drift.",
            std::collections::BTreeMap::new(),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);
        assert!(
            !html.contains("<footer class=\"warning__metadata\">"),
            "unexpected metadata footer: {html}"
        );

        let block = make_warning(
            "auth.session.clock-skew",
            "medium",
            "Session clocks can drift.",
            std::collections::BTreeMap::from([("owner".to_string(), "platform".to_string())]),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);
        assert!(
            html.contains("<footer class=\"warning__metadata\">\n<dl>\n"),
            "missing metadata footer: {html}"
        );
        assert!(
            html.contains("<dt>owner</dt><dd>platform</dd>"),
            "missing metadata field: {html}"
        );
    }

    #[test]
    fn warning_html_escapes_body_and_metadata() {
        let block = make_warning(
            "auth.session.clock-skew",
            "critical",
            "clock < token && token > drift",
            std::collections::BTreeMap::from([("note".to_string(), "value 'x' & <y>".to_string())]),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        assert!(
            html.contains("clock &lt; token &amp;&amp; token &gt; drift"),
            "body not escaped: {html}"
        );
        assert!(
            html.contains("<dt>note</dt><dd>value &#39;x&#39; &amp; &lt;y&gt;</dd>"),
            "metadata not escaped: {html}"
        );
    }

    #[test]
    fn thematic_break_renders_as_hr_element() {
        use crate::domain::ast::{PageAst, ThematicBreakAst};
        use crate::domain::identity::PageId;

        let span = dummy_span();
        let page = PageAst {
            id: PageId::from_string("team.guide").expect("test page id is valid"),
            title: None,
            source_path: PathBuf::from("guide.md"),
            blocks: vec![BlockAst::ThematicBreak(ThematicBreakAst {
                source_text: "---".to_string(),
                span,
            })],
        };

        let html = HtmlRenderer.render(&[page]);

        assert!(
            html.contains("<hr />"),
            "expected <hr /> in rendered output; got: {html}"
        );
        assert!(
            !html.contains("quarantined-html"),
            "thematic break must not use quarantine class; got: {html}"
        );
    }

    #[test]
    fn glossary_renders_definition_and_metadata_footer_only_when_fields_are_present() {
        let block = make_glossary(
            "billing.credits",
            "Credits adjust account balances.",
            std::collections::BTreeMap::new(),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);

        assert!(
            html.contains("<section class=\"glossary\" id=\"billing.credits\">"),
            "missing glossary section: {html}"
        );
        assert!(
            html.contains("<span class=\"glossary__kind\">glossary</span>"),
            "missing glossary kind: {html}"
        );
        assert!(
            html.contains("<code class=\"glossary__id\">billing.credits</code>"),
            "missing glossary id: {html}"
        );
        assert!(
            html.contains(
                "<div class=\"glossary__body\"><p>Credits adjust account balances.</p></div>"
            ),
            "missing glossary body: {html}"
        );
        assert!(
            !html.contains("<footer class=\"glossary__metadata\">"),
            "unexpected metadata footer: {html}"
        );

        let block = make_glossary(
            "billing.credits",
            "Credits adjust account balances.",
            std::collections::BTreeMap::from([("status".to_string(), "draft".to_string())]),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, None, &HashSet::new(), &mut html);
        assert!(
            html.contains("<footer class=\"glossary__metadata\">\n<dl>\n"),
            "missing metadata footer: {html}"
        );
        assert!(
            html.contains("<dt>status</dt><dd>draft</dd>"),
            "missing preserved status metadata: {html}"
        );
    }

    // --- Loose list / nested list / tight list renderer tests ---

    /// Tight list: rendered HTML must be byte-for-byte identical to the
    /// pre-existing output pattern — no wrapper elements or extra whitespace
    /// when `item.content` is empty.
    #[test]
    fn tight_list_renders_without_extra_content_elements() {
        use crate::domain::ast::{ListAst, ListItem, ListKind, PageAst};
        use crate::domain::identity::PageId;

        let span = dummy_span;
        let page = PageAst {
            id: PageId::from_string("team.guide").expect("test page id"),
            title: None,
            source_path: PathBuf::from("guide.md"),
            blocks: vec![BlockAst::List(ListAst {
                kind: ListKind::Unordered,
                items: vec![
                    ListItem {
                        inlines: vec![InlineSegment::Text("alpha".to_string())],
                        span: span(),
                        task_state: None,
                        content: Vec::new(),
                    },
                    ListItem {
                        inlines: vec![InlineSegment::Text("beta".to_string())],
                        span: span(),
                        task_state: None,
                        content: Vec::new(),
                    },
                ],
                span: span(),
            })],
        };

        let html = HtmlRenderer.render(&[page]);

        assert!(html.contains("<li>alpha</li>"), "tight item alpha: {html}");
        assert!(html.contains("<li>beta</li>"), "tight item beta: {html}");
        // Ensure the </li> closes immediately after the inline text with no
        // embedded block-level tags.
        assert!(
            !html.contains("<li>alpha<p>"),
            "tight item must not have a <p> inside: {html}"
        );
    }

    /// Loose list: a continuation paragraph inside `item.content` must render
    /// INSIDE the `<li>` — before `</li>` and before `</ul>`.
    #[test]
    fn loose_list_continuation_paragraph_renders_inside_li() {
        use crate::domain::ast::{ListAst, ListItem, ListKind, PageAst, ParagraphAst};
        use crate::domain::identity::PageId;

        let span = dummy_span;
        let page = PageAst {
            id: PageId::from_string("team.guide").expect("test page id"),
            title: None,
            source_path: PathBuf::from("guide.md"),
            blocks: vec![BlockAst::List(ListAst {
                kind: ListKind::Unordered,
                items: vec![ListItem {
                    inlines: vec![InlineSegment::Text("intro line".to_string())],
                    span: span(),
                    task_state: None,
                    content: vec![BlockAst::Paragraph(ParagraphAst {
                        inlines: vec![InlineSegment::Text("continuation text".to_string())],
                        span: span(),
                    })],
                }],
                span: span(),
            })],
        };

        let html = HtmlRenderer.render(&[page]);

        // The continuation text must appear before `</li>`.
        let cont_pos = html
            .find("continuation text")
            .expect("continuation text present");
        let close_li = html.find("</li>").expect("</li> present");
        let close_ul = html.find("</ul>").expect("</ul> present");

        assert!(
            cont_pos < close_li,
            "continuation text must come before </li>; html: {html}"
        );
        assert!(
            close_li < close_ul,
            "</li> must come before </ul>; html: {html}"
        );
    }

    /// Nested sub-list: a `BlockAst::List` inside `item.content` must render
    /// as a nested `<ul>` inside the parent `<li>`.
    #[test]
    fn nested_sub_list_renders_inside_parent_li() {
        use crate::domain::ast::{ListAst, ListItem, ListKind, PageAst};
        use crate::domain::identity::PageId;

        let span = dummy_span;
        let sub_list = BlockAst::List(ListAst {
            kind: ListKind::Unordered,
            items: vec![
                ListItem {
                    inlines: vec![InlineSegment::Text("child one".to_string())],
                    span: span(),
                    task_state: None,
                    content: Vec::new(),
                },
                ListItem {
                    inlines: vec![InlineSegment::Text("child two".to_string())],
                    span: span(),
                    task_state: None,
                    content: Vec::new(),
                },
            ],
            span: span(),
        });
        let page = PageAst {
            id: PageId::from_string("team.guide").expect("test page id"),
            title: None,
            source_path: PathBuf::from("guide.md"),
            blocks: vec![BlockAst::List(ListAst {
                kind: ListKind::Unordered,
                items: vec![ListItem {
                    inlines: vec![InlineSegment::Text("parent".to_string())],
                    span: span(),
                    task_state: None,
                    content: vec![sub_list],
                }],
                span: span(),
            })],
        };

        let html = HtmlRenderer.render(&[page]);

        // Nested <ul> must appear inside <li>parent — before the parent's </li>.
        let parent_li = html.find("<li>parent").expect("parent li present");
        let inner_ul = html[parent_li..].find("<ul>").expect("inner <ul> present");
        let close_li = html[parent_li..]
            .find("</li>")
            .expect("</li> after parent li");

        assert!(
            inner_ul < close_li,
            "inner <ul> must appear before </li>; html: {html}"
        );

        // Both child items must appear.
        assert!(html.contains("<li>child one</li>"), "child one: {html}");
        assert!(html.contains("<li>child two</li>"), "child two: {html}");
    }

    // ── V5.10 TB4: effective_status HTML badge tests ──────────────────────────

    fn make_contradiction(id: &str, status: &str, claim_ids: Vec<&str>) -> BlockAst {
        use crate::domain::knowledge_object::{KnowledgeObject, contradiction::Contradiction};
        let c = Contradiction::try_new(
            id,
            "high",
            status,
            claim_ids,
            "They conflict.",
            std::collections::BTreeMap::new(),
            dummy_span(),
        )
        .expect("valid contradiction");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Contradiction(c)))
    }

    /// A verified+expired claim must render the `--stale` badge.
    /// (This test was missing in TB2; added in TB4 per the task spec.)
    #[test]
    fn verified_expired_claim_renders_stale_badge() {
        use crate::domain::ast::PageAst;
        use crate::domain::identity::PageId;
        use crate::domain::knowledge_object::{
            KnowledgeObject,
            claim::{Claim, Evidence, Owner, Verification, VerifiedAt},
        };

        let span = dummy_span();
        let owner = Owner::try_new("team-billing").expect("owner");
        let verified_at = VerifiedAt::try_new("2025-01-01").expect("verified_at");
        let source_ev = Evidence::from_field("source", "ledger").expect("evidence");
        let verification = Verification::new(owner, verified_at, vec![source_ev]);
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("expires_at".to_string(), "2025-06-01".to_string());
        let stale_claim = Claim::try_new(
            "billing.stale",
            Some("verified"),
            "Claim body.",
            fields,
            Some(verification),
            span,
        )
        .expect("valid verified claim");

        let page = PageAst {
            id: PageId::from_string("docs.billing").expect("page id"),
            title: None,
            source_path: std::path::PathBuf::from("docs/billing.adoc"),
            blocks: vec![BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(
                stale_claim,
            )))],
        };
        // today is after expires_at
        let today = chrono::NaiveDate::from_ymd_opt(2026, 6, 1).expect("valid date");
        let html = HtmlRenderer.render_workspace_for_date(
            &crate::domain::ast::WorkspaceAst { pages: vec![page] },
            Some(today),
        );

        assert!(
            html.contains(
                "<span class=\"ko__effective-status ko__effective-status--stale\">stale</span>"
            ),
            "verified expired claim must render stale badge; html: {html}"
        );
    }

    /// A claim referenced by an unresolved contradiction must render the
    /// `--contradicted` badge.
    #[test]
    fn claim_referenced_by_unresolved_contradiction_renders_contradicted_badge() {
        use crate::domain::ast::PageAst;
        use crate::domain::identity::PageId;

        let page = PageAst {
            id: PageId::from_string("docs.auth").expect("page id"),
            title: None,
            source_path: std::path::PathBuf::from("docs/auth.adoc"),
            blocks: vec![
                make_claim(
                    "auth.a",
                    "plain",
                    "Claim A.",
                    std::collections::BTreeMap::new(),
                    dummy_span(),
                ),
                make_claim(
                    "auth.b",
                    "plain",
                    "Claim B.",
                    std::collections::BTreeMap::new(),
                    dummy_span(),
                ),
                make_contradiction("auth.conflict", "unresolved", vec!["auth.a", "auth.b"]),
            ],
        };

        let html = HtmlRenderer.render_workspace_for_date(
            &crate::domain::ast::WorkspaceAst { pages: vec![page] },
            None,
        );

        assert!(
            html.contains(
                "<span class=\"ko__effective-status ko__effective-status--contradicted\">contradicted</span>"
            ),
            "claim referenced by unresolved contradiction must render contradicted badge; html: {html}"
        );

        // Both claims should have the badge.
        let count = html.matches("ko__effective-status--contradicted").count();
        assert_eq!(
            count, 2,
            "both claims must have contradicted badge; html: {html}"
        );
    }

    /// A claim that is both stale (verified+expired) AND referenced by an
    /// unresolved contradiction must render only the `--stale` badge (stale wins).
    #[test]
    fn stale_plus_contradicted_claim_renders_only_stale_badge() {
        use crate::domain::ast::PageAst;
        use crate::domain::identity::PageId;
        use crate::domain::knowledge_object::{
            KnowledgeObject,
            claim::{Claim, Evidence, Owner, Verification, VerifiedAt},
        };

        let span = dummy_span();
        let owner = Owner::try_new("team-billing").expect("owner");
        let verified_at = VerifiedAt::try_new("2025-01-01").expect("verified_at");
        let source_ev = Evidence::from_field("source", "ledger").expect("evidence");
        let verification = Verification::new(owner, verified_at, vec![source_ev]);
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("expires_at".to_string(), "2025-06-01".to_string());
        let stale_claim = Claim::try_new(
            "auth.stale",
            Some("verified"),
            "Stale body.",
            fields,
            Some(verification),
            span.clone(),
        )
        .expect("stale claim");

        let plain_claim = Claim::try_new(
            "auth.plain",
            Some("plain"),
            "Plain body.",
            std::collections::BTreeMap::new(),
            None,
            span,
        )
        .expect("plain claim");

        let page = PageAst {
            id: PageId::from_string("docs.auth").expect("page id"),
            title: None,
            source_path: std::path::PathBuf::from("docs/auth.adoc"),
            blocks: vec![
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(stale_claim))),
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(plain_claim))),
                make_contradiction(
                    "auth.conflict",
                    "unresolved",
                    vec!["auth.stale", "auth.plain"],
                ),
            ],
        };
        let today = chrono::NaiveDate::from_ymd_opt(2026, 6, 1).expect("valid date");
        let html = HtmlRenderer.render_workspace_for_date(
            &crate::domain::ast::WorkspaceAst { pages: vec![page] },
            Some(today),
        );

        // auth.stale must render stale, not contradicted.
        assert!(
            html.contains(
                "<span class=\"ko__effective-status ko__effective-status--stale\">stale</span>"
            ),
            "stale+contradicted claim must render stale badge; html: {html}"
        );
        // auth.plain must render contradicted (it's not stale, just contradicted).
        assert!(
            html.contains(
                "<span class=\"ko__effective-status ko__effective-status--contradicted\">contradicted</span>"
            ),
            "plain+contradicted claim must render contradicted badge; html: {html}"
        );
        // auth.stale must NOT render the contradicted badge.
        let stale_section_start = html.find("id=\"auth.stale\"").expect("auth.stale section");
        let stale_section_end = html[stale_section_start..]
            .find("</section>")
            .expect("close section")
            + stale_section_start;
        let stale_section = &html[stale_section_start..stale_section_end];
        assert!(
            !stale_section.contains("ko__effective-status--contradicted"),
            "stale claim section must not contain contradicted badge; section: {stale_section}"
        );
    }

    /// A resolved contradiction must NOT cause a `contradicted` badge.
    #[test]
    fn resolved_contradiction_does_not_render_contradicted_badge() {
        use crate::domain::ast::PageAst;
        use crate::domain::identity::PageId;

        let page = PageAst {
            id: PageId::from_string("docs.auth").expect("page id"),
            title: None,
            source_path: std::path::PathBuf::from("docs/auth.adoc"),
            blocks: vec![
                make_claim(
                    "auth.a",
                    "plain",
                    "Claim A.",
                    std::collections::BTreeMap::new(),
                    dummy_span(),
                ),
                make_claim(
                    "auth.b",
                    "plain",
                    "Claim B.",
                    std::collections::BTreeMap::new(),
                    dummy_span(),
                ),
                make_contradiction("auth.conflict", "resolved", vec!["auth.a", "auth.b"]),
            ],
        };

        let html = HtmlRenderer.render_workspace_for_date(
            &crate::domain::ast::WorkspaceAst { pages: vec![page] },
            None,
        );

        assert!(
            !html.contains("ko__effective-status--contradicted"),
            "resolved contradiction must not produce contradicted badge; html: {html}"
        );
    }
}
