use std::path::{Path, PathBuf};

use crate::diagnostic::{SourcePosition, SourceSpan};

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub identity_path: PathBuf,
    pub text: String,
    pub line_index: LineIndex,
}

impl SourceFile {
    pub fn new_with_identity_path(path: PathBuf, text: String, identity_path: PathBuf) -> Self {
        let line_index = LineIndex::new(&text);
        Self {
            path,
            identity_path,
            text,
            line_index,
        }
    }

    pub fn span_for_line(&self, line_number: u32, text: &str) -> SourceSpan {
        let start = self.line_index.position_for_line(line_number);
        let end = SourcePosition {
            line: line_number,
            column: start.column + text.len() as u32,
            offset: start.offset + text.len() as u32,
        };
        SourceSpan {
            file: self.path.clone(),
            start,
            end,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset + 1);
            }
        }
        Self { line_starts }
    }

    pub fn position_for_line(&self, line_number: u32) -> SourcePosition {
        let line_index = line_number.saturating_sub(1) as usize;
        let offset = self.line_starts.get(line_index).copied().unwrap_or(0);
        SourcePosition {
            line: line_number,
            column: 1,
            offset: offset as u32,
        }
    }
}

pub fn derive_page_id(path: &Path) -> String {
    let path_segments: Vec<_> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();
    let last_index = path_segments.len().saturating_sub(1);

    let id_segments: Vec<_> = path_segments
        .iter()
        .enumerate()
        .filter_map(|(index, segment)| {
            let value = if index == last_index {
                Path::new(segment)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or(*segment)
            } else {
                segment
            };
            let normalized = normalize_id_segment(value);
            (!normalized.is_empty()).then_some(normalized)
        })
        .collect();

    if id_segments.is_empty() {
        "untitled".to_string()
    } else {
        id_segments.join(".")
    }
}

fn normalize_id_segment(value: &str) -> String {
    let mut id = String::new();
    let mut previous_was_dash = false;

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            id.push(character.to_ascii_lowercase());
            previous_was_dash = false;
        } else if !previous_was_dash {
            id.push('-');
            previous_was_dash = true;
        }
    }

    id.trim_matches('-').to_string()
}
