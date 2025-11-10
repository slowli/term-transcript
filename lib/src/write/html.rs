//! `HtmlWriter` and related types.

use std::{fmt, io};

use termcolor::{ColorSpec, WriteColor};

use super::{
    fmt_to_io_error, IndexOrRgb, LineBreak, LineSplitter, StyledSpan, WriteLines, WriteStr,
};

impl StyledSpan {
    #[cfg(feature = "test")]
    pub(crate) fn html(spec: &ColorSpec) -> io::Result<Self> {
        let mut this = Self::new(spec, "color")?;
        this.set_html_bg(spec)?;
        Ok(this)
    }

    #[cfg(feature = "test")]
    pub(crate) fn write_html_tag(self, buffer: &mut String) {
        self.write_tag(buffer, "span")
            .expect("writing to String never fails");
    }

    fn set_html_bg(&mut self, spec: &ColorSpec) -> io::Result<()> {
        use fmt::Write as _;

        let mut back_color_class = String::with_capacity(4);
        back_color_class.push_str("bg");
        let back_color = spec.bg().map(|&color| IndexOrRgb::new(color)).transpose()?;
        match back_color {
            Some(IndexOrRgb::Index(idx)) => {
                let final_idx = if spec.intense() { idx | 8 } else { idx };
                write!(&mut back_color_class, "{final_idx}").unwrap();
                // ^-- `unwrap` is safe; writing to a string never fails.
                self.classes.push(back_color_class);
            }
            Some(IndexOrRgb::Rgb(r, g, b)) => {
                self.styles
                    .push(format!("background: #{r:02x}{g:02x}{b:02x}"));
            }
            None => { /* Do nothing. */ }
        }
        Ok(())
    }
}

/// `WriteColor` implementation that renders output as HTML.
///
/// **NB.** The implementation relies on `ColorSpec`s supplied to `set_color` always having
/// `reset()` flag set. This is true for `TermOutputParser`.
pub(crate) struct HtmlWriter<'a> {
    output: &'a mut dyn fmt::Write,
    is_colored: bool,
    line_splitter: Option<LineSplitter>,
}

impl<'a> HtmlWriter<'a> {
    pub fn new(output: &'a mut dyn fmt::Write, max_width: Option<usize>) -> Self {
        Self {
            output,
            is_colored: false,
            line_splitter: max_width.map(LineSplitter::new),
        }
    }

    fn write_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        let mut span = StyledSpan::new(spec, "color")?;
        span.set_html_bg(spec)?;
        span.write_tag(self, "span")?;
        Ok(())
    }
}

impl WriteStr for HtmlWriter<'_> {
    fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.output.write_str(s).map_err(fmt_to_io_error)
    }
}

impl WriteLines for HtmlWriter<'_> {
    fn line_splitter_mut(&mut self) -> Option<&mut LineSplitter> {
        self.line_splitter.as_mut()
    }

    fn write_line_break(&mut self, br: LineBreak, _char_width: usize) -> io::Result<()> {
        self.write_str(br.as_html())
    }

    fn write_new_line(&mut self, _char_width: usize) -> io::Result<()> {
        self.write_str("\n")
    }
}

impl io::Write for HtmlWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.io_write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl WriteColor for HtmlWriter<'_> {
    fn supports_color(&self) -> bool {
        true
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        debug_assert!(spec.reset());
        self.reset()?;
        if !spec.is_none() {
            self.write_color(spec)?;
            self.is_colored = true;
        }
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        if self.is_colored {
            self.is_colored = false;
            self.write_str("</span>")?;
        }
        Ok(())
    }
}

impl LineBreak {
    fn as_html(self) -> &'static str {
        match self {
            Self::Hard => r#"<b class="hard-br"><br/></b>"#,
        }
    }
}
