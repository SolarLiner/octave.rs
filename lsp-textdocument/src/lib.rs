use lsp_types::{Position, Range, TextDocumentContentChangeEvent, Url, TextEdit};
use thiserror::Error;
use std::ops::Deref;

#[derive(Copy, Clone, Debug, Error)]
pub enum TextDocumentMutationError {
    #[error("Overlapping edit")]
    OverlappingEdit
}

#[derive(Clone, Debug)]
pub struct TextDocument {
    uri: Url,
    language_id: String,
    version: u64,
    content: String,
    line_offsets: Vec<usize>,
}

impl Deref for TextDocument {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.content
    }
}

impl TextDocument {
    pub fn new<S: Into<String>>(uri: Url, language_id: S, version: u64, content: String) -> Self {
        Self {
            uri,
            language_id: language_id.into(),
            version,
            content,
            line_offsets: vec![],
        }
    }

    pub fn uri(&self) -> &Url {
        &self.uri
    }

    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn get_range(&self, range: Range) -> &str {
        let start = self.offset_at(range.start);
        let end = self.offset_at(range.end);
        &self.content[start..end]
    }

    pub fn update(&mut self, changes: Vec<TextDocumentContentChangeEvent>, version: Option<i64>) {
        for change in changes {
            if let Some(range) = change.range {
                let range = get_wellformed_range(range);
                let start = self.offset_at(range.start);
                let end = self.offset_at(range.end);
                self.content = format!(
                    "{}{}{}",
                    &self.content[0..start],
                    change.text,
                    &self.content[end..]
                );

                let start_line = range.start.line.max(0) as usize;
                let end_line = range.end.line.max(0) as usize;
                let added_offsets = compute_line_offsets(&change.text, false, start);
                let added_offsets_len = added_offsets.len();
                if end_line - start_line == added_offsets_len {
                    for (i, off) in added_offsets.into_iter().enumerate() {
                        self.line_offsets[i + start_line + 1] = off;
                    }
                } else {
                    self.line_offsets
                        .splice(start_line + 1..end_line - start_line, added_offsets);
                }
                let diff = change.text.len() - (end - start);
                if diff != 0 {
                    for i in (start_line + 1 + added_offsets_len)..self.line_offsets.len() {
                        self.line_offsets[i] += diff;
                    }
                }
            } else {
                self.line_offsets = compute_line_offsets(&change.text, true, 0);
                self.content = change.text;
            }
        }
        self.version = version.map(|v| v as u64).unwrap_or(0);
    }

    pub fn apply_edits(&mut self, edits: Vec<TextEdit>) -> Result<(), TextDocumentMutationError> {
        let mut edits = edits.into_iter().map(get_wellformed_edit).collect::<Vec<_>>();
        edits.sort_by_key(|v| v.range.start);
        let mut last_modified_off = 0;
        let mut spans = vec![];
        for e in edits {
            let start_off = self.offset_at(e.range.start);
            if start_off < last_modified_off {
                return Err(TextDocumentMutationError::OverlappingEdit);
            } else if start_off > last_modified_off {
                spans.push(self.content[last_modified_off..start_off].to_string());
            }
            if e.new_text.len() > 0 {
                spans.push(e.new_text);
            }
            last_modified_off = self.offset_at(e.range.end);
        }
        spans.push(self.content[last_modified_off..].to_string());
        return Ok(())
    }

    pub fn position_at(&self, mut offset: usize) -> Position {
        offset = offset.max(0).min(self.content.len());

        if self.line_offsets.len() == 0 {
            Position {
                line: 0,
                character: offset as u64,
            }
        } else {
            let mut low = 0;
            let mut high = self.line_offsets.len();
            while low < high {
                let mid = ((low as f32 + high as f32) / 2.0).floor() as usize;
                if self.line_offsets[mid] > offset {
                    high = mid;
                } else {
                    low = mid + 1;
                }
            }
            Position {
                line: (low - 1) as u64,
                character: (offset - self.line_offsets[low - 1]) as u64,
            }
        }
    }

    pub fn offset_at(&self, pos: Position) -> usize {
        if pos.line >= self.line_offsets.len() as u64 {
            self.content.len()
        } else {
            let line_off = self.line_offsets[pos.line as usize];
            let next_line_off = if pos.line + 1 < self.line_offsets.len() as u64 {
                self.line_offsets[pos.line as usize + 1]
            } else {
                self.content.len()
            };
            (line_off + pos.character as usize)
                .min(next_line_off)
                .max(line_off)
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_offsets.len()
    }
}

fn compute_line_offsets(s: &str, is_line_start: bool, start_offset: usize) -> Vec<usize> {
    let start = if is_line_start {
        vec![start_offset]
    } else {
        vec![]
    };
    start.into_iter().chain(s.match_indices('\n').map(|(i, _)| i)).collect()
}

fn get_wellformed_range(range: Range) -> Range {
    if range.start.line > range.end.line
        || (range.start.line == range.end.line) && (range.start.character > range.end.character)
    {
        Range {
            start: range.end,
            end: range.start,
        }
    } else {
        range
    }
}

fn get_wellformed_edit(edit: TextEdit) -> TextEdit {
    let range = get_wellformed_range(edit.range);
    if range != edit.range {
        TextEdit {
            range,
            new_text: edit.new_text
        }
    } else { edit }
}