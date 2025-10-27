use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufWriter, Write},
};

use itertools::Itertools;
use profile_preprocessor::{Id, Profile};

fn main() -> Result<(), io::Error> {
    let s = fs::read_to_string("./profile_prove_flattened_2.json")?;
    let profile: Profile = serde_json::from_str(&s)?;

    statistic(&profile);
    reverse_search(&profile, 4713);

    Ok(())
}

fn reverse_search(profile: &Profile, string_idx: usize) {
    let traces = profile.reverse_search(Id::new(string_idx));
    for (i, trace) in traces.iter().enumerate() {
        print!("{i}: ");
        for func in trace {
            print!("{} -> ", profile.shared.string_array[*func]);
        }
        println!();
    }
}

fn statistic(profile: &Profile) {
    // Showing the main thread and a single thread as all worker threads usually look the same when using rayon
    for thread in profile.threads.iter().take(2) {
        println!("{}", thread.name);

        let count = thread.total_samples();
        let (own, cumulative) = thread.sample_count();

        let top = |n, it: HashMap<_, usize>| {
            it.iter()
                .sorted_by(|a, b| b.1.cmp(a.1))
                .take(n)
                .for_each(|(k, v)| {
                    println!(
                        "{}({}%): {} {}",
                        v,
                        v * 100 / count,
                        *k,
                        profile.shared.string_array[*k]
                    )
                });
        };

        println!("Self");
        top(25, own);

        println!("\nCumulative");
        top(25, cumulative);
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
