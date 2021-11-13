use super::*;

use assert_matches::assert_matches;
use quick_xml::events::{BytesEnd, BytesStart, BytesText};

use std::io::{Cursor, Read};

const SVG: &[u8] = br#"
        <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg">
          <foreignObject x="0" y="0" width="652" height="344">
            <div xmlns="http://www.w3.org/1999/xhtml" class="container">
              <div class="user-input"><pre><span class="prompt">$</span> ls -al --color=always</pre></div>
              <div class="term-output"><pre>total 28
drwxr-xr-x 1 alex alex 4096 Apr 18 12:54 <span class="fg4">.</span>
drwxrwxrwx 1 alex alex 4096 Apr 18 12:38 <span class="fg4 bg2">..</span>
-rw-r--r-- 1 alex alex 8199 Apr 18 12:48 Cargo.lock</pre>
              </div>
            </div>
          </foreignObject>
        </svg>
    "#;

#[test]
fn reading_file() {
    let transcript = Transcript::from_svg(SVG).unwrap();
    assert_eq!(transcript.interactions.len(), 1);

    let interaction = &transcript.interactions[0];
    assert_matches!(
        &interaction.input,
        UserInput { text, prompt }
            if text == "ls -al --color=always" && prompt.as_deref() == Some("$")
    );

    let plaintext = &interaction.output.plaintext;
    assert!(plaintext.starts_with("total 28\ndrwxr-xr-x"));
    assert!(plaintext.contains("4096 Apr 18 12:54 .\n"));
    assert!(!plaintext.contains(r#"<span class="fg4">.</span>"#));

    let html = &interaction.output.html;
    assert!(html.starts_with("total 28\ndrwxr-xr-x"));
    assert!(html.contains(r#"Apr 18 12:54 <span class="fg4">.</span>"#));

    let ansi_text = &interaction.output.ansi_text;
    assert!(ansi_text.starts_with("total 28\ndrwxr-xr-x"));
    assert!(ansi_text.contains("Apr 18 12:54 \u{1b}[0m\u{1b}[34m.\u{1b}[0m"));
}

#[test]
fn reading_file_with_extra_info() {
    let mut data = SVG.to_owned();
    data.extend_from_slice(b"<other>data</other>");
    let mut data = Cursor::new(data.as_slice());

    let transcript = Transcript::from_svg(&mut data).unwrap();
    assert_eq!(transcript.interactions.len(), 1);

    // Check that the parser stops after `</svg>`.
    let mut end = String::new();
    data.read_to_string(&mut end).unwrap();
    assert_eq!(end.trim_start(), "<other>data</other>");
}

#[test]
fn reading_file_with_no_output() {
    const SVG: &[u8] = br#"
            <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg">
              <foreignObject x="0" y="0" width="652" height="344">
                <div xmlns="http://www.w3.org/1999/xhtml" class="container">
                  <div class="user-input"><pre><span class="prompt">$</span> ls &gt; /dev/null</pre></div>
                  <div class="user-input"><pre><span class="prompt">$</span> ls</pre></div>
                  <div class="term-output"><pre>total 28
    drwxr-xr-x 1 alex alex 4096 Apr 18 12:54 <span class="fg-blue">.</span>
    drwxrwxrwx 1 alex alex 4096 Apr 18 12:38 <span class="fg-blue bg-green">..</span>
    -rw-r--r-- 1 alex alex 8199 Apr 18 12:48 Cargo.lock</pre>
                  </div>
                </div>
              </foreignObject>
            </svg>
        "#;

    let transcript = Transcript::from_svg(SVG).unwrap();
    assert_eq!(transcript.interactions.len(), 2);

    assert_eq!(transcript.interactions[0].input.text, "ls > /dev/null");
    assert!(transcript.interactions[0].output.plaintext.is_empty());
    assert!(transcript.interactions[0].output.html.is_empty());

    assert_eq!(transcript.interactions[1].input.text, "ls");
    assert!(!transcript.interactions[1].output.plaintext.is_empty());
    assert!(!transcript.interactions[1].output.html.is_empty());
}

#[test]
fn reading_file_without_svg_tag() {
    let data: &[u8] = b"<div>Text</div>";
    let err = Transcript::from_svg(data).unwrap_err();

    assert_matches!(err, ParseError::UnexpectedRoot(tag) if tag == "div");
}

#[test]
fn reading_file_without_container() {
    let bogus_data: &[u8] = br#"
            <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg" version="1.1">
              <style>.container { color: #eee; }</style>
            </svg>
        "#;
    let err = Transcript::from_svg(bogus_data).unwrap_err();

    assert_matches!(err, ParseError::UnexpectedEof);
}

#[test]
fn reading_file_with_invalid_container() {
    const INVALID_ATTRS: &[&str] = &[
        "",
        // no class
        r#"xmlns="http://www.w3.org/1999/xhtml""#,
        // no namespace
        r#"class="container""#,
        // invalid namespace
        r#"xmlns="http://www.w3.org/2000/svg" class="container""#,
        // invalid class
        r#"xmlns="http://www.w3.org/1999/xhtml" class="cont""#,
    ];

    for &attrs in INVALID_ATTRS {
        let bogus_data = format!(
            r#"
                <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg" version="1.1">
                  <foreignObject x="0" y="0" width="652" height="344">
                    <div {}>Test</div>
                  </foreignObject>
                </svg>
                "#,
            attrs
        );
        let err = Transcript::from_svg(bogus_data.as_bytes()).unwrap_err();

        assert_matches!(err, ParseError::InvalidContainer);
    }
}

#[test]
fn reading_file_without_term_output() {
    let bogus_data: &[u8] = br#"
            <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg" version="1.1">
              <foreignObject x="0" y="0" width="652" height="344">
                <div xmlns="http://www.w3.org/1999/xhtml" class="container">
                  <div class="user-input"><pre>$ ls -al --color=always</pre></div>
                </div>
              </foreignObject>
            </svg>
        "#;
    let err = Transcript::from_svg(bogus_data).unwrap_err();

    assert_matches!(err, ParseError::UnexpectedEof);
}

#[test]
fn reading_user_input_with_manual_events() {
    let mut state = UserInputState::default();
    {
        let event = Event::Start(BytesStart::borrowed_name(b"pre"));
        assert!(state.process(event).unwrap().is_none());
        assert_eq!(state.prompt_open_tags, None);
        assert!(state.text.plaintext_buffer.is_empty());
    }
    {
        let event = Event::Start(BytesStart::borrowed(br#"span class="prompt""#, 4));
        assert!(state.process(event).unwrap().is_none());
        assert_eq!(state.prompt_open_tags, Some(2));
        assert!(state.text.plaintext_buffer.is_empty());
    }
    {
        let event = Event::Text(BytesText::from_escaped(b"$" as &[u8]));
        assert!(state.process(event).unwrap().is_none());
        assert_eq!(state.text.plaintext_buffer, "$");
    }
    {
        let event = Event::End(BytesEnd::borrowed(b"span"));
        assert!(state.process(event).unwrap().is_none());
        assert_eq!(state.prompt.as_deref(), Some("$"));
        assert!(state.text.plaintext_buffer.is_empty());
    }
    {
        let event = Event::Text(BytesText::from_escaped(b" rainbow" as &[u8]));
        assert!(state.process(event).unwrap().is_none());
    }

    let event = Event::End(BytesEnd::borrowed(b"pre"));
    assert!(state.process(event).unwrap().is_none());

    let event = Event::End(BytesEnd::borrowed(b"div"));
    let user_input = state.process(event).unwrap().unwrap();
    assert_eq!(user_input.prompt(), Some("$"));
    assert_eq!(user_input.text, "rainbow");
}

fn read_user_input(input: &[u8]) -> UserInput {
    let mut wrapped_input = Vec::with_capacity(input.len() + 11);
    wrapped_input.extend_from_slice(b"<div>");
    wrapped_input.extend_from_slice(input);
    wrapped_input.extend_from_slice(b"</div>");

    let mut reader = XmlReader::from_reader(wrapped_input.as_slice());
    let mut buffer = vec![];
    let mut state = UserInputState::default();

    // Skip the `<div>` start event.
    while !matches!(reader.read_event(&mut buffer).unwrap(), Event::Start(_)) {
        // Drop the event.
    }

    loop {
        let event = reader.read_event(&mut buffer).unwrap();
        if let Event::Eof = &event {
            panic!("Reached EOF without creating `UserInput`");
        }

        if let Some(user_input) = state.process(event).unwrap() {
            break user_input;
        }
    }
}

#[test]
fn reading_user_input_base_case() {
    let user_input = read_user_input(br#"<pre><span class="prompt">&gt;</span> echo foo</pre>"#);

    assert_eq!(user_input.prompt.as_deref(), Some(">"));
    assert_eq!(user_input.text, "echo foo");
}

#[test]
fn reading_user_input_without_wrapper() {
    let user_input = read_user_input(br#"<span class="prompt">&gt;</span> echo foo"#);

    assert_eq!(user_input.prompt.as_deref(), Some(">"));
    assert_eq!(user_input.text, "echo foo");
}

#[test]
fn reading_user_input_without_prompt() {
    let user_input = read_user_input(br#"<pre>echo <span class="bold">foo</span></pre>"#);

    assert_eq!(user_input.prompt.as_deref(), None);
    assert_eq!(user_input.text, "echo foo");
}

#[test]
fn reading_user_input_with_prompt_only() {
    let user_input = read_user_input(br#"<pre><span class="prompt">$</span></pre>"#);

    assert_eq!(user_input.prompt.as_deref(), Some("$"));
    assert_eq!(user_input.text, "");
}

#[test]
fn reading_user_input_with_bogus_prompt_location() {
    let user_input =
        read_user_input(br#"<pre>echo foo <span class="prompt">&gt;</span> output.log</pre>"#);

    assert_eq!(user_input.prompt.as_deref(), None);
    assert_eq!(user_input.text, "echo foo > output.log");
}

#[test]
fn reading_user_input_with_multiple_prompts() {
    let user_input = read_user_input(
        b"<pre><span class=\"prompt\">&gt;&gt;&gt;</span>  \
                    echo foo <span class=\"prompt\">&gt;</span> output.log</pre>",
    );

    assert_eq!(user_input.prompt.as_deref(), Some(">>>"));
    assert_eq!(user_input.text, "echo foo > output.log");
}

#[test]
fn newline_breaks_are_normalized() {
    let mut state = TextReadingState::default();
    let text = BytesText::from_escaped(b"some\ntext\r\nand more text" as &[u8]);
    state.process(Event::Text(text)).unwrap();
    assert_eq!(state.plaintext_buffer, "some\ntext\nand more text");
}

#[test]
fn parsing_color_index() {
    assert_eq!(
        TextReadingState::parse_indexed_color(b"0"),
        Some(Color::Black)
    );
    assert_eq!(
        TextReadingState::parse_indexed_color(b"3"),
        Some(Color::Yellow)
    );
    assert_eq!(
        TextReadingState::parse_indexed_color(b"9"),
        Some(Color::Ansi256(9))
    );
    assert_eq!(
        TextReadingState::parse_indexed_color(b"10"),
        Some(Color::Ansi256(10))
    );
    assert_eq!(
        TextReadingState::parse_indexed_color(b"15"),
        Some(Color::Ansi256(15))
    );

    assert_eq!(TextReadingState::parse_indexed_color(b""), None);
    assert_eq!(TextReadingState::parse_indexed_color(b"17"), None);
    assert_eq!(TextReadingState::parse_indexed_color(b"01"), None);
    assert_eq!(TextReadingState::parse_indexed_color(b"333"), None);
}

#[test]
fn parsing_color_from_classes() {
    let mut color_spec = ColorSpec::new();
    TextReadingState::parse_color_from_classes(&mut color_spec, b"bold fg3 underline bg11");

    assert!(color_spec.bold(), "{:?}", color_spec);
    assert!(color_spec.underline(), "{:?}", color_spec);
    assert_eq!(color_spec.fg(), Some(&Color::Ansi256(3)));
    assert_eq!(color_spec.bg(), Some(&Color::Ansi256(11)));
}

#[test]
fn parsing_color_from_style() {
    let mut color_spec = ColorSpec::new();
    TextReadingState::parse_color_from_style(&mut color_spec, b"color: #fed; background: #c0ffee")
        .unwrap();

    assert_eq!(color_spec.fg(), Some(&Color::Rgb(0xff, 0xee, 0xdd)));
    assert_eq!(color_spec.bg(), Some(&Color::Rgb(0xc0, 0xff, 0xee)));
}

#[test]
fn parsing_color_from_style_with_terminal_semicolon() {
    let mut color_spec = ColorSpec::new();
    TextReadingState::parse_color_from_style(&mut color_spec, b"color: #fed; background: #c0ffee;")
        .unwrap();

    assert_eq!(color_spec.fg(), Some(&Color::Rgb(0xff, 0xee, 0xdd)));
    assert_eq!(color_spec.bg(), Some(&Color::Rgb(0xc0, 0xff, 0xee)));
}
