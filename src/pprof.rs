use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufWriter, Write},
};

use itertools::Itertools;
use profile_preprocessor::Profile;

fn main() -> Result<(), io::Error> {
    let s = fs::read_to_string("./profile_prove_flattened_2.json")?;
    let profile: Profile = serde_json::from_str(&s)?;

    statistic(&profile);

    Ok(())
}

fn statistic(profile: &Profile) {
    // Showing the main thread and a single thread as all worker threads usually look the same when using rayon
    for thread in profile.threads.iter().take(2) {
        println!("{}", thread.name);

        let count = thread.total_samples();
        let (own, cumulative) = thread.sample_count();

        let top15 = |it: HashMap<_, usize>| {
            it.iter()
                .sorted_by(|a, b| b.1.cmp(a.1))
                .take(15)
                .for_each(|(k, v)| {
                    println!(
                        "{}({}%): {}",
                        v,
                        v * 100 / count,
                        profile.shared.string_array[*k]
                    )
                });
        };

        println!("Own");
        top15(own);

        println!("\nCumulative");
        top15(cumulative);
        println!();
    }
}

fn graph(edge_set: HashSet<Vec<String>>) -> Result<(), io::Error> {
    let file = File::create("output.dot")?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "digraph G{{")?;
    for edge in edge_set {
        writeln!(writer, "\"{}\" -> \"{}\";", edge[0], edge[1])?;
    }
    writeln!(writer, "}}")?;
    writer.flush()?;
    Ok(())
}
