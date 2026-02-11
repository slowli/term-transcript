//! Processing `StyledString`s to convert them to the data model used by Handlebars.

use styled_str::StyledStr;
use unicode_width::UnicodeWidthChar;

use super::data::{LineBreak, SerdeStyledSpan, StyledLine};

pub(super) fn split_into_lines(
    str: StyledStr<'_>,
    max_width: Option<usize>,
) -> Vec<StyledLine<'_>> {
    let max_width = max_width.unwrap_or(usize::MAX);
    str.lines()
        .flat_map(|mut line| {
            let text = line.text();
            let mut line_start = 0;
            let mut split_lines = Vec::with_capacity(1);
            let mut current_width = 0;

            if max_width < usize::MAX {
                for (pos, ch) in text.char_indices() {
                    let ch_width = ch.width().unwrap_or(0);
                    if current_width + ch_width > max_width {
                        let head;
                        (head, line) = line.split_at(pos - line_start);
                        split_lines.push(StyledLine {
                            spans: map_spans(head),
                            br: Some(LineBreak::Hard),
                        });
                        current_width = ch_width;
                        line_start = pos;
                    } else {
                        current_width += ch_width;
                    }
                }
            }

            split_lines.push(StyledLine {
                spans: map_spans(line),
                br: None,
            });
            split_lines
        })
        .collect()
}

fn map_spans(line: StyledStr<'_>) -> Vec<SerdeStyledSpan<'_>> {
    line.spans()
        .map(|span| SerdeStyledSpan {
            text: span.text,
            style: span.style.into(),
        })
        .collect()
}
