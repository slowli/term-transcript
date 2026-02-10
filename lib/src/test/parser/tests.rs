use std::io::{Cursor, Read};

use assert_matches::assert_matches;
use quick_xml::events::{BytesEnd, BytesStart, BytesText};
use test_casing::test_casing;

use super::*;
use crate::ExitStatus;

const SVG: &[u8] = br#"
    <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg">
      <foreignObject x="0" y="0" width="652" height="344">
        <div xmlns="http://www.w3.org/1999/xhtml" class="container">
          <div class="input"><pre><span class="prompt">$</span> ls -al --color=always</pre></div>
          <div class="output"><pre>total 28
drwxr-xr-x 1 alex alex 4096 Apr 18 12:54 <span class="fg4">.</span>
drwxrwxrwx 1 alex alex 4096 Apr 18 12:38 <span class="fg4 bg2">..</span>
-rw-r--r-- 1 alex alex 8199 Apr 18 12:48 Cargo.lock</pre>
          </div>
        </div>
      </foreignObject>
    </svg>
"#;

const LEGACY_SVG: &[u8] = br#"
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

const PURE_SVG: &[u8] = br#"
<svg viewBox="0 0 720 138" width="720" height="138" xmlns="http://www.w3.org/2000/svg">
  <svg x="0" y="10" width="720" height="118" viewBox="0 0 720 118">
    <g class="input-bg"><rect x="0" y="0" width="100%" height="22"></rect></g>
    <g class="container fg7"><g class="input"><g><text class="prompt">$</text><text> ls -al --color&#x3D;always</text><text>
</text></g></g><g class="output"><g><text>total 28</text><text>
</text></g><g><text>drwxr-xr-x 1 alex alex 4096 Apr 18 12:54 </text><text class="fg4">.</text><text>
</text></g><text>drwxrwxrwx 1 alex alex 4096 Apr 18 12:38 </text><text class="fg4 bg2">..</text><text>
</text><g><text>-rw-r--r-- 1 alex alex 8199 Apr 18 12:48 Cargo.lock</text><text>
</text></g></g></g>
  </svg>
</svg>
"#;

#[test_casing(3, [SVG, LEGACY_SVG, PURE_SVG])]
fn reading_file(file_contents: &[u8]) {
    let transcript = Transcript::from_svg(file_contents).unwrap();
    assert_eq!(transcript.interactions().len(), 1);

    let interaction = &transcript.interactions()[0];
    assert_eq!(interaction.input().as_ref(), "ls -al --color=always");
    assert_eq!(interaction.input().prompt(), Some("$"));

    let plaintext = interaction.output().text();
    assert!(plaintext.starts_with("total 28\ndrwxr-xr-x"));
    assert!(plaintext.contains("4096 Apr 18 12:54 .\n"));
    assert!(!plaintext.contains(r#"<span class="fg4">.</span>"#));
    assert!(!plaintext.contains("__"), "{plaintext}");

    let color_spans = interaction.output().as_str().spans();
    assert_eq!(
        color_spans.len(),
        5,
        "{:#?}",
        color_spans.collect::<Vec<_>>()
    ); // 2 colored regions + 3 surrounding areas
}

#[test]
fn reading_file_with_extra_info() {
    let mut data = SVG.to_owned();
    data.extend_from_slice(b"<other>data</other>");
    let mut data = Cursor::new(data.as_slice());

    let transcript = Transcript::from_svg(&mut data).unwrap();
    assert_eq!(transcript.interactions().len(), 1);

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
              <div class="input"><pre><span class="prompt">$</span> ls &gt; /dev/null</pre></div>
              <div class="input"><pre><span class="prompt">$</span> ls</pre></div>
              <div class="output"><pre>total 28
drwxr-xr-x 1 alex alex 4096 Apr 18 12:54 <span class="fg-blue">.</span>
drwxrwxrwx 1 alex alex 4096 Apr 18 12:38 <span class="fg-blue bg-green">..</span>
-rw-r--r-- 1 alex alex 8199 Apr 18 12:48 Cargo.lock</pre>
              </div>
            </div>
          </foreignObject>
        </svg>
    "#;

    let transcript = Transcript::from_svg(SVG).unwrap();
    assert_eq!(transcript.interactions().len(), 2);

    assert_eq!(
        transcript.interactions()[0].input().as_ref(),
        "ls > /dev/null"
    );
    assert!(transcript.interactions()[0].output().text().is_empty());

    assert_eq!(transcript.interactions()[1].input().as_ref(), "ls");
    assert!(!transcript.interactions()[1].output().text().is_empty());
}

#[test]
fn reading_file_with_exit_code_info() {
    const SVG: &[u8] = br#"
        <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg">
          <foreignObject x="0" y="0" width="652" height="344">
            <div xmlns="http://www.w3.org/1999/xhtml" class="container">
              <div class="input input-failure" data-exit-status="127"><pre><span class="prompt">$</span> what</pre></div>
            </div>
          </foreignObject>
        </svg>
    "#;

    let transcript = Transcript::from_svg(SVG).unwrap();
    assert_eq!(transcript.interactions().len(), 1);

    let interaction = &transcript.interactions()[0];
    assert_eq!(interaction.input().as_ref(), "what");
    assert_eq!(interaction.exit_status(), Some(ExitStatus(127)));
}

#[test]
fn reading_file_with_hidden_input() {
    const SVG: &[u8] = br#"
        <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg">
          <foreignObject x="0" y="0" width="652" height="344">
            <div xmlns="http://www.w3.org/1999/xhtml" class="container">
              <div class="input input-hidden" data-exit-status="127"><pre><span class="prompt">$</span> what</pre></div>
            </div>
          </foreignObject>
        </svg>
    "#;

    let transcript = Transcript::from_svg(SVG).unwrap();
    assert_eq!(transcript.interactions().len(), 1);
    assert!(transcript.interactions()[0].input().is_hidden());
}

#[test]
fn invalid_exit_code_info() {
    const SVG: &[u8] = br#"
        <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg">
          <foreignObject x="0" y="0" width="652" height="344">
            <div xmlns="http://www.w3.org/1999/xhtml" class="container">
              <div class="input input-failure" data-exit-status="??"><pre><span class="prompt">$</span> what</pre></div>
            </div>
          </foreignObject>
        </svg>
    "#;

    let err = Transcript::from_svg(SVG).unwrap_err();
    assert_matches!(err.inner(), ParseError::InvalidExitStatus(_));
    assert!(err.location().start >= 200);
    assert!(!err.location().is_empty());
}

#[test]
fn reading_file_without_svg_tag() {
    let data: &[u8] = b"<div>Text</div>";
    let err = Transcript::from_svg(data).unwrap_err();

    assert_matches!(err.inner(), ParseError::UnexpectedRoot(tag) if tag == "div");
}

#[test]
fn reading_file_without_container() {
    let bogus_data: &[u8] = br#"
        <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg" version="1.1">
          <style>.container { color: #eee; }</style>
        </svg>"#;
    let err = Transcript::from_svg(bogus_data).unwrap_err();

    assert_matches!(err.inner(), ParseError::UnexpectedEof);
    assert_eq!(err.location().start, bogus_data.len());
    assert!(err.location().is_empty());
}

const INVALID_ATTRS: [&str; 5] = [
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

#[test_casing(5, INVALID_ATTRS)]
fn reading_file_with_invalid_container(attrs: &str) {
    let bogus_data = format!(
        r#"
        <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg" version="1.1">
          <foreignObject x="0" y="0" width="652" height="344">
            <div {attrs}>Test</div>
          </foreignObject>
        </svg>
        "#
    );
    let err = Transcript::from_svg(bogus_data.as_bytes()).unwrap_err();

    assert_matches!(err.inner(), ParseError::InvalidContainer);
    assert_ne!(err.location().start, 0);
    assert!(!err.location().is_empty());
}

#[test]
fn reading_user_input_with_manual_events() {
    let mut state = UserInputState::new(None, false);
    {
        let event = Event::Start(BytesStart::new("pre"));
        assert!(state.process(event, 0..1).unwrap().is_none());
        assert_eq!(state.prompt_open_tags, None);
        assert!(state.text.plaintext_buffer().is_empty());
    }
    {
        let event = Event::Start(BytesStart::from_content(r#"span class="prompt""#, 4));
        assert!(state.process(event, 1..2).unwrap().is_none());
        assert_eq!(state.prompt_open_tags, Some(2));
        assert!(state.text.plaintext_buffer().is_empty());
    }
    {
        let event = Event::Text(BytesText::from_escaped("$"));
        assert!(state.process(event, 2..3).unwrap().is_none());
        assert_eq!(state.text.plaintext_buffer(), "$");
    }
    {
        let event = Event::End(BytesEnd::new("span"));
        assert!(state.process(event, 3..4).unwrap().is_none());
        assert_eq!(state.prompt.as_deref(), Some("$"));
        assert!(state.text.plaintext_buffer().is_empty());
    }
    {
        let event = Event::Text(BytesText::from_escaped(" rainbow"));
        assert!(state.process(event, 4..5).unwrap().is_none());
    }

    let event = Event::End(BytesEnd::new("pre"));
    assert!(state.process(event, 5..6).unwrap().is_none());

    let event = Event::End(BytesEnd::new("div"));
    let user_input = state.process(event, 6..7).unwrap().unwrap().input;
    assert_eq!(user_input.prompt(), Some("$"));
    assert_eq!(user_input.as_ref(), "rainbow");
}

fn read_user_input(input: &[u8]) -> UserInput {
    let mut wrapped_input = Vec::with_capacity(input.len() + 11);
    wrapped_input.extend_from_slice(b"<div>");
    wrapped_input.extend_from_slice(input);
    wrapped_input.extend_from_slice(b"</div>");

    let mut reader = XmlReader::from_reader(wrapped_input.as_slice());
    let mut state = UserInputState::new(None, false);

    // Skip the `<div>` start event.
    while !matches!(reader.read_event().unwrap(), Event::Start(_)) {
        // Drop the event.
    }

    loop {
        let event = reader.read_event().unwrap();
        if let Event::Eof = &event {
            panic!("Reached EOF without creating `UserInput`");
        }

        if let Some(interaction) = state.process(event, 0..1).unwrap() {
            break interaction.input;
        }
    }
}

#[test]
fn reading_user_input_base_case() {
    let user_input = read_user_input(br#"<pre><span class="prompt">&gt;</span> echo foo</pre>"#);

    assert_eq!(user_input.prompt(), Some(">"));
    assert_eq!(user_input.as_ref(), "echo foo");
}

#[test]
fn reading_user_input_without_wrapper() {
    let user_input = read_user_input(br#"<span class="prompt">&gt;</span> echo foo"#);

    assert_eq!(user_input.prompt(), Some(">"));
    assert_eq!(user_input.as_ref(), "echo foo");
}

#[test]
fn reading_user_input_without_prompt() {
    let user_input = read_user_input(br#"<pre>echo <span class="bold">foo</span></pre>"#);

    assert_eq!(user_input.prompt(), None);
    assert_eq!(user_input.as_ref(), "echo foo");
}

#[test]
fn reading_user_input_with_prompt_only() {
    let user_input = read_user_input(br#"<pre><span class="prompt">$</span></pre>"#);

    assert_eq!(user_input.prompt(), Some("$"));
    assert_eq!(user_input.as_ref(), "");
}

#[test]
fn reading_user_input_with_bogus_prompt_location() {
    let user_input =
        read_user_input(br#"<pre>echo foo <span class="prompt">&gt;</span> output.log</pre>"#);

    assert_eq!(user_input.prompt(), None);
    assert_eq!(user_input.as_ref(), "echo foo > output.log");
}

#[test]
fn reading_user_input_with_multiple_prompts() {
    let user_input = read_user_input(
        b"<pre><span class=\"prompt\">&gt;&gt;&gt;</span> \
          echo foo <span class=\"prompt\">&gt;</span> output.log</pre>",
    );

    assert_eq!(user_input.prompt(), Some(">>>"));
    assert_eq!(user_input.as_ref(), "echo foo > output.log");
}

#[test]
fn reading_user_input_with_leading_spaces() {
    let user_input =
        read_user_input(b"<pre><span class=\"prompt\">&gt;</span>   ls &gt; /dev/null</pre>");
    assert_eq!(user_input.as_ref(), "  ls > /dev/null");
}

#[test]
fn newline_breaks_are_normalized() {
    let mut state = TextReadingState::default();
    let text = BytesText::from_escaped("some\ntext\r\nand more text");
    state.process(Event::Text(text), 0..1).unwrap();
    assert_eq!(state.plaintext_buffer(), "some\ntext\nand more text");
}

#[test]
fn parser_errors_on_unknown_entity() {
    const SVG: &[u8] = br#"
        <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg">
          <foreignObject x="0" y="0" width="652" height="344">
            <div xmlns="http://www.w3.org/1999/xhtml" class="container">
              <div class="input"><pre>&what;</pre></div>
            </div>
          </foreignObject>
        </svg>
    "#;

    let err = Transcript::from_svg(SVG).unwrap_err();

    assert_matches!(
        err.inner(),
        ParseError::Xml(quick_xml::Error::Escape(
            quick_xml::escape::EscapeError::UnrecognizedEntity(pos, entity),
        )) if entity == "what" && *pos == err.location()
    );
}

#[test]
fn reading_legacy_pure_svg_with_hard_breaks() {
    const SVG: &str = r#"
<svg x="0" y="10" width="720" height="342" viewBox="0 0 720 342">
  <g class="container fg7"><g xml:space="preserve" class="input"><text x="10" y="16"><tspan class="prompt">$</tspan> font-subset info RobotoMono.ttf
</text></g><g xml:space="preserve" class="output"><text x="42" y="42"><tspan class="bold">Roboto Mono</tspan> <tspan class="dimmed">Regular</tspan>
</text><text x="42" y="60"><tspan class="bold">License:</tspan> This Font Software is licensed under the SIL Open Font License, Version<tspan class="hard-br" dx="5">&gt;</tspan>
</text><text x="42" y="78"> 1.1. This license is available with a FAQ at: https://openfontlicense.org
</text></g>
  </g>
</svg>"#;

    let transcript = Transcript::from_svg(SVG.as_bytes()).unwrap();
    assert_eq!(transcript.interactions().len(), 1);
    let output = transcript.interactions()[0].output();
    assert!(
        output.text().starts_with("Roboto Mono Regular\nLicense:"),
        "{output:#?}"
    );
    assert_eq!(output.text().lines().count(), 2, "{output:#?}");
    assert!(!output.text().contains('>'), "{output:#?}");
}

#[test]
fn reading_pure_svg_with_hard_breaks() {
    const SVG: &str = r#"
<svg x="0" y="10" width="720" height="342" viewBox="0 0 720 342">
  <g class="container fg7"><g class="input"><text x="10" y="16">$</text> <text>font-subset info RobotoMono.ttf
</text></g><g class="output"><text x="42" y="42" class="bold">Roboto Mono </text><text class="dimmed">Regular</text><text>
</text><text x="42" y="60" class="bold">License:</text><text> This Font Software is licensed under the SIL Open Font License, Version</text><text class="hard-br">
</text><text x="42" y="78"> 1.1. This license is available with a FAQ at: https://openfontlicense.org
</text></g>
  </g>
</svg>"#;

    let transcript = Transcript::from_svg(SVG.as_bytes()).unwrap();
    assert_eq!(transcript.interactions().len(), 1);
    let output = transcript.interactions()[0].output();
    assert!(
        output.text().starts_with("Roboto Mono Regular\nLicense:"),
        "{output:#?}"
    );
    assert_eq!(output.text().lines().count(), 2, "{output:#?}");
}

#[test]
fn reading_pure_svg_with_styled_hard_breaks() {
    const SVG: &str = r#"
<svg x="0" y="10" width="720" height="342" viewBox="0 0 720 342">
  <g class="container fg7"><g xml:space="preserve" class="input"><text x="10" y="16"><tspan class="prompt">$</tspan> font-subset info RobotoMono.ttf
</text></g><g xml:space="preserve" class="output"><text x="42" y="60"><tspan class="bold">License:</tspan> <tspan class="fg3">don't know</tspan><tspan class="hard-br" dx="5">&gt;</tspan>
</text><text x="42" y="78"><tspan class="fg3"> lol</tspan>
</text></g>
  </g>
</svg>"#;

    let transcript = Transcript::from_svg(SVG.as_bytes()).unwrap();
    assert_eq!(transcript.interactions().len(), 1);
    let output = transcript.interactions()[0].output();
    assert_eq!(output.text(), "License: don't know lol");
}
