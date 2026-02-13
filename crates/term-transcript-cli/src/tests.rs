//! Checks consistency of example snapshots and regenerates examples if appropriate.
//!
//! By default, all differing snapshots will be written near the real ones with the `.new.svg` extension.
//! This behavior is controlled via the following env vars:
//!
//! - `CI`: if set, disables writing differing snapshots.
//! - `TT_IMG_FILTER`: filters snapshots by the image name. E.g., `TT_IMG_FILTER=pure` will only check snapshots
//!   with "pure" in the image name. Only snapshot generation is skipped; the command is still checked.
//! - `TT_IMG_SKIP`: same as `TT_IMG_FILTER`, but skips snapshot generation on match.

use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    fmt, fs, io,
    io::BufReader,
    mem,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use clap::Parser as _;
use pulldown_cmark::{CodeBlockKind, Event, LinkType, Parser, Tag};
use term_transcript::Transcript;

use crate::{Cli, Command, shell::ShellArgs};

/// Directory with SVG snapshots.
fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/src/assets")
}

/// Directory with Markdown files linking to SVG snapshots.
fn markdown_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/src/examples")
}

fn rainbow_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e-tests/rainbow/bin")
}

#[derive(Debug, PartialEq)]
enum ParsingState {
    Normal { is_escape: bool },
    Comment,
    SingleQuote,
    DoubleQuote,
    Var { in_double_quote: bool },
}

fn split_into_args(command: &str, env_vars: &HashMap<&'static str, String>) -> Vec<String> {
    let mut args = vec![];
    let mut current_arg = String::new();
    let mut current_arg_has_quotes = false;
    let mut current_var = String::new();
    let mut state = ParsingState::Normal { is_escape: false };
    // Add a surrogate ' ' at the end to terminate non-normal states.
    for (idx, ch) in command.char_indices().chain([(command.len(), ' ')]) {
        let next_char = command.as_bytes().get(idx + 1).copied();
        match &mut state {
            ParsingState::Normal { is_escape } => {
                if *is_escape {
                    assert_eq!(ch, '\n', "escape not supported");
                    *is_escape = false;
                    continue;
                }

                match ch {
                    '\'' => {
                        current_arg_has_quotes = true;
                        state = ParsingState::SingleQuote;
                    }
                    '"' => {
                        current_arg_has_quotes = true;
                        state = ParsingState::DoubleQuote;
                    }
                    '\\' => {
                        assert_eq!(next_char, Some(b'\n'), "escape not supported");
                        // Gobble the escaped newline
                        *is_escape = true;
                    }
                    '\n' => panic!("unescaped newline in {command}"),
                    '#' => {
                        state = ParsingState::Comment;
                    }
                    '$' => {
                        let next_char = next_char.expect("unfinished var");
                        assert!(next_char.is_ascii_alphabetic());
                        state = ParsingState::Var {
                            in_double_quote: false,
                        };
                    }
                    ch if ch.is_ascii_whitespace() => {
                        // **NB.** This is the only place where the current arg is reset.
                        if !current_arg.is_empty() || current_arg_has_quotes {
                            args.push(mem::take(&mut current_arg));
                            current_arg_has_quotes = false;
                        }
                    }
                    _ => {
                        current_arg.push(ch);
                    }
                }
            }
            ParsingState::Comment => {
                if ch == '\n' {
                    state = ParsingState::Normal { is_escape: false };
                }
                // Otherwise, gobble the current char
            }
            ParsingState::SingleQuote => {
                if ch == '\'' {
                    state = ParsingState::Normal { is_escape: false };
                } else {
                    current_arg.push(ch);
                }
            }
            ParsingState::DoubleQuote => match ch {
                '"' => state = ParsingState::Normal { is_escape: false },
                '\\' => panic!("escapes are not supported in double-quoted strings"),
                '$' => {
                    let next_char = next_char.expect("unfinished var");
                    assert!(next_char.is_ascii_alphabetic());
                    state = ParsingState::Var {
                        in_double_quote: true,
                    };
                }
                _ => {
                    current_arg.push(ch);
                }
            },
            ParsingState::Var { in_double_quote } => {
                current_var.push(ch);
                let next_char = next_char.expect("unfinished var");

                // We perform a look forward to not lose the next char.
                if next_char != b'_' && !next_char.is_ascii_alphanumeric() {
                    let var_name = mem::take(&mut current_var);
                    let var_value = env_vars.get(var_name.as_str()).unwrap_or_else(|| {
                        panic!("env var {var_name} is undefined");
                    });
                    // This pretends that the var doesn't contain whitespace even if `!in_double_quote`.
                    current_arg.push_str(var_value);

                    state = if *in_double_quote {
                        ParsingState::DoubleQuote
                    } else {
                        ParsingState::Normal { is_escape: false }
                    };
                }
            }
        }
    }

    assert_eq!(state, ParsingState::Normal { is_escape: false });
    assert!(current_arg.is_empty());
    assert!(current_var.is_empty());

    args
}

#[test]
fn splitting_into_args_works() {
    let command = "term-transcript exec";
    let env = HashMap::from([
        ("FONT_ROBOTO", "roboto.ttf".to_owned()),
        ("FONT_ROBOTO_ITALIC", "roboto-it.ttf".to_owned()),
    ]);
    let args = split_into_args(command, &env);
    assert_eq!(args, ["term-transcript", "exec"]);

    let command = "term-transcript exec -T='100ms' \\\n --palette gjm8";
    let args = split_into_args(command, &env);
    assert_eq!(
        args,
        ["term-transcript", "exec", "-T=100ms", "--palette", "gjm8"]
    );

    let command = "term-transcript exec -T='100ms' \\\n  --continued-mark '' \\\n  # Embed font\n--embed-font=\"$FONT_ROBOTO:$FONT_ROBOTO_ITALIC\"";
    let args = split_into_args(command, &env);
    assert_eq!(
        args,
        [
            "term-transcript",
            "exec",
            "-T=100ms",
            "--continued-mark",
            "",
            "--embed-font=roboto.ttf:roboto-it.ttf"
        ]
    );
}

#[cfg(feature = "tracing")]
fn setup_test_tracing() {
    use tracing_subscriber::{EnvFilter, FmtSubscriber};

    FmtSubscriber::builder()
        .pretty()
        .with_test_writer()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .ok();
}

#[test]
fn examples_are_consistent() {
    #[cfg(feature = "tracing")]
    setup_test_tracing();

    let assets_dir = assets_dir();
    env::set_current_dir(&assets_dir).expect("cannot change current dir");

    let markdown_dir = markdown_dir();
    let dir = fs::read_dir(&markdown_dir).unwrap();
    let markdown_files = dir.filter_map(|file| {
        let file = file.unwrap();
        if !file.file_type().unwrap().is_file() {
            return None;
        }
        let path = file.path();
        path.extension()
            .is_some_and(|ext| ext == "md")
            .then_some(path)
    });

    let temp_dir = tempfile::tempdir().unwrap();
    let img_filter = env::var("TT_IMG_FILTER").unwrap_or_else(|_| String::new());
    let img_skip_filter = env::var("TT_IMG_SKIP").ok();
    let threads: Vec<_> = markdown_files
        .flat_map(|file_path| {
            parse_markdown_file(
                &file_path,
                temp_dir.path(),
                &img_filter,
                img_skip_filter.as_deref(),
            )
        })
        .collect();

    // Wait for all snapshot generation threads to finish.
    let failures: Vec<_> = threads
        .into_iter()
        .filter_map(|(img_path, handle)| handle.join().is_err().then_some(img_path))
        .collect();
    assert!(failures.is_empty(), "Some examples failed: {failures:#?}");
}

fn parse_markdown_file(
    file_path: &Path,
    temp_dir: &Path,
    img_filter: &str,
    img_skip_filter: Option<&str>,
) -> Vec<(String, thread::JoinHandle<()>)> {
    let markdown = fs::read_to_string(file_path).expect("cannot read readme");
    let parser = Parser::new(&markdown);

    let mut shell_command = None;
    let mut img_path = None;
    let mut threads = vec![];
    for event in parser {
        match event {
            Event::Start(Tag::Image(LinkType::Inline, path, _)) if path.ends_with(".svg") => {
                img_path = Some(path);
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                if lang.as_ref() == "bash" =>
            {
                assert!(shell_command.is_none(), "Embedded code samples");
                shell_command = Some(String::with_capacity(1_024));
            }
            Event::End(Tag::Heading(..)) => {
                if let Some(img_path) = &img_path {
                    panic!("Image not having the following shell code: {img_path}");
                }
            }
            Event::End(Tag::CodeBlock(_)) => {
                let Some(shell_command) = shell_command.take() else {
                    continue;
                };
                assert!(!shell_command.is_empty());

                let img_path = img_path.take();
                let Some(cli) = prepare_cli(&shell_command, img_path.as_deref()) else {
                    continue;
                };
                let img_path = extract_out_path(&cli)
                    .to_str()
                    .expect("non-UTF8 out path")
                    .to_owned();

                if cfg!(windows)
                    && matches!(
                        &cli.command,
                        Command::Exec {
                            shell: ShellArgs { shell: Some(_), .. },
                            ..
                        }
                    )
                {
                    #[cfg(feature = "tracing")]
                    tracing::info!(img_path, ?cli, "skipping snapshot with specified shell");
                    continue;
                }

                if !img_path.contains(img_filter) {
                    #[cfg(feature = "tracing")]
                    tracing::info!(img_path, img_filter, "snapshot filtered out");
                    continue;
                }
                if let Some(skip_filter) = &img_skip_filter {
                    if img_path.contains(skip_filter) {
                        #[cfg(feature = "tracing")]
                        tracing::info!(img_path, skip_filter, "snapshot filtered out");
                        continue;
                    }
                }

                // Spawn snapshot generation into a separate thread so that it's effectively parallelized.
                let temp_dir = temp_dir.to_owned();
                let handle = thread::spawn(move || {
                    check_snapshot(cli, &temp_dir);
                });
                threads.push((img_path, handle));
            }
            Event::Text(text) => {
                if let Some(code) = &mut shell_command {
                    code.push_str(text.as_ref());
                }
            }
            _ => { /* do nothing */ }
        }
    }
    threads
}

fn prepare_cli(command: &str, img_path: Option<&str>) -> Option<Cli> {
    let rainbow_dir = rainbow_dir();

    let command = command.trim_end();
    if !command.starts_with("term-transcript exec") {
        #[cfg(feature = "tracing")]
        tracing::info!(?command, "skipping non-exec command");
        return None;
    }
    let args = split_into_args(command, &HashMap::new());
    #[cfg(feature = "tracing")]
    tracing::info!(?args, "split command-line args");

    if cfg!(not(feature = "portable-pty")) && args.iter().any(|arg| arg == "--pty") {
        #[cfg(feature = "tracing")]
        tracing::info!("pty not enabled, skipping test");
        return None;
    }

    let mut args = Cli::try_parse_from(args).unwrap_or_else(|err| panic!("{err}"));
    if let Command::Exec {
        template, shell, ..
    } = &mut args.command
    {
        shell.io_timeout = Duration::from_secs(1).into();
        let path_extension = if cfg!(windows) {
            format!("set PATH={};%PATH%", rainbow_dir.display())
        } else if cfg!(unix) {
            format!("export PATH={}:$PATH", rainbow_dir.display())
        } else {
            panic!("unsupported platform");
        };
        shell.init.push(path_extension);

        #[cfg(all(windows, feature = "portable-pty"))]
        if shell.pty.is_some() {
            // TODO: On Windows, PTY transforms / "optimizes" styles (e.g., merges non-styled whitespace
            //   into the preceding styled span). It may make sense to either optimize spans during capture as well,
            //   and/or compare produced transcripts more smartly.

            #[cfg(feature = "tracing")]
            tracing::info!("disabling PTY because of capturing inconsistencies");
            shell.pty = None;
        }

        let out_path = if let Some(specified_path) = &template.out {
            assert!(img_path.is_none(), "both image and -o option are specified");
            assert!(specified_path.is_relative());
            specified_path.clone()
        } else {
            PathBuf::from(img_path.expect("no image path or -o option in the script"))
        };
        template.out = Some(out_path);
    } else {
        panic!("unexpected command: {args:?}");
    }

    Some(args)
}

fn extract_out_path(cli: &Cli) -> &Path {
    match &cli.command {
        Command::Exec { template, .. } => template.out.as_deref().expect("no out path"),
        _ => panic!("unexpected command: {cli:?}"),
    }
}

// Since `Comparison` uses `fmt::Debug`, we define this simple wrapper
// to switch to `fmt::Display`.
struct DebugStr<'a>(&'a str);

impl fmt::Debug for DebugStr<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

#[cfg_attr(feature = "tracing", tracing::instrument(skip(cli), fields(out)))]
fn check_snapshot(mut cli: Cli, temp_dir: &Path) {
    let out_path = extract_out_path(&cli).to_owned();
    assert!(out_path.is_relative());

    let path_in_assets = out_path.strip_prefix("../assets").unwrap_or(&out_path);
    assert!(
        !path_in_assets.starts_with(".."),
        "bogus out path: {out_path:?}"
    );

    #[cfg(feature = "tracing")]
    tracing::Span::current().record("out", tracing::field::display(path_in_assets.display()));

    let (full_out_path, is_custom_template) =
        if let Command::Exec { template, .. } = &mut cli.command {
            let full_out_path = temp_dir.join(path_in_assets);
            template.out = Some(full_out_path.clone());
            (full_out_path, template.template_path.is_some())
        } else {
            unreachable!()
        };

    cli.command.run().unwrap();
    #[cfg(feature = "tracing")]
    tracing::info!("run command");

    // Read the generated transcript and check that it can be parsed.
    let raw_transcript = fs::read_to_string(&full_out_path).unwrap();
    // Normalize newlines for portability.
    let raw_transcript = raw_transcript.replace("\r\n", "\n");

    #[cfg(feature = "tracing")]
    tracing::info!(
        ?full_out_path,
        byte_len = raw_transcript.len(),
        "read transcript"
    );
    if !is_custom_template {
        let parsed = Transcript::from_svg(BufReader::new(raw_transcript.as_bytes())).unwrap();
        assert!(!parsed.interactions().is_empty());
    }

    let ref_path = assets_dir().join(path_in_assets);
    let mut raw_reference = fs::read_to_string(&ref_path).unwrap_or_else(|err| {
        if matches!(err.kind(), io::ErrorKind::NotFound) {
            String::new() // will lead to a failure later, but that's just what we need
        } else {
            panic!("failed reading reference at {}: {err}", ref_path.display());
        }
    });
    #[cfg(feature = "tracing")]
    tracing::info!(?ref_path, byte_len = raw_reference.len(), "read reference");

    // Normalize newlines for portability.
    raw_reference = raw_reference.replace("\r\n", "\n");
    if cfg!(windows) {
        // Remove `data-exit-status` mentions, which aren't supported by the default shell.
        raw_reference = raw_reference.replace(" data-exit-status=\"0\"", "");
    }

    if raw_reference != raw_transcript {
        let is_ci = env::var_os("CI").is_some_and(|flag| flag != "0");
        if !is_ci {
            let mut save_path = ref_path.clone();
            let extension = save_path.extension().expect("no extension");
            let mut new_extension = OsString::from("new.");
            new_extension.push(extension);
            save_path.set_extension(new_extension);

            fs::write(&save_path, &raw_transcript).unwrap_or_else(|err| {
                panic!(
                    "failed saving new transcript to {}: {err}",
                    save_path.display()
                );
            });
            #[cfg(feature = "tracing")]
            tracing::info!(?save_path, "saved new transcript");
        }

        panic!(
            "Transcript {out_path} failed:\n{cmp}",
            out_path = out_path.display(),
            cmp = pretty_assertions::Comparison::new(
                &DebugStr(&raw_reference),
                &DebugStr(&raw_transcript),
            )
        );
    }
}
