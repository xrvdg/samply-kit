use profile_preprocessor::Profile;
use std::{fs, io};

fn main() -> Result<(), io::Error> {
    let s = fs::read_to_string("./profile_prove.json")?;
    let mut content: Profile = serde_json::from_str(&s)?;

    content.exclude_function("rayon");

    // statistics?
    println!("weights: {:?}", content.total_samples());

    fs::write(
        "./profile_prove_flattened.json",
        serde_json::to_string_pretty(&content)?,
    )?;

    Ok(())
}
