//! Property testing for styled strings.

use std::{cell::Cell, fmt::Write as _, num::NonZeroUsize, ops};

use anstyle::{Ansi256Color, AnsiColor, Color, Effects, RgbColor, Style};
use proptest::{num, prelude::*};
use styled_str::{StyledStr, StyledString};

fn effects() -> impl Strategy<Value = Effects> + Clone {
    proptest::bits::u8::between(0, 6).prop_map(|val| {
        let mut this = Effects::new();
        if val & 1 != 0 {
            this = this.insert(Effects::BOLD);
        }
        if val & 2 != 0 {
            this = this.insert(Effects::ITALIC);
        }
        if val & 4 != 0 {
            this = this.insert(Effects::UNDERLINE);
        }
        if val & 8 != 0 {
            this = this.insert(Effects::DIMMED);
        }
        if val & 16 != 0 {
            this = this.insert(Effects::BLINK);
        }
        if val & 32 != 0 {
            this = this.insert(Effects::STRIKETHROUGH);
        }
        if val & 64 != 0 {
            this = this.insert(Effects::HIDDEN);
        }
        this
    })
}

fn ansi_color() -> impl Strategy<Value = AnsiColor> {
    prop_oneof![
        Just(AnsiColor::Black),
        Just(AnsiColor::Red),
        Just(AnsiColor::Green),
        Just(AnsiColor::Yellow),
        Just(AnsiColor::Blue),
        Just(AnsiColor::Magenta),
        Just(AnsiColor::Cyan),
        Just(AnsiColor::White),
        Just(AnsiColor::BrightBlack),
        Just(AnsiColor::BrightRed),
        Just(AnsiColor::BrightGreen),
        Just(AnsiColor::BrightYellow),
        Just(AnsiColor::BrightBlue),
        Just(AnsiColor::BrightMagenta),
        Just(AnsiColor::BrightCyan),
        Just(AnsiColor::BrightWhite),
    ]
}

fn color() -> impl Strategy<Value = Color> + Clone {
    prop_oneof![
        ansi_color().prop_map(Color::Ansi),
        num::u8::ANY.prop_map(|idx| Ansi256Color(idx).into()),
        (num::u8::ANY, num::u8::ANY, num::u8::ANY)
            .prop_map(|(r, g, b)| { RgbColor(r, g, b).into() }),
    ]
}

fn any_style() -> impl Strategy<Value = Style> + Clone {
    let effects_and_color = (
        effects(),
        proptest::option::of(color()),
        proptest::option::of(color()),
    );
    effects_and_color
        .prop_map(|(effects, fg, bg)| Style::new().effects(effects).fg_color(fg).bg_color(bg))
}

fn limited_style() -> impl Strategy<Value = Style> + Clone {
    prop_oneof![Just(Style::new()), Just(Style::new().bold()),]
}

const UTF8_CONTINUATION_MASK: u8 = 0b1100_0000;
const UTF8_CONTINUATION_MARKER: u8 = 0b1000_0000;

fn ceil_char_boundary(bytes: &[u8], mut pos: usize) -> usize {
    if pos > bytes.len() {
        return bytes.len();
    }

    while pos < bytes.len() && bytes[pos] & UTF8_CONTINUATION_MASK == UTF8_CONTINUATION_MARKER {
        pos += 1;
    }
    pos
}

#[derive(Debug)]
struct StyleAndLen {
    style: Style,
    len: NonZeroUsize,
}

fn span_lengths(
    text: String,
    style: impl Strategy<Value = Style> + Clone + 'static,
    span_count: ops::RangeInclusive<usize>,
) -> impl Strategy<Value = Vec<StyleAndLen>> {
    assert!(!span_count.is_empty());
    assert!(*span_count.start() > 0);
    assert!(!text.is_empty());

    let item = (1..=text.len()).prop_map(|len| NonZeroUsize::new(len).unwrap());
    let lengths = proptest::collection::vec(item, span_count).prop_map(move |mut lengths| {
        let mut pos = 0;
        for (i, len) in lengths.iter_mut().enumerate() {
            let prev_pos = pos;
            pos = ceil_char_boundary(text.as_bytes(), pos + len.get());
            *len = NonZeroUsize::new(pos - prev_pos).unwrap();

            if pos >= text.len() {
                lengths.truncate(i + 1);
                break;
            }
        }
        if pos < text.len() {
            lengths.push(NonZeroUsize::new(text.len() - pos).unwrap());
        }
        lengths
    });

    lengths
        .prop_flat_map(move |lengths| {
            (
                proptest::collection::vec(style.clone(), lengths.len()),
                Just(lengths),
            )
        })
        .prop_map(|(styles, lengths)| {
            styles
                .into_iter()
                .zip(lengths)
                .map(|(style, len)| StyleAndLen { style, len })
                .collect()
        })
}

fn styled_string(
    text: impl Strategy<Value = String>,
    style: impl Strategy<Value = Style> + Clone + 'static,
    span_count: ops::RangeInclusive<usize>,
) -> impl Strategy<Value = StyledString> {
    text.prop_flat_map(move |text| {
        (
            span_lengths(text.clone(), style.clone(), span_count.clone()),
            Just(text),
        )
    })
    .prop_map(|(spans, text)| {
        let mut builder = StyledString::builder();
        let mut pos = 0;
        for span in spans {
            builder.push_style(span.style);
            let end_pos = pos + span.len.get();
            builder.push_text(&text[pos..end_pos]);
            pos = end_pos;
        }
        builder.build()
    })
}

fn assert_spans_iterator(styled: StyledStr<'_>) -> Result<(), TestCaseError> {
    let mut prev_style = None;
    let mut text = String::new();
    for span_str in styled.spans() {
        if let Some(prev_style) = prev_style {
            prop_assert_ne!(prev_style, span_str.style);
        }
        prev_style = Some(span_str.style);
        text += span_str.text;
    }
    prop_assert_eq!(text, styled.text());
    Ok(())
}

fn test_string_slice(styled: StyledStr<'_>, range: ops::Range<usize>) -> Result<(), TestCaseError> {
    let slice = styled.get(range.clone());
    let text_slice = styled.text().get(range.clone());
    if let Some(text_slice) = text_slice {
        prop_assert!(slice.is_some());
        let slice = slice.unwrap();
        prop_assert_eq!(slice.text(), text_slice);

        let pos = styled.find(slice).unwrap();
        prop_assert!(pos <= range.start);
        if pos < range.start {
            let found_slice = styled.get(pos..pos + slice.text().len()).unwrap();
            prop_assert_eq!(found_slice, slice);
        }

        let before = styled.get(..range.start).unwrap();
        let after = styled.get(range.end..).unwrap();
        let concat: StyledString = [before, slice, after].into_iter().collect();
        prop_assert_eq!(concat, styled);
    } else {
        prop_assert!(slice.is_none());
    }
    Ok(())
}

const VISIBLE_ASCII: &str = r"[\n\t\x20-\x7e]{1,32}";
const ANY_CHARS: &str = r"[^\x1b\r]{1,32}";

proptest! {
    #[test]
    fn styled_ascii_string_roundtrip(styled in styled_string(VISIBLE_ASCII, any_style(), 1..=5)) {
        let rich_str = styled.to_string();
        let parsed: StyledString = rich_str.parse()?;
        prop_assert_eq!(styled, parsed);
    }

    #[test]
    fn styled_ascii_string_roundtrip_via_ansi(styled in styled_string(VISIBLE_ASCII, any_style(), 1..=5)) {
        let ansi_str = styled.as_str().ansi().to_string();
        let parsed: StyledString = StyledString::from_ansi(&ansi_str)?;
        prop_assert_eq!(styled, parsed);
    }

    #[test]
    fn styled_string_roundtrip(styled in styled_string(ANY_CHARS, any_style(), 1..=5)) {
        let rich_str = styled.to_string();
        let parsed: StyledString = rich_str.parse()?;
        prop_assert_eq!(styled, parsed);
    }

    #[test]
    fn styled_string_roundtrip_via_ansi(styled in styled_string(ANY_CHARS, any_style(), 1..=5)) {
        let ansi_str = styled.as_str().ansi().to_string();
        let parsed: StyledString = StyledString::from_ansi(&ansi_str)?;
        prop_assert_eq!(styled, parsed);
    }

    #[test]
    fn styles_are_optimized(styled in styled_string(r"[\n\t\x20-\x7e]{32}", any_style(), 2..=5)) {
        assert_spans_iterator(styled.as_str())?;
    }

    #[test]
    fn concatenating_styled_strings(
        start in styled_string(r"[^\x1b\r]{32}", any_style(), 1..=5),
        end in styled_string(r"[^\x1b\r]{32}", any_style(), 1..=5),
    ) {
        let mut concat = start.clone();
        concat.push_str(end.as_str());

        prop_assert_eq!(concat.text().len(), start.text().len() + end.text().len());
        prop_assert!(concat.text().starts_with(start.text()));
        prop_assert!(concat.text().ends_with(end.text()));

        let concat_ansi = format!("{}{}", start.as_str().ansi(), end.as_str().ansi());
        let concat_ansi = StyledString::from_ansi(&concat_ansi)?;
        prop_assert_eq!(concat_ansi, concat);
    }

    #[test]
    fn splitting_styled_string(
        (pos, styled) in styled_string(r"[\x20-\x7e]{32}", any_style(), 1..=5).prop_flat_map(|string| {
            (0..=string.text().len(), Just(string))
        })
    ) {
        let (start, end) = styled.as_str().split_at(pos);
        prop_assert_eq!(styled.text().len(), start.text().len() + end.text().len());
        prop_assert!(styled.text().starts_with(start.text()));
        prop_assert!(styled.text().ends_with(end.text()));

        assert_spans_iterator(start)?;
        assert_spans_iterator(end)?;
    }

    #[test]
    fn lines_in_styled_string(styled in styled_string(VISIBLE_ASCII, any_style(), 1..=5)) {
        let mut builder = StyledString::builder();
        let mut ansi_str = String::new();
        for line in styled.as_str().lines() {
            builder.push_str(line);
            builder.push_text("\n");
            writeln!(&mut ansi_str, "{}", line.ansi()).unwrap();
        }

        let mut recovered = builder.build();
        if !styled.text().ends_with('\n') {
            prop_assert_eq!(ansi_str.pop(), Some('\n'));
            prop_assert_eq!(recovered.pop().map(|(ch, _)| ch), Some('\n'));
        }
        let recovered_ansi = StyledString::from_ansi(&ansi_str)?;
        // The recovered string may differ in newline styling, but this must not matter when creating a diff
        recovered_ansi.as_str().diff(styled.as_str())?;
        recovered.as_str().diff(styled.as_str())?;
    }

    #[test]
    fn looking_up_spans(styled in styled_string(VISIBLE_ASCII, any_style(), 1..=5)) {
        let styled = styled.as_str();
        let mut spans_iter = styled.spans().peekable();
        let mut span_start = 0;
        for pos in 0..styled.text().len() {
            let expected = spans_iter.peek().unwrap();
            let span = styled.span_at(pos).unwrap();
            assert_eq!(span, *expected);

            if span_start + expected.text.len() == pos + 1 {
                spans_iter.next();
                span_start = pos + 1;
            }
        }
    }

    #[test]
    fn slicing_string(
        (styled, start, end) in styled_string(VISIBLE_ASCII, any_style(), 1..=5).prop_flat_map(|string| {
            let text_len = string.text().len();
            (Just(string), 0..=text_len, 0..=text_len)
        })
    ) {
        test_string_slice(styled.as_str(), start..end)?;
    }

    #[test]
    fn starts_with_positive(styled in styled_string(VISIBLE_ASCII, any_style(), 1..=5)) {
        let styled = styled.as_str();
        for end in 0..=styled.text().len() {
            let prefix = styled.get(..end).unwrap();
            prop_assert!(styled.starts_with(prefix), "end={end}");
            if end < styled.text().len() {
                prop_assert!(!prefix.starts_with(styled), "end={end}");
            }
        }
    }

    #[test]
    fn ends_with_positive(styled in styled_string(VISIBLE_ASCII, any_style(), 1..=5)) {
        let styled = styled.as_str();
        for start in 0..=styled.text().len() {
            let suffix = styled.get(start..).unwrap();
            prop_assert!(styled.ends_with(suffix), "start={start}");
            if start > 0 {
                prop_assert!(!suffix.ends_with(styled), "start={start}");
            }
        }
    }
}

fn test_starts_with_random(
    haystack: StyledStr<'_>,
    needle: StyledStr<'_>,
) -> Result<bool, TestCaseError> {
    if haystack.starts_with(needle) {
        prop_assert!(haystack.text().starts_with(needle.text()));
        let end = needle.text().len();
        prop_assert_eq!(haystack.get(..end).unwrap(), needle);

        for (pos, _) in haystack.text().char_indices().rev() {
            if pos < end {
                break;
            }
            let haystack_prefix = haystack.get(..pos).unwrap();
            prop_assert!(haystack_prefix.starts_with(needle));
        }
        Ok(true)
    } else {
        prop_assert!(
            haystack
                .get(..needle.text().len())
                .is_none_or(|prefix| prefix != needle)
        );
        Ok(false)
    }
}

#[test]
fn starts_with_random() {
    let positive_count = Cell::new(0);

    proptest!(|(
        // Restrict the alphabet so that there's a non-zero chance to hit the positive case
        haystack in styled_string("[aл]{32}", limited_style(), 1..=5),
        needle in styled_string("[aл]{3}", limited_style(), 1..=3),
    )| {
        if test_starts_with_random(haystack.as_str(), needle.as_str())? {
            positive_count.update(|count| count + 1);
        }
    });

    println!("Positive count: {}", positive_count.get());
}

fn test_ends_with_random(
    haystack: StyledStr<'_>,
    needle: StyledStr<'_>,
) -> Result<bool, TestCaseError> {
    let possible_start = haystack.text().len().checked_sub(needle.text().len());
    if haystack.ends_with(needle) {
        prop_assert!(haystack.text().ends_with(needle.text()));
        let start = possible_start.unwrap();
        prop_assert_eq!(haystack.get(start..).unwrap(), needle);

        for (pos, _) in haystack.text().char_indices() {
            if pos > start {
                break;
            }
            let haystack_suffix = haystack.get(pos..).unwrap();
            prop_assert!(haystack_suffix.ends_with(needle));
        }
        Ok(true)
    } else if let Some(possible_start) = possible_start {
        prop_assert!(
            haystack
                .get(possible_start..)
                .is_none_or(|prefix| prefix != needle)
        );
        Ok(false)
    } else {
        Ok(false)
    }
}

#[test]
fn ends_with_random() {
    let positive_count = Cell::new(0);

    proptest!(|(
        // Restrict the alphabet so that there's a non-zero chance to hit the positive case
        haystack in styled_string("[aл]{32}", limited_style(), 1..=5),
        needle in styled_string("[aл]{3}", limited_style(), 1..=3),
    )| {
        if test_ends_with_random(haystack.as_str(), needle.as_str())? {
            positive_count.update(|count| count + 1);
        }
    });

    println!("Positive count: {}", positive_count.get());
}

fn test_find_random(haystack: StyledStr<'_>, needle: StyledStr<'_>) -> Result<bool, TestCaseError> {
    if let Some(pos) = haystack.find(needle) {
        prop_assert_eq!(
            haystack.get(pos..pos + needle.text().len()).unwrap(),
            needle
        );

        for prev_pos in 0..pos {
            if let Some(substr) = haystack.get(prev_pos..prev_pos + needle.text().len()) {
                prop_assert_ne!(substr, needle);
            }
            if let Some(suffix) = haystack.get(prev_pos..) {
                prop_assert_eq!(suffix.find(needle), Some(pos - prev_pos));
            }
        }
        Ok(true)
    } else {
        for pos in 0..haystack.text().len() {
            if let Some(substr) = haystack.get(pos..pos + needle.text().len()) {
                prop_assert_ne!(substr, needle);
            }
        }
        Ok(false)
    }
}

#[test]
fn find_random() {
    let positive_count = Cell::new(0);

    proptest!(|(
        // Restrict the alphabet so that there's a non-zero chance to hit the positive case
        haystack in styled_string("[aл]{32}", limited_style(), 1..=5),
        needle in styled_string("[aл]{3}", limited_style(), 1..=3),
    )| {
        if test_find_random(haystack.as_str(), needle.as_str())? {
            positive_count.update(|count| count + 1);
        }
    });

    println!("Positive count: {}", positive_count.get());
}
