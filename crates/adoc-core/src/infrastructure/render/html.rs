use crate::domain::ast::{BlockAst, ListKind, PageAst};
use crate::domain::inline::InlineSegment;
use crate::domain::knowledge_object::{
    KnowledgeObject,
    claim::{Claim, Evidence},
    decision::{DECIDED_BY_FIELD, Decision},
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
        },
        BlockAst::KnowledgeObjectPending(_) => {
            unreachable!("resolver must replace pending knowledge objects before rendering")
        }
    }
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
    html.push_str(&escape_html(decision.body().as_str()));
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
    html.push_str(&escape_html(claim.body().as_str()));
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
            claim::{Claim, Evidence, NonEmpty, Owner, Verification, VerifiedAt},
        };
        let verification = Verification::new(
            Owner::try_new(fields.get("owner").expect("owner")).expect("owner"),
            VerifiedAt::try_new(fields.get("verified_at").expect("verified_at"))
                .expect("verified_at"),
            NonEmpty::from_vec(vec![
                Evidence::source(fields.get("source").expect("source")).expect("source"),
            ])
            .expect("non-empty evidence"),
        );
        let claim = Claim::try_new(id, Some("verified"), body, fields, Some(verification), span)
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
}
