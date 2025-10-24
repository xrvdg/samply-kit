use std::{
    collections::{BTreeMap, HashSet},
    fs::{self},
    io,
};

use itertools::{self, Itertools};
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn main() -> Result<(), io::Error> {
    let s = fs::read_to_string("./profile.json")?;
    let mut content: Profile = serde_json::from_str(&s)?;

    content.exclude_function("try");

    fs::write(
        "./test_profile.json",
        serde_json::to_string_pretty(&content)?,
    )?;

    Ok(())
}

fn summary(content: &Profile) {
    println!("{content:?}");

    content.threads[0]
        .func_table
        .name
        .iter()
        .enumerate()
        .for_each(|(i, id)| println!("{i} {}: {}", id, content.shared.string_array[*id]));

    println!("\nframe table");
    content.threads[0]
        .frame_table
        .func
        .iter()
        .enumerate()
        .for_each(|(i, func_id)| {
            println!(
                "{i} {}: {}",
                func_id, content.shared.string_array[content.threads[0].func_table.name[*func_id]]
            )
        });

    println!("\nstack table");
    content.threads[0]
        .stack_table
        .paths()
        .iter()
        .enumerate()
        .for_each(|(i, path)| println!("{i}: {:?}", path));

    println!("\nsample table");
    content.threads[0]
        .samples
        .stack
        .iter()
        .enumerate()
        .for_each(|(i, stack_id)| {
            println!(
                "{i}: {stack_id} \t weight: {:?} \t path: {:?}",
                content.threads[0].samples.weight[i],
                content.threads[0].stack_table.path(*stack_id)
            )
        });
}

impl StackTable {
    fn path(&self, id: IndexToStackTable) -> Vec<usize> {
        let mut p = match self.prefix[id] {
            Some(prefix_id) => self.path(prefix_id),
            None => Vec::new(),
        };
        p.push(self.frame[id]);
        p
    }

    fn paths(&self) -> Vec<Vec<usize>> {
        let mut p = Vec::with_capacity(self.length);
        for i in 0..self.length {
            p.push(self.path(i));
        }
        p
    }

    // TODO tree paths?

    /// Rewrites the prefix attribute until the point a non-excluded parent is reached
    fn exclude_parent(&mut self, id: IndexToStackTable, excluded: &HashSet<IndexToStackTable>) {
        if let Some(prefix_id) = self.prefix[id] {
            if excluded.contains(&prefix_id) {
                self.prefix[id] = self.prefix[prefix_id];
                self.exclude_parent(id, excluded);
            }
        }
    }

    /// Rewrite such that non-excluded frames do not point at excluded frames anymore.
    /// Excluded frames themselves stay included to not mess up the indexing and they act as a fast way to
    fn exclude(&mut self, excluded: &HashSet<IndexToStackTable>) {
        for i in 0..self.length {
            self.exclude_parent(i, &excluded);
        }
    }
}

impl Thread {
    fn exclude_function(&mut self, exclude_string_table: &HashSet<usize>) {
        let exclude_func_table: HashSet<_> = self
            .func_table
            .name
            .iter()
            .positions(|id| exclude_string_table.contains(id))
            .collect();

        let exclude_frame_table: HashSet<_> = self
            .frame_table
            .func
            .iter()
            .positions(|id| exclude_func_table.contains(id))
            .collect();

        let exclude_stack_table: HashSet<_> = self
            .stack_table
            .frame
            .iter()
            .positions(|id| exclude_frame_table.contains(id))
            .collect();

        let new_stack = &mut self.stack_table;

        new_stack.exclude(&exclude_stack_table);

        self.fixup_samples(&exclude_stack_table);
    }

    fn fixup_samples(&mut self, excluded: &HashSet<IndexToStackTable>) {
        for s in &mut self.samples.stack {
            if excluded.contains(s) {
                if let Some(prefix) = self.stack_table.prefix[*s] {
                    *s = prefix
                }
            }
        }
    }
}

impl Profile {
    fn exclude_function(&mut self, function_wildcard: &str) {
        let exclude_string_table: HashSet<_> = self
            .shared
            .string_array
            .iter()
            .positions(|string| string.contains(function_wildcard))
            .collect();

        for thread in &mut self.threads {
            thread.exclude_function(&exclude_string_table);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Profile {
    threads: Vec<Thread>,
    shared: ProfileSharedData,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Thread {
    samples: SampleTable,
    #[serde(rename = "stackTable")]
    stack_table: StackTable,
    #[serde(rename = "frameTable")]
    frame_table: FrameTable,
    #[serde(rename = "funcTable")]
    func_table: FuncTable,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SampleTable {
    stack: Vec<IndexToStackTable>,
    weight: Vec<Option<usize>>,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StackTable {
    prefix: Vec<Option<IndexToStackTable>>,
    frame: Vec<IndexToFrameTable>,
    length: usize,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct FrameTable {
    func: Vec<IndexToFuncTable>,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

type IndexToStackTable = usize;
type IndexToFrameTable = usize;
type IndexToFuncTable = usize;
type IndexToStringTable = usize;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FuncTable {
    name: Vec<IndexToStringTable>,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProfileSharedData {
    #[serde(rename = "stringArray")]
    string_array: Vec<String>,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}
