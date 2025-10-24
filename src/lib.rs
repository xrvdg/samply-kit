use std::collections::{BTreeMap, HashSet};

use itertools::{self, Itertools};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

    /// Squash out excluded prefixes.
    fn squash_excluded_parents(
        &mut self,
        id: IndexToStackTable,
        excluded: &HashSet<IndexToStackTable>,
    ) {
        if let Some(prefix_id) = self.prefix[id] {
            if excluded.contains(&prefix_id) {
                self.prefix[id] = self.prefix[prefix_id];
                self.squash_excluded_parents(id, excluded);
            }
        }
    }

    /// Rewrite such that non-excluded frames do not point at excluded frames anymore.
    /// Excluded frames themselves stay included to not mess up the indexing and they act as a fast way to
    fn exclude(&mut self, excluded: &HashSet<IndexToStackTable>) {
        for i in 0..self.length {
            self.squash_excluded_parents(i, excluded);
        }
    }
}

impl Thread {
    fn exclude_function(&mut self, exclude_string_table: &HashSet<IndexToStringTable>) {
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

        self.stack_table.exclude(&exclude_stack_table);

        self.reattribute_samples(&exclude_stack_table);
    }

    /// Samples that point to an excluded stack entry needs to be reassigned to its parent.
    ///
    /// Run this after stack.exclude to prevent reassing the sample to another excluded stack entry
    fn reattribute_samples(&mut self, excluded: &HashSet<IndexToStackTable>) {
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
    pub fn exclude_function(&mut self, regex: &str) {
        // TODO friendlier error handling
        let r = Regex::new(regex).expect("Invalid regex");

        let exclude_string_table: HashSet<_> = self
            .shared
            .string_array
            .iter()
            .positions(|string| r.is_match(string))
            .collect();

        for thread in &mut self.threads {
            thread.exclude_function(&exclude_string_table);
        }
    }

    pub fn total_samples(&self) -> Vec<usize> {
        self.threads
            .iter()
            .map(|thread| thread.samples.total_weight())
            .collect()
    }
}

impl SampleTable {
    fn total_weight(&self) -> usize {
        match &self.weight {
            Some(weights) => weights.iter().sum(),
            // Weights is assumed to be 1
            None => self.stack.len(),
        }
    }
}

type IndexToStackTable = usize;
type IndexToFrameTable = usize;
type IndexToFuncTable = usize;
type IndexToStringTable = usize;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
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
    weight: Option<Vec<usize>>,
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
