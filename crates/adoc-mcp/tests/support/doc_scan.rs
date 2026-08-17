//! Fence-aware doc scanning shared by the docs-truth guards (ADR-0041).
//! Lines inside ``` fences are sample text, never document structure — they
//! must neither terminate a section nor contribute phantom headings, labels,
//! or boundary claims. One implementation, so the guards' notion of
//! "structural" can never drift apart.

/// Yields `(1-based line number, line)` outside ``` fences.
pub fn structural_lines(content: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut in_fence = false;
    content.lines().enumerate().filter_map(move |(i, line)| {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            return None;
        }
        (!in_fence).then_some((i + 1, line))
    })
}
