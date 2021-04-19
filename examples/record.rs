use std::io::{self, Read};

use term_svg::{SvgTemplate, SvgTemplateOptions, Transcript, UserInput};

fn main() -> anyhow::Result<()> {
    let mut term_output = vec![];
    io::stdin().read_to_end(&mut term_output)?;

    let cmd = "for i in {0..10}; do echo 'term-svg is awesome!'; done | lolcat -F 0.7 -f";
    let mut transcript = Transcript::new();
    transcript
        .add_interaction(UserInput::Command(cmd), &term_output)
        .add_interaction(UserInput::Command(cmd), &term_output);

    let mut template = SvgTemplate::new(SvgTemplateOptions::default());
    template.render(&transcript, io::stdout())?;
    Ok(())
}
