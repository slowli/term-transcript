//! `HtmlWriter` and related types.

use std::{fmt, io};

use termcolor::ColorSpec;

use super::{IndexOrRgb, Styling, StyledSpan, StyledSpan};

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
                self.push_class(&back_color_class);
            }
            Some(IndexOrRgb::Rgb(r, g, b)) => {
                self.push_style("background", &format!("#{r:02x}{g:02x}{b:02x}"));
            }
            None => { /* Do nothing. */ }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct HtmlStyling;

impl Styling for HtmlStyling {
    fn styles(spec: &ColorSpec) -> io::Result<StyledSpan> {
        let mut fg = StyledSpan::new(spec, "color")?;
        fg.set_html_bg(spec)?;
        Ok(StyledSpan {
            fg,
            ..StyledSpan::default()
        })
    }
}
