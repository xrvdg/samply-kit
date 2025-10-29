use argh::FromArgs;
use samply_kit::Profile;
use std::{fs, io};

#[derive(FromArgs)]
#[argh(description = "Utility for filtering out frames from a profile")]
struct CMDLine {
    #[argh(positional, description = "input file")]
    input: String,
    #[argh(positional, description = "output file")]
    output: String,
    #[argh(positional)]
    regex: String,
}

fn main() -> Result<(), io::Error> {
    let cmd: CMDLine = argh::from_env();

    let s = fs::read_to_string(cmd.input)?;
    let mut content: Profile = serde_json::from_str(&s)?;

    content.exclude_function(&cmd.regex);

    // statistics?
    println!("weights: {:?}", content.total_samples());

    fs::write(cmd.output, serde_json::to_string_pretty(&content)?)?;

    Ok(())
}
