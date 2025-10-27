use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufWriter, Write},
};

use itertools::Itertools;
use profile_preprocessor::{FuncIdx, Id, Profile};

fn main() -> Result<(), io::Error> {
    let s = fs::read_to_string("./profile_prove_flattened_2.json")?;
    let profile: Profile = serde_json::from_str(&s)?;

    let mut total_count = 0;
    let mut total_cumulative = HashMap::new();
    let mut total_own = HashMap::new();
    for thread in profile.threads {
        println!("{}", thread.name);

        let count = thread.samples.total_weight();

        let mut cumulative = HashMap::new();
        let mut own = HashMap::new();
        for (id, stack) in thread.samples.stack.inner.iter().enumerate() {
            let add = match &thread.samples.weight {
                Some(weigth_vec) => weigth_vec[Id::new(id)],
                None => 1,
            };
            let path = thread.path(*stack);
            if let Some(last) = path.last().copied() {
                *own.entry(last).or_insert(0) += add
            }
            // Only count a function once in a sample. Recursion should not lead to multiple counts
            let path: HashSet<FuncIdx> = HashSet::from_iter(path);
            for func in path {
                *cumulative.entry(func).or_insert(0) += add
            }
        }

        println!("Own");
        own.iter()
            .sorted_by(|a, b| b.1.cmp(a.1))
            .take(15)
            .for_each(|(k, v)| {
                println!(
                    "{}({}%): {}",
                    v,
                    v * 100 / count,
                    profile.shared.string_array[thread.func_table.name[*k]]
                )
            });

        println!("cumulative");
        cumulative
            .iter()
            .sorted_by(|a, b| b.1.cmp(a.1))
            .take(15)
            .for_each(|(k, v)| {
                println!(
                    "{}({}%): {}",
                    v,
                    v * 100 / count,
                    profile.shared.string_array[thread.func_table.name[*k]]
                )
            });
        println!();
        total_count += count;

        for (key, value) in cumulative {
            *total_cumulative
                .entry(thread.func_table.name[key])
                .or_insert(0) += value;
        }
        for (key, value) in own {
            *total_own.entry(thread.func_table.name[key]).or_insert(0) += value;
        }
    }

    // Total not that useful
    println!("Totals");
    println!("Own");
    total_own
        .iter()
        .sorted_by(|a, b| b.1.cmp(a.1))
        .take(15)
        .for_each(|(k, v)| {
            println!(
                "{}({}%): {}",
                v,
                v * 100 / total_count,
                profile.shared.string_array[*k]
            )
        });

    println!("cumulative");
    total_cumulative
        .iter()
        .sorted_by(|a, b| b.1.cmp(a.1))
        .take(15)
        .for_each(|(k, v)| {
            println!(
                "{}({}%): {}",
                v,
                v * 100 / total_count,
                profile.shared.string_array[*k]
            )
        });

    Ok(())
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
