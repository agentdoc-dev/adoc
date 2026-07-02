//! Resolve one parser-produced typed-block shell into a Knowledge Object.
//!
//! This service owns the supported-kind registry and dispatches to the
//! aggregate-specific `build_from_parsed` adapters. It does not walk or mutate
//! pages; the application resolver stage owns that pipeline concern.

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::identity::ObjectId;
use crate::domain::knowledge_object::{
    BlockKind, KnowledgeObject, agent_instruction::AgentInstruction, api::Api, claim::Claim,
    constraint::Constraint, contradiction::Contradiction, decision::Decision, example::Example,
    glossary::Glossary, observation::Observation, policy::Policy, procedure::Procedure,
    source::Source, warning::Warning,
};

type KnowledgeObjectBuilder = fn(ParsedTypedBlock, &mut Vec<Diagnostic>) -> Option<KnowledgeObject>;

struct KnowledgeObjectResolver {
    kind: BlockKind,
    build: KnowledgeObjectBuilder,
}

const UNKNOWN_KIND_DEFERRED_HELP: &str = "Custom schemas are deferred.";

const RESOLVERS: &[KnowledgeObjectResolver] = &[
    KnowledgeObjectResolver {
        kind: BlockKind::Claim,
        build: build_claim,
    },
    KnowledgeObjectResolver {
        kind: BlockKind::Decision,
        build: build_decision,
    },
    KnowledgeObjectResolver {
        kind: BlockKind::Glossary,
        build: build_glossary,
    },
    KnowledgeObjectResolver {
        kind: BlockKind::Warning,
        build: build_warning,
    },
    KnowledgeObjectResolver {
        kind: BlockKind::Constraint,
        build: build_constraint,
    },
    KnowledgeObjectResolver {
        kind: BlockKind::Policy,
        build: build_policy,
    },
    KnowledgeObjectResolver {
        kind: BlockKind::Procedure,
        build: build_procedure,
    },
    KnowledgeObjectResolver {
        kind: BlockKind::Example,
        build: build_example,
    },
    KnowledgeObjectResolver {
        kind: BlockKind::AgentInstruction,
        build: build_agent_instruction,
    },
    KnowledgeObjectResolver {
        kind: BlockKind::Contradiction,
        build: build_contradiction,
    },
    KnowledgeObjectResolver {
        kind: BlockKind::Source,
        build: build_source,
    },
    KnowledgeObjectResolver {
        kind: BlockKind::Api,
        build: build_api,
    },
    KnowledgeObjectResolver {
        kind: BlockKind::Observation,
        build: build_observation,
    },
];

pub(crate) fn resolve_pending_block(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    let Some(kind) = BlockKind::from_fence_word(&parsed.kind_word) else {
        diagnostics.push(unknown_kind_diagnostic(&parsed));
        return None;
    };

    let resolver = resolver_for(kind).expect("every BlockKind must have a pending-block resolver");
    (resolver)(parsed, diagnostics)
}

fn resolver_for(_kind: BlockKind) -> Option<KnowledgeObjectBuilder> {
    RESOLVERS
        .iter()
        .find(|resolver| resolver.kind == _kind)
        .map(|resolver| resolver.build)
}

pub(crate) fn is_supported_kind_word(kind_word: &str) -> bool {
    BlockKind::from_fence_word(kind_word).is_some()
}

pub(crate) fn unknown_kind_help() -> String {
    let supported = BlockKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("supported kinds: {supported}. {UNKNOWN_KIND_DEFERRED_HELP}")
}

fn build_claim(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    Claim::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::Claim)
}

fn build_decision(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    Decision::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::Decision)
}

fn build_glossary(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    Glossary::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::Glossary)
}

fn build_warning(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    Warning::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::Warning)
}

fn build_constraint(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    Constraint::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::Constraint)
}

fn build_policy(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    Policy::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::Policy)
}

fn build_procedure(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    Procedure::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::Procedure)
}

fn build_example(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    Example::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::Example)
}

fn build_agent_instruction(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    AgentInstruction::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::AgentInstruction)
}

fn build_contradiction(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    Contradiction::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::Contradiction)
}

fn build_source(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    Source::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::Source)
}

fn build_api(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    Api::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::Api)
}

fn build_observation(
    parsed: ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<KnowledgeObject> {
    Observation::build_from_parsed(parsed, diagnostics).map(KnowledgeObject::Observation)
}

fn unknown_kind_diagnostic(parsed: &ParsedTypedBlock) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        DiagnosticCode::SchemaUnknownKind,
        format!("unknown typed-block kind `{}`", parsed.kind_word),
    )
    .with_span(parsed.kind_word_span.clone())
    .with_help(unknown_kind_help());

    if ObjectId::new(parsed.id_text.clone()).is_ok() {
        diagnostic = diagnostic.with_object_id(&parsed.id_text);
    }

    diagnostic
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{DiagnosticCode, SourcePosition, SourceSpan};
    use crate::domain::knowledge_object::claim::STATUS_FIELD;

    fn span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("test.adoc"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 8,
                offset: 7,
            },
        }
    }

    fn pending(kind_word: &str, id_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: kind_word.to_string(),
            kind_word_span: span(),
            id_text: id_text.to_string(),
            raw_fields: BTreeMap::from([(STATUS_FIELD.to_string(), "draft".to_string())]),
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "The system credits users automatically.".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(
                "The system credits users automatically.",
            ),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
        }
    }

    #[test]
    fn resolves_claim_pending_block() {
        let mut diagnostics = Vec::new();

        let object = resolve_pending_block(pending("claim", "billing.credits"), &mut diagnostics);

        assert!(
            diagnostics.is_empty(),
            "expected no diagnostics: {diagnostics:?}"
        );
        let Some(KnowledgeObject::Claim(claim)) = object else {
            panic!("expected claim KnowledgeObject");
        };
        assert_eq!(claim.id().as_str(), "billing.credits");
    }

    #[test]
    fn emits_schema_unknown_kind_for_valid_id() {
        let mut diagnostics = Vec::new();

        let object = resolve_pending_block(pending("fact", "billing.policy"), &mut diagnostics);

        assert!(object.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaUnknownKind);
        assert_eq!(diagnostics[0].span.as_ref(), Some(&span()));
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.policy"));
        assert!(
            diagnostics[0]
                .help
                .as_deref()
                .is_some_and(|help| help.contains("claim"))
        );
    }

    #[test]
    fn emits_schema_unknown_kind_without_object_id_when_id_is_invalid() {
        let mut diagnostics = Vec::new();

        let object = resolve_pending_block(pending("fact", "Billing.Policy"), &mut diagnostics);

        assert!(object.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaUnknownKind);
        assert!(diagnostics[0].object_id.is_none());
    }

    #[test]
    fn every_supported_block_kind_has_a_resolver() {
        for kind in BlockKind::ALL {
            assert!(
                resolver_for(*kind).is_some(),
                "missing resolver for `{}`",
                kind.as_str()
            );
        }
    }

    #[test]
    fn unknown_kind_help_lists_supported_kinds_from_registry() {
        let help = unknown_kind_help();

        assert_eq!(
            help,
            "supported kinds: claim, decision, glossary, warning, constraint, policy, procedure, example, agent_instruction, contradiction, source, api, observation. \
            Custom schemas are deferred."
        );
        for kind in BlockKind::ALL {
            assert!(
                help.contains(kind.as_str()),
                "help must mention supported kind `{}`: {help}",
                kind.as_str()
            );
        }
    }
}
