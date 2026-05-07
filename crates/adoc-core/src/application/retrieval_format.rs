use std::error::Error;
use std::fmt::{self, Write};

use crate::application::retrieval::RetrievalEnvelope;
use crate::domain::artifact::AgentJsonRelations;
use crate::domain::retrieval::RetrievalRecord;

pub trait RetrievalFormatter {
    fn render(&self, envelope: &RetrievalEnvelope) -> Result<String, RetrievalFormatError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TextRetrievalFormatter;

#[derive(Debug, Clone, Copy, Default)]
pub struct JsonRetrievalFormatter;

#[derive(Debug)]
#[non_exhaustive]
pub enum RetrievalFormatError {
    JsonSerialize { source: serde_json::Error },
}

impl fmt::Display for RetrievalFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsonSerialize { source } => {
                write!(formatter, "could not serialize retrieval JSON: {source}")
            }
        }
    }
}

impl Error for RetrievalFormatError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::JsonSerialize { source } => Some(source),
        }
    }
}

impl RetrievalFormatter for TextRetrievalFormatter {
    fn render(&self, envelope: &RetrievalEnvelope) -> Result<String, RetrievalFormatError> {
        let mut output = String::new();

        for (index, record) in envelope.records.iter().enumerate() {
            if index > 0 {
                output.push('\n');
            }
            render_record(&mut output, record);
        }

        Ok(output)
    }
}

impl RetrievalFormatter for JsonRetrievalFormatter {
    fn render(&self, envelope: &RetrievalEnvelope) -> Result<String, RetrievalFormatError> {
        serde_json::to_string_pretty(envelope)
            .map_err(|source| RetrievalFormatError::JsonSerialize { source })
    }
}

fn render_record(output: &mut String, record: &RetrievalRecord) {
    writeln!(output, "Object: {}", record.id).expect("writing to String cannot fail");
    writeln!(output, "Kind: {}", record.kind).expect("writing to String cannot fail");
    if let Some(status) = &record.status {
        if record.kind == "warning" {
            writeln!(output, "Severity: {status}").expect("writing to String cannot fail");
        } else {
            writeln!(output, "Status: {status}").expect("writing to String cannot fail");
        }
    }
    if let Some(owner) = &record.owner {
        writeln!(output, "Owner: {owner}").expect("writing to String cannot fail");
    }
    if let Some(verified_at) = &record.verified_at {
        writeln!(output, "Verified: {verified_at}").expect("writing to String cannot fail");
    }

    output.push('\n');
    output.push_str("Statement:\n");
    output.push_str(&record.body);
    if !record.body.ends_with('\n') {
        output.push('\n');
    }

    render_evidence(output, record);
    render_fields(output, record);

    output.push('\n');
    writeln!(
        output,
        "Source: {}:{}:{}",
        record.source.path, record.source.line, record.source.column
    )
    .expect("writing to String cannot fail");

    render_relations(output, &record.relations);
}

fn render_evidence(output: &mut String, record: &RetrievalRecord) {
    let evidence_fields = ["source", "test", "reviewed_by"];
    if record.evidence.is_empty() {
        return;
    }

    output.push('\n');
    output.push_str("Evidence:\n");
    for field in evidence_fields {
        if let Some(value) = record.evidence.get(field) {
            writeln!(output, "- {field}: {value}").expect("writing to String cannot fail");
        }
    }
    for (field, value) in &record.evidence {
        if !evidence_fields.contains(&field.as_str()) {
            writeln!(output, "- {field}: {value}").expect("writing to String cannot fail");
        }
    }
}

fn render_fields(output: &mut String, record: &RetrievalRecord) {
    if record.fields.is_empty() {
        return;
    }

    output.push('\n');
    output.push_str("Fields:\n");
    for (field, value) in &record.fields {
        writeln!(output, "- {field}: {value}").expect("writing to String cannot fail");
    }
}

fn render_relations(output: &mut String, relations: &AgentJsonRelations) {
    if relations.depends_on.is_empty()
        && relations.supersedes.is_empty()
        && relations.related_to.is_empty()
    {
        return;
    }

    output.push('\n');
    output.push_str("Relations:\n");
    if !relations.depends_on.is_empty() {
        writeln!(output, "- depends_on: {}", relations.depends_on.join(", "))
            .expect("writing to String cannot fail");
    }
    if !relations.supersedes.is_empty() {
        writeln!(output, "- supersedes: {}", relations.supersedes.join(", "))
            .expect("writing to String cannot fail");
    }
    if !relations.related_to.is_empty() {
        writeln!(output, "- related_to: {}", relations.related_to.join(", "))
            .expect("writing to String cannot fail");
    }
}
