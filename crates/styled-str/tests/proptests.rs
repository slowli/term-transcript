//! Property testing for styled strings.

use std::{fmt::Write as _, num::NonZeroUsize, ops};

use anstyle::{Ansi256Color, AnsiColor, Color, Effects, RgbColor, Style};
use proptest::{num, prelude::*};
use styled_str::{StyledStr, StyledString};

fn effects() -> impl Strategy<Value = Effects> {
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

fn color() -> impl Strategy<Value = Color> {
    prop_oneof![
        ansi_color().prop_map(Color::Ansi),
        num::u8::ANY.prop_map(|idx| Ansi256Color(idx).into()),
        (num::u8::ANY, num::u8::ANY, num::u8::ANY)
            .prop_map(|(r, g, b)| { RgbColor(r, g, b).into() }),
    ]
}

fn style() -> impl Strategy<Value = Style> {
    let effects_and_color = (
        effects(),
        proptest::option::of(color()),
        proptest::option::of(color()),
    );
    effects_and_color
        .prop_map(|(effects, fg, bg)| Style::new().effects(effects).fg_color(fg).bg_color(bg))
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
        .prop_flat_map(|lengths| {
            (
                proptest::collection::vec(style(), lengths.len()),
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
    span_count: ops::RangeInclusive<usize>,
) -> impl Strategy<Value = StyledString> {
    text.prop_flat_map(move |text| (span_lengths(text.clone(), span_count.clone()), Just(text)))
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

const VISIBLE_ASCII: &str = r"[\n\t\x20-\x7e]{1,32}";
const ANY_CHARS: &str = r"[^\x1b\r]{1,32}";

proptest! {
    #[test]
    fn styled_ascii_string_roundtrip(styled in styled_string(VISIBLE_ASCII, 1..=5)) {
        let rich_str = styled.to_string();
        let parsed: StyledString = rich_str.parse()?;
        prop_assert_eq!(styled, parsed);
    }

    #[test]
    fn styled_ascii_string_roundtrip_via_ansi(styled in styled_string(VISIBLE_ASCII, 1..=5)) {
        let ansi_str = styled.as_str().ansi().to_string();
        let parsed: StyledString = StyledString::from_ansi(&ansi_str)?;
        prop_assert_eq!(styled, parsed);
    }

    #[test]
    fn styled_string_roundtrip(styled in styled_string(ANY_CHARS, 1..=5)) {
        let rich_str = styled.to_string();
        let parsed: StyledString = rich_str.parse()?;
        prop_assert_eq!(styled, parsed);
    }

    #[test]
    fn styled_string_roundtrip_via_ansi(styled in styled_string(ANY_CHARS, 1..=5)) {
        let ansi_str = styled.as_str().ansi().to_string();
        let parsed: StyledString = StyledString::from_ansi(&ansi_str)?;
        prop_assert_eq!(styled, parsed);
    }

    #[test]
    fn styles_are_optimized(styled in styled_string(r"[\n\t\x20-\x7e]{32}", 2..=5)) {
        assert_spans_iterator(styled.as_str())?;
    }

    #[test]
    fn concatenating_styled_strings(
        start in styled_string(r"[^\x1b\r]{32}", 1..=5),
        end in styled_string(r"[^\x1b\r]{32}", 1..=5),
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
        (pos, styled) in styled_string(r"[\x20-\x7e]{32}", 1..=5).prop_flat_map(|string| {
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
    fn lines_in_styled_string(styled in styled_string(VISIBLE_ASCII, 1..=5)) {
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
    fn looking_up_spans(styled in styled_string(VISIBLE_ASCII, 1..=5)) {
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
}
