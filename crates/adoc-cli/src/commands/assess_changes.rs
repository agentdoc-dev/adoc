use std::fmt::Write as _;

use adoc_core::{ChangeAssessmentEnvelope, PathClassification};
use adoc_local::{AssessmentInput, LocalContext, UnrestrictedPathPolicy};

use crate::error::CliError;
use crate::presentation::{ResolvedFormat, json as json_presentation};

use super::{current_dir, report};

pub(crate) struct AssessChangesCommandInput {
    pub(crate) base_ref: String,
    pub(crate) head_ref: Option<String>,
    pub(crate) as_of: Option<chrono::NaiveDate>,
}

pub(crate) fn assess_changes(input: AssessChangesCommandInput, resolved: ResolvedFormat) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };
    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match context.assess_changes(AssessmentInput {
        base_ref: input.base_ref,
        head_ref: input.head_ref,
        as_of: input.as_of,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    for diagnostic in &outcome.envelope.diagnostics {
        eprintln!(
            "{}[{}] {}",
            diagnostic.severity, diagnostic.code, diagnostic.message
        );
    }
    match resolved {
        ResolvedFormat::Json => {
            json_presentation::write_json(&outcome.envelope, &mut std::io::stdout()).map_or_else(
                |source| report(CliError::StdoutIo { source }),
                |()| outcome.exit_code,
            )
        }
        ResolvedFormat::Plain | ResolvedFormat::Styled => {
            print!("{}", render_text(&outcome.envelope, false));
            outcome.exit_code
        }
        ResolvedFormat::Markdown => {
            print!("{}", render_text(&outcome.envelope, true));
            outcome.exit_code
        }
    }
}

fn render_text(envelope: &ChangeAssessmentEnvelope, markdown: bool) -> String {
    let mut output = String::new();
    let prefix = if markdown { "- " } else { "" };
    writeln!(
        output,
        "{prefix}Assessment: {:?} / {:?}",
        envelope.completeness, envelope.outcome
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "{prefix}Evaluation date: {}",
        envelope.evaluation_date
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "{prefix}Paths: {} changed, {} covered, {} provisional, {} uncovered, {} excluded",
        envelope.summary.changed_paths,
        envelope.summary.covered,
        envelope.summary.provisional,
        envelope.summary.uncovered,
        envelope.summary.excluded
    )
    .expect("writing to String cannot fail");
    if let Some(paths) = &envelope.paths.value {
        for path in paths {
            let marker = match path.classification {
                PathClassification::Covered => "covered",
                PathClassification::Provisional => "provisional",
                PathClassification::Uncovered => "uncovered",
                PathClassification::Excluded => "excluded",
            };
            writeln!(output, "{prefix}{marker}: {}", path.path)
                .expect("writing to String cannot fail");
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::render_text;
    use adoc_core::{
        AssessmentCompleteness, AssessmentOutcome, ChangeAssessmentInput, assess_changes_from_git,
    };
    use chrono::NaiveDate;
    use std::path::PathBuf;

    #[test]
    fn markdown_presenter_is_heading_free_even_for_error_envelopes() {
        let envelope = assess_changes_from_git(ChangeAssessmentInput {
            project_root: PathBuf::from("/definitely/not/a/repository"),
            base_ref: "main".to_string(),
            head_ref: None,
            evaluation_date: NaiveDate::from_ymd_opt(2026, 7, 22).expect("date"),
        });
        let output = render_text(&envelope, true);
        assert!(!output.lines().any(|line| line.starts_with('#')));
        assert_eq!(envelope.completeness, AssessmentCompleteness::Error);
        assert_eq!(envelope.outcome, AssessmentOutcome::NotEvaluated);
    }
}
