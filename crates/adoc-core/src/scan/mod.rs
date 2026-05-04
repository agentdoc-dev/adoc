//! Pure text-scanning utilities used by validation rules.
//!
//! Scanners are stateless functions that recognize patterns at a tag boundary
//! (start of line, after whitespace, etc.) and return a column-anchored match
//! struct. They have no validator-specific logic — no diagnostic codes, no
//! AST awareness — so they can be unit-tested at their own interface and
//! reused by future rules or by the parser. See ADR-0007.

pub(crate) mod raw_html;
