//! `HtmlWriter` and related types.

use std::{fmt, io};

use serde::Serialize;
use termcolor::ColorSpec;

use super::{IndexOrRgb, LineBreak, StyledLine, StyledSpan};

impl StyledSpan {
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

#[derive(Debug, Default, Serialize)]
pub(crate) struct HtmlLine {
    pub html: String,
    pub br: Option<LineBreak>,
}

impl AsMut<String> for HtmlLine {
    fn as_mut(&mut self) -> &mut String {
        &mut self.html
    }
}

impl StyledLine for HtmlLine {
    fn write_color(&mut self, spec: &ColorSpec, _start_pos: usize) -> io::Result<()> {
        let mut span = StyledSpan::new(spec, "color")?;
        span.set_html_bg(spec)?;
        span.write_tag(&mut self.html, "span");
        Ok(())
    }

    fn reset_color(&mut self, _prev_spec: &ColorSpec, _current_width: usize) {
        self.html.push_str("</span>");
    }

    fn set_br(&mut self, br: Option<LineBreak>) {
        self.br = br;
    }
}
