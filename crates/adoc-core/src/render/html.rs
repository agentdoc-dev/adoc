use crate::ast::{BlockAst, ListKind, PageAst};
use crate::inline::InlineSegment;

pub fn render_html(pages: &[PageAst]) -> String {
    let mut html = String::from(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>AgentDoc</title>\n</head>\n<body>\n",
    );

    for page in pages {
        html.push_str("<article data-page-id=\"");
        html.push_str(&escape_html(&page.id));
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
