use crate::ast::{BlockAst, ListKind, PageAst};
use crate::inline::InlineSegment;

pub fn render_html(pages: &[PageAst]) -> String {
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
                render_inlines(item, html);
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
    }
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
        InlineSegment::Link { text, url } => {
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
    use super::*;

    fn render(segments: &[InlineSegment]) -> String {
        let mut html = String::new();
        render_inlines(segments, &mut html);
        html
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
        }]);
        assert_eq!(
            html,
            "<a href=\"https://example.test/?q=&quot;a&amp;b&quot;\">docs</a>"
        );
    }

    #[test]
    fn render_html_flows_inlines_through_heading_paragraph_and_list_item() {
        use crate::ast::{HeadingAst, ListAst, ListKind, ParagraphAst};
        use crate::diagnostic::{SourcePosition, SourceSpan};
        use crate::identity::PageId;
        use std::path::PathBuf;

        fn span() -> SourceSpan {
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

        let page = PageAst {
            id: PageId::from_string("guide"),
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
                    items: vec![vec![
                        InlineSegment::Text("Run ".to_string()),
                        InlineSegment::Code("adoc check".to_string()),
                    ]],
                    span: span(),
                }),
            ],
        };

        let html = render_html(&[page]);

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
        }]);
        assert_eq!(
            html,
            "<a href=\"https://example.test\">see <code>adoc</code></a>"
        );
    }
}
