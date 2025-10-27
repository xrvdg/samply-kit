use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, BufWriter, Write},
};

use profile_preprocessor::{Id, Profile};

fn main() -> Result<(), io::Error> {
    let s = fs::read_to_string("./profile_prove_flattened.json")?;
    let mut profile: Profile = serde_json::from_str(&s)?;

    let thread = &mut profile.threads[0];
    let file = File::create("output.dot")?;
    let mut writer = BufWriter::new(file);
    let paths = thread.paths();
    let paths: Vec<Vec<_>> = paths
        .iter()
        .map(|v| {
            v.iter()
                .map(|id| profile.shared.string_array[thread.func_table.name[*id]].to_owned())
                .collect()
        })
        .collect();

    let count = thread.samples.total_weight();

    // TODO should be more efficient to do on integers and then retrieve the names
    let mut edge_set = HashSet::new();
    for path in paths {
        for pair in path.windows(2) {
            edge_set.insert(pair.to_owned());
        }
    }

    writeln!(writer, "digraph G{{")?;
    for edge in edge_set {
        writeln!(writer, "\"{}\" -> \"{}\";", edge[0], edge[1])?;
    }
    writeln!(writer, "}}")?;

    // Count samples
    // Don't include those with less than %X samples

    writer.flush()?;
    Ok(())
}
