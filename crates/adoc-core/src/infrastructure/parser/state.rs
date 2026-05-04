//! Block-in-progress state machine for `parse_page`.
//!
//! Replaces the previous tuple of mutable variables (`paragraph_lines`,
//! `paragraph_start_line`, `paragraph_end_line`, `pending_list`, plus an
//! inline code-fence sub-loop) with a single typed enum. Each variant owns
//! its in-progress builder; `flush_in_place` consumes the current state
//! (leaving `Idle` in its place) and emits any completed block plus the
//! optional structural diagnostic that goes with it (e.g. an unclosed code
//! fence at EOF).

use crate::domain::ast::BlockAst;
use crate::domain::diagnostic::Diagnostic;
use crate::domain::source::SourceFile;

use super::builders::{CodeBlockBuilder, ListBuilder, ParagraphBuilder};

pub(super) enum ParseState {
    Idle,
    Paragraph(ParagraphBuilder),
    List(ListBuilder),
    CodeBlock(CodeBlockBuilder),
}

impl ParseState {
    pub(super) fn is_in_code_block(&self) -> bool {
        matches!(self, Self::CodeBlock(_))
    }

    /// Flush the in-progress block, leaving `Idle` in place.
    pub(super) fn flush_in_place(&mut self, source: &SourceFile) -> FlushOutcome {
        std::mem::replace(self, ParseState::Idle).flush(source)
    }

    /// Flush an owned `ParseState` value. Use after `std::mem::replace` when
    /// the caller already needs to take the state by value to fork on its
    /// variant before re-binding.
    pub(super) fn flush(self, source: &SourceFile) -> FlushOutcome {
        match self {
            Self::Idle => FlushOutcome::default(),
            Self::Paragraph(builder) => FlushOutcome {
                block: Some(builder.finish(source)),
                diagnostic: None,
            },
            Self::List(builder) => FlushOutcome {
                block: Some(BlockAst::List(builder.finish())),
                diagnostic: None,
            },
            Self::CodeBlock(builder) => {
                let (block, diagnostic) = builder.finish();
                FlushOutcome {
                    block: Some(block),
                    diagnostic,
                }
            }
        }
    }
}

#[derive(Default)]
pub(super) struct FlushOutcome {
    pub(super) block: Option<BlockAst>,
    pub(super) diagnostic: Option<Diagnostic>,
}
