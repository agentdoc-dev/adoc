use crate::domain::ast::{BlockAst, ListKind, PageAst};
use crate::domain::inline::InlineSegment;
use crate::domain::knowledge_object::{
    KnowledgeObject,
    claim::{Claim, Evidence},
    decision::{DECIDED_BY_FIELD, Decision},
    glossary::Glossary,
    warning::Warning,
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
        BlockAst::KnowledgeObject(ko) => match ko.as_ref() {
            KnowledgeObject::Claim(claim) => {
                render_claim(claim, html);
            }
            KnowledgeObject::Decision(decision) => {
                render_decision(decision, html);
            }
            KnowledgeObject::Glossary(glossary) => {
                render_glossary(glossary, html);
            }
            KnowledgeObject::Warning(warning) => {
                render_warning(warning, html);
            }
        },
        BlockAst::KnowledgeObjectPending(_) => {
            unreachable!("resolver must replace pending knowledge objects before rendering")
        }
    }
}

fn render_glossary(glossary: &Glossary, html: &mut String) {
    html.push_str("<section class=\"glossary\" id=\"");
    html.push_str(&escape_html(glossary.id().as_str()));
    html.push_str("\">\n");
    html.push_str("<header class=\"glossary__header\">");
    html.push_str("<span class=\"glossary__kind\">glossary</span>");
    html.push_str("<code class=\"glossary__id\">");
    html.push_str(&escape_html(glossary.id().as_str()));
    html.push_str("</code>");
    html.push_str("</header>\n");
    html.push_str("<div class=\"glossary__body\"><p>");
    render_inlines(glossary.body().inlines(), html);
    html.push_str("</p></div>\n");
    render_glossary_metadata(glossary, html);
    html.push_str("</section>\n");
}

fn render_glossary_metadata(glossary: &Glossary, html: &mut String) {
    if glossary.fields().is_empty() {
        return;
    }

    html.push_str("<footer class=\"glossary__metadata\">\n");
    html.push_str("<dl>\n");
    for (key, value) in glossary.fields().iter() {
        html.push_str("<dt>");
        html.push_str(&escape_html(key));
        html.push_str("</dt><dd>");
        html.push_str(&escape_html(value));
        html.push_str("</dd>\n");
    }
    html.push_str("</dl>\n");
    html.push_str("</footer>\n");
}

fn render_warning(warning: &Warning, html: &mut String) {
    html.push_str("<section class=\"warning warning--");
    html.push_str(warning.severity().as_str());
    html.push_str("\" id=\"");
    html.push_str(&escape_html(warning.id().as_str()));
    html.push_str("\">\n");
    html.push_str("<header class=\"warning__header\">");
    html.push_str("<span class=\"warning__kind\">warning</span>");
    html.push_str("<code class=\"warning__id\">");
    html.push_str(&escape_html(warning.id().as_str()));
    html.push_str("</code>");
    html.push_str("<span class=\"warning__severity\">");
    html.push_str(&escape_html(warning.severity().as_str()));
    html.push_str("</span>");
    html.push_str("</header>\n");
    html.push_str("<div class=\"warning__body\"><p>");
    render_inlines(warning.body().inlines(), html);
    html.push_str("</p></div>\n");
    render_warning_metadata(warning, html);
    html.push_str("</section>\n");
}

fn render_warning_metadata(warning: &Warning, html: &mut String) {
    if warning.fields().is_empty() {
        return;
    }

    html.push_str("<footer class=\"warning__metadata\">\n");
    html.push_str("<dl>\n");
    for (key, value) in warning.fields().iter() {
        html.push_str("<dt>");
        html.push_str(&escape_html(key));
        html.push_str("</dt><dd>");
        html.push_str(&escape_html(value));
        html.push_str("</dd>\n");
    }
    html.push_str("</dl>\n");
    html.push_str("</footer>\n");
}

fn render_decision(decision: &Decision, html: &mut String) {
    let class = if decision.verdict().is_some() {
        "decision decision--accepted"
    } else {
        "decision"
    };
    html.push_str("<section class=\"");
    html.push_str(class);
    html.push_str("\" id=\"");
    html.push_str(&escape_html(decision.id().as_str()));
    html.push_str("\">\n");
    html.push_str("<header class=\"decision__header\">");
    html.push_str("<span class=\"decision__kind\">decision</span>");
    html.push_str("<code class=\"decision__id\">");
    html.push_str(&escape_html(decision.id().as_str()));
    html.push_str("</code>");
    html.push_str("<span class=\"decision__status\">");
    html.push_str(&escape_html(decision.status().as_str()));
    html.push_str("</span>");
    html.push_str("</header>\n");
    html.push_str("<div class=\"decision__body\"><p>");
    render_inlines(decision.body().inlines(), html);
    html.push_str("</p></div>\n");
    if let Some(verdict) = decision.verdict() {
        html.push_str("<div class=\"decision__verdict\"><dl>");
        html.push_str("<div class=\"decision__verdict-item\"><dt>");
        html.push_str(DECIDED_BY_FIELD);
        html.push_str("</dt><dd>");
        html.push_str(&escape_html(verdict.decided_by().as_str()));
        html.push_str("</dd></div>");
        html.push_str("</dl></div>\n");
    }
    render_decision_metadata(decision, html);
    html.push_str("</section>\n");
}

fn render_decision_metadata(decision: &Decision, html: &mut String) {
    if decision.fields().is_empty() {
        return;
    }

    html.push_str("<footer class=\"decision__metadata\">\n");
    html.push_str("<dl>\n");
    for (key, value) in decision.fields().iter() {
        html.push_str("<dt>");
        html.push_str(&escape_html(key));
        html.push_str("</dt><dd>");
        html.push_str(&escape_html(value));
        html.push_str("</dd>\n");
    }
    html.push_str("</dl>\n");
    html.push_str("</footer>\n");
}

fn render_claim(claim: &Claim, html: &mut String) {
    let class = if claim.verification().is_some() {
        "claim claim--verified"
    } else {
        "claim"
    };
    html.push_str("<section class=\"");
    html.push_str(class);
    html.push_str("\" id=\"");
    html.push_str(&escape_html(claim.id().as_str()));
    html.push_str("\">\n");
    html.push_str("<header class=\"claim__header\">");
    html.push_str("<span class=\"claim__kind\">claim</span>");
    html.push_str("<code class=\"claim__id\">");
    html.push_str(&escape_html(claim.id().as_str()));
    html.push_str("</code>");
    html.push_str("<span class=\"claim__status\">");
    html.push_str(&escape_html(claim.status().as_str()));
    html.push_str("</span>");
    html.push_str("</header>\n");
    html.push_str("<div class=\"claim__body\"><p>");
    render_inlines(claim.body().inlines(), html);
    html.push_str("</p></div>\n");

    if let Some(verification) = claim.verification() {
        html.push_str("<div class=\"claim__verification\">\n");
        html.push_str("<dl>\n");
        html.push_str("<div class=\"claim__verification-item\"><dt>owner</dt><dd>");
        html.push_str(&escape_html(verification.owner().as_str()));
        html.push_str("</dd></div>\n");
        html.push_str("<div class=\"claim__verification-item\"><dt>verified_at</dt><dd>");
        html.push_str(&escape_html(verification.verified_at().as_str()));
        html.push_str("</dd></div>\n");
        html.push_str("</dl>\n");
        html.push_str("</div>\n");

        html.push_str("<div class=\"claim__evidence\">\n");
        html.push_str("<dl>\n");
        for evidence in verification.evidence() {
            render_evidence(evidence, html);
        }
        html.push_str("</dl>\n");
        html.push_str("</div>\n");
    }

    render_claim_metadata(claim, html);
    html.push_str("</section>\n");
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

fn render_claim_metadata(claim: &Claim, html: &mut String) {
    if claim.fields().is_empty() {
        return;
    }

    html.push_str("<footer class=\"claim__metadata\">\n");
    html.push_str("<dl>\n");
    for (key, value) in claim.fields().iter() {
        html.push_str("<dt>");
        html.push_str(&escape_html(key));
        html.push_str("</dt><dd>");
        html.push_str(&escape_html(value));
        html.push_str("</dd>\n");
    }
    html.push_str("</dl>\n");
    html.push_str("</footer>\n");
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
            html.push_str("<a class=\"object-ref\" href=\"#");
            html.push_str(&escape_html(id.as_str()));
            html.push_str("\">");
            html.push_str(&escape_html(id.as_str()));
            html.push_str("</a>");
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
