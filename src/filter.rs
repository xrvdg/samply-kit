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
    let content: Profile = serde_json::from_str(&s)?;
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

    println!("\nexclude func table");
    let exclude_func_table: HashSet<_> = content.threads[0]
        .func_table
        .name
        .iter()
        .positions(|id| content.shared.string_array[*id].contains("try"))
        .collect();

    println!("{exclude_func_table:?}");

    println!("\nexclude frame table");
    let exclude_frame_table: HashSet<_> = content.threads[0]
        .frame_table
        .func
        .iter()
        .positions(|id| exclude_func_table.contains(id))
        .collect();

    println!("{exclude_frame_table:?}");

    println!("\nexclude stack table");
    let exclude_stack_table: HashSet<_> = content.threads[0]
        .stack_table
        .frame
        .iter()
        .positions(|id| exclude_frame_table.contains(id))
        .collect();

    println!("{exclude_stack_table:?}");

    let mut new_content = content.clone();

    let new_thread = &mut new_content.threads[0];

    let new_stack = &mut new_thread.stack_table;

    new_stack.exclude(&exclude_stack_table);

    println!("\nnew stack table");
    new_stack
        .paths()
        .iter()
        .enumerate()
        .for_each(|(i, path)| println!("{i}: {:?}", path));

    new_thread.fixup_samples(&exclude_stack_table);

    println!("\nnew sample table");
    new_thread
        .samples
        .stack
        .iter()
        .enumerate()
        .for_each(|(i, stack_id)| {
            println!(
                "{i}: {stack_id}: {:?}",
                new_thread.stack_table.path(*stack_id)
            )
        });

    fs::write(
        "./test_profile.json",
        serde_json::to_string_pretty(&new_content)?,
    )?;

    Ok(())
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

    fn exclude_parent(&mut self, id: IndexToStackTable, excluded: &HashSet<IndexToStackTable>) {
        if let Some(prefix_id) = self.prefix[id] {
            if excluded.contains(&prefix_id) {
                self.prefix[id] = self.prefix[prefix_id];
                self.exclude_parent(id, excluded);
            }
        }
    }

    fn exclude(&mut self, excluded: &HashSet<IndexToStackTable>) {
        for i in 0..self.length {
            self.exclude_parent(i, &excluded);
        }
    }
}

impl Thread {
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
