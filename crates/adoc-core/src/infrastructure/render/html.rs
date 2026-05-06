use crate::domain::ast::{BlockAst, ListKind, PageAst};
use crate::domain::inline::InlineSegment;
use crate::domain::knowledge_object::{
    KnowledgeObject, RelationField, Relations,
    claim::Evidence,
    projection::{KnowledgeObjectMetadata, MetadataDiscriminant, MetadataField},
};
use crate::domain::ports::renderer::Renderer;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct HtmlRenderer;

impl Renderer for HtmlRenderer {
    fn render(&self, pages: &[PageAst]) -> String {
        let mut html = String::from(
            "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>AgentDoc</title>\n</head>\n<body>\n",
        );

        for page in pages {
            html.push_str("<article data-page-id=\"");
            html.push_str(&escape_html(page.id.as_str()));
            html.push_str("\">\n");

            for block in &page.blocks {
                render_block(block, &mut html);
            }

            html.push_str("</article>\n");
        }

        html.push_str("</body>\n</html>\n");
        html
    }
}

fn render_block(block: &BlockAst, html: &mut String) {
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
            html.push('<');
            html.push_str(tag);
            html.push_str(">\n");
            for item in &list.items {
                html.push_str("<li>");
                render_inlines(&item.inlines, html);
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
        BlockAst::KnowledgeObject(ko) => render_knowledge_object(ko, html),
        BlockAst::KnowledgeObjectPending(_) => {
            unreachable!("resolver must replace pending knowledge objects before rendering")
        }
    }
}

fn render_knowledge_object(knowledge_object: &KnowledgeObject, html: &mut String) {
    let metadata = knowledge_object.metadata_projection();

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
    }
}

fn render_glossary(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    render_object_section_open(knowledge_object, "glossary", html);
    render_object_header(knowledge_object, metadata.discriminant(), html);
    render_object_body(knowledge_object, html);
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
}

fn render_warning(
    knowledge_object: &KnowledgeObject,
    metadata: &KnowledgeObjectMetadata<'_>,
    html: &mut String,
) {
    let class = match metadata.discriminant() {
        Some(discriminant) => format!("warning warning--{}", discriminant.value_as_str()),
        None => "warning".to_string(),
    };

    render_object_section_open(knowledge_object, &class, html);
    render_object_header(knowledge_object, metadata.discriminant(), html);
    render_object_body(knowledge_object, html);
    render_object_metadata(knowledge_object, metadata, html);
    html.push_str("</section>\n");
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
    render_object_header(knowledge_object, metadata.discriminant(), html);
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
    render_object_header(knowledge_object, metadata.discriminant(), html);
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
    metadata.fields().iter().filter_map(|field| match field {
        MetadataField::Evidence(evidence) => Some(*evidence),
        _ => None,
    })
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
    let modifier = match evidence {
        Evidence::Source(_) => "source",
        Evidence::Test(_) => "test",
        Evidence::ReviewedBy(_) => "reviewed-by",
    };
    html.push_str("<div class=\"claim__evidence-item claim__evidence-item--");
    html.push_str(modifier);
    html.push_str("\"><dt>");
    html.push_str(evidence.field_key());
    html.push_str("</dt><dd>");
    html.push_str(&escape_html(evidence.value().as_str()));
    html.push_str("</dd></div>\n");
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

fn render_object_header(
    knowledge_object: &KnowledgeObject,
    discriminant: Option<MetadataDiscriminant<'_>>,
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

    if let Some(discriminant) = discriminant {
        html.push_str("<span class=\"");
        html.push_str(kind);
        html.push_str("__");
        html.push_str(discriminant_field_name(discriminant));
        html.push_str("\">");
        html.push_str(&escape_html(discriminant.value_as_str()));
        html.push_str("</span>");
    }

    html.push_str("</header>\n");
}

fn discriminant_field_name(discriminant: MetadataDiscriminant<'_>) -> &'static str {
    match discriminant {
        MetadataDiscriminant::ClaimStatus(_) | MetadataDiscriminant::DecisionStatus(_) => "status",
        MetadataDiscriminant::WarningSeverity(_) => "severity",
    }
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
    for field in RelationField::ALL {
        render_relation_group(field, relations.targets(field), html);
    }
    html.push_str("</dl></section>\n");
}

fn render_relation_group(
    field: RelationField,
    targets: &[crate::domain::knowledge_object::RelationTarget],
    html: &mut String,
) {
    if targets.is_empty() {
        return;
    }

    html.push_str("<dt>");
    html.push_str(field.as_str());
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
            html.push_str("<a href=\"");
            html.push_str(&escape_html(url));
            html.push_str("\">");
            render_inlines(text, html);
            html.push_str("</a>");
        }
        InlineSegment::ObjectReference { id, .. } => {
            render_object_ref_anchor(id.as_str(), html);
        }
        InlineSegment::ObjectReferencePending { .. } => {
            unreachable!("object references must resolve before rendering")
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
        use crate::domain::ast::{HeadingAst, ListAst, ListItem, ListKind, ParagraphAst};
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
                Claim, Evidence, NonEmpty, OWNER_FIELD, Owner, REVIEWED_BY_FIELD, SOURCE_FIELD,
                TEST_FIELD, VERIFIED_AT_FIELD, Verification, VerifiedAt,
            },
        };
        let verification = Verification::new(
            Owner::try_new(fields.get(OWNER_FIELD).expect("owner")).expect("owner"),
            VerifiedAt::try_new(fields.get(VERIFIED_AT_FIELD).expect("verified_at"))
                .expect("verified_at"),
            NonEmpty::from_vec(vec![
                Evidence::source(fields.get(SOURCE_FIELD).expect("source")).expect("source"),
            ])
            .expect("non-empty evidence"),
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
        let verdict = AcceptedVerdict::new(DecidedBy::try_new("architecture").expect("decided_by"));
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
        render_block(&block, &mut html);

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
        render_block(&block, &mut html);

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
        render_block(&block, &mut html);

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
        render_block(&block, &mut html);

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
        render_block(&block, &mut html);

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
        render_block(&block, &mut html);

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
        render_block(&block, &mut html);

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
        assert!(
            html.contains(
                "<div class=\"claim__evidence-item claim__evidence-item--source\"><dt>source</dt><dd>payments ledger</dd></div>"
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
        render_block(&block, &mut html);

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
        render_block(&block, &mut html);

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
        render_block(&block, &mut html);
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
        render_block(&block, &mut html);
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
        render_block(&block, &mut html);

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
    fn glossary_renders_definition_and_metadata_footer_only_when_fields_are_present() {
        let block = make_glossary(
            "billing.credits",
            "Credits adjust account balances.",
            std::collections::BTreeMap::new(),
            dummy_span(),
        );
        let mut html = String::new();
        render_block(&block, &mut html);

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
        render_block(&block, &mut html);
        assert!(
            html.contains("<footer class=\"glossary__metadata\">\n<dl>\n"),
            "missing metadata footer: {html}"
        );
        assert!(
            html.contains("<dt>status</dt><dd>draft</dd>"),
            "missing preserved status metadata: {html}"
        );
    }
}
