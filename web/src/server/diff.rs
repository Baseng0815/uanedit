//! The would-be save against the bytes on disk, as line hunks.
//!
//! Computed here rather than in the browser because the server already holds both sides, and
//! shipping two multi-megabyte documents to diff them there would cost more than the answer.

use similar::{
    ChangeTag,
    TextDiff,
};

use crate::api::{
    DiffHunk,
    DiffLine,
    DiffLineKind,
    DiffPreview,
};

/// Lines of context around each change, as in a unified diff.
const CONTEXT: usize = 3;
/// Enough to read a real edit, few enough that a reformatted file does not become the payload.
const MAX_LINES: usize = 4_000;

pub fn preview(
    name: &str,
    before: &str,
    after: &str,
) -> DiffPreview {
    let mut preview = DiffPreview {
        name: name.to_owned(),
        changed: before != after,
        ..DiffPreview::default()
    };
    if !preview.changed {
        return preview;
    }

    let diff = TextDiff::from_lines(before, after);
    let mut lines = 0;
    for group in diff.grouped_ops(CONTEXT) {
        let mut hunk = DiffHunk {
            old_start: 0,
            new_start: 0,
            lines: Vec::new(),
        };
        for operation in &group {
            for change in diff.iter_changes(operation) {
                let line = DiffLine {
                    kind: match change.tag() {
                        ChangeTag::Equal => DiffLineKind::Context,
                        ChangeTag::Insert => DiffLineKind::Added,
                        ChangeTag::Delete => DiffLineKind::Removed,
                    },
                    old_line: change.old_index().map(|index| index + 1),
                    new_line: change.new_index().map(|index| index + 1),
                    text: change.value().trim_end_matches(['\r', '\n']).to_owned(),
                };
                match line.kind {
                    DiffLineKind::Added => preview.added += 1,
                    DiffLineKind::Removed => preview.removed += 1,
                    DiffLineKind::Context => {}
                }
                if hunk.lines.is_empty() {
                    hunk.old_start = line.old_line.unwrap_or_default();
                    hunk.new_start = line.new_line.unwrap_or_default();
                }
                hunk.lines.push(line);
                lines += 1;
            }
        }
        preview.hunks.push(hunk);
        if lines >= MAX_LINES {
            preview.truncated = true;
            break;
        }
    }
    preview
}
