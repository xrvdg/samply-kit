use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufWriter, Write},
};

use argh::FromArgs;
use itertools::Itertools;
use samply_kit::{Id, Profile};

#[derive(FromArgs, Debug)]
#[argh(description = "pprof-style analysis for samply profiles")]
struct Args {
    #[argh(positional)]
    file: String,
    #[argh(subcommand)]
    cmd: Command,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
enum Command {
    Lookup(Lookup),
    Statistics(Statistics),
}

#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "stat")]
#[argh(description = "Statistics")]
struct Statistics {}

#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "lookup")]
#[argh(description = "show all traces that contain function")]
struct Lookup {
    #[argh(positional, description = "function id. Can be found via stat")]
    id: usize,
}

fn main() -> Result<(), io::Error> {
    let args: Args = argh::from_env();
    let s = fs::read_to_string(args.file)?;
    let profile: Profile = serde_json::from_str(&s)?;

    match args.cmd {
        Command::Lookup(lookup) => reverse_search(&profile, lookup.id),
        Command::Statistics(_statistics) => statistic(&profile),
    }

    Ok(())
}

fn reverse_search(profile: &Profile, string_idx: usize) {
    let traces = profile.reverse_search(Id::new(string_idx));
    for (i, (mut trace, count)) in traces
        .into_iter()
        .sorted_by(|a, b| b.1.cmp(&a.1))
        .enumerate()
    {
        trace.reverse();
        print!("{i}: #{count} ");
        for func in trace {
            print!("{} -> ", profile.shared.string_array[func]);
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
            println!("#COUNT(%): FUNCTION_ID NAME");
            it.iter()
                .sorted_by(|a, b| b.1.cmp(a.1))
                .take(n)
                .for_each(|(k, v)| {
                    println!(
                        "#{}({}%): {} {}",
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
