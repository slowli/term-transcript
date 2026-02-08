//! Processing `StyledString`s to convert them to the data model used by Handlebars.

use std::mem;

use term_style::{SpansSlice, StyledStr};
use unicode_width::UnicodeWidthChar;

use super::data::{LineBreak, SerdeStyledSpan, StyledLine};

pub(super) fn split_into_lines(
    str: StyledStr<'_>,
    max_width: Option<usize>,
) -> Vec<StyledLine<'_>> {
    let max_width = max_width.unwrap_or(usize::MAX);
    str.lines()
        .flat_map(|line| {
            let mut split_lines = Vec::with_capacity(1);
            let mut current_width = 0;
            let mut line_start = 0;
            let (text, spans) = line.into_parts();
            let mut spans = SpansSlice::new(&spans);

            if max_width < usize::MAX {
                for (pos, ch) in text.char_indices() {
                    let ch_width = ch.width().unwrap_or(0);
                    if current_width + ch_width > max_width {
                        let tail = spans.split_off(pos - line_start);
                        let head = mem::replace(&mut spans, tail);
                        split_lines.push(StyledLine {
                            spans: map_spans(&text[line_start..pos], head),
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
                spans: map_spans(&text[line_start..], spans),
                br: None,
            });
            split_lines
        })
        .collect()
}

fn map_spans<'s>(text: &'s str, spans: SpansSlice<'_>) -> Vec<SerdeStyledSpan<'s>> {
    let mut pos = 0;
    spans
        .iter()
        .map(|span| {
            let serde_span = SerdeStyledSpan {
                text: &text[pos..pos + span.len],
                style: span.style.into(),
            };
            pos += span.len;
            serde_span
        })
        .collect()
}
