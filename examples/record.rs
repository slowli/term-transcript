use std::io;

use term_svg::{ShellOptions, SvgTemplate, SvgTemplateOptions, Transcript, UserInput};

fn main() -> anyhow::Result<()> {
    let cmd = "for i in {0..10}; do echo 'term-svg is awesome!'; done | lolcat -F 0.7 -f";
    let transcript =
        Transcript::from_inputs(&mut ShellOptions::default(), vec![UserInput::command(cmd)])?;

    let mut template = SvgTemplate::new(SvgTemplateOptions::default());
    template.render(&transcript, io::stdout())?;
    Ok(())
}
