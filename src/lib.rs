use std::{
    collections::{BTreeMap, HashSet},
    marker::PhantomData,
    ops::{Index, IndexMut},
};

use itertools::{self, Itertools};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

impl StackTable {
    /// Squash out excluded prefixes.
    fn squash_excluded_parents(&mut self, id: StackIdx, excluded: &HashSet<StackIdx>) {
        if let Some(prefix_id) = self.prefix[id] {
            if excluded.iter().contains(&prefix_id) {
                self.prefix[id] = self.prefix[prefix_id];
                self.squash_excluded_parents(id, excluded);
            }
        }
    }

    /// Rewrite such that non-excluded frames do not point at excluded frames anymore.
    /// Excluded frames themselves stay included to not mess up the indexing and they act as a fast way to
    fn exclude(&mut self, excluded: &HashSet<StackIdx>) {
        for i in 0..self.length {
            self.squash_excluded_parents(
                // TODO better way to do this
                Id {
                    idx: i,
                    _marker: PhantomData,
                },
                excluded,
            );
        }
    }
}

impl Thread {
    // Isn't stack structure more what we need?
    // We need edges
    // For now frame is mostly an indirection
    // Building the path this way is fine and then grouping it by 2
    fn path(&self, id: StackIdx) -> Vec<FuncIdx> {
        let stack = &self.stack_table;
        let frame = &self.frame_table;
        let mut p = match stack.prefix[id] {
            Some(prefix_id) => self.path(prefix_id),
            None => Vec::new(),
        };
        let frame_id = stack.frame[id];
        p.push(frame.func[frame_id]);
        p
    }

    fn paths(&self) -> Vec<Vec<FuncIdx>> {
        let stack = &self.stack_table;
        let mut p = Vec::with_capacity(stack.length);
        for i in 0..stack.length {
            // TODO better
            p.push(self.path(Id::new(i)));
        }
        p
    }

    // fn stack_to_func(&self, id: IndexToStackTable) -> Vec<IndexToFuncTable> {}

    // TODO tree paths?

    fn exclude_function(&mut self, exclude_string_table: &HashSet<Id<IndexStringTable>>) {
        let exclude_func_table: HashSet<FuncIdx> = self
            .func_table
            .name
            .inner
            .iter()
            .positions(|id| exclude_string_table.contains(id))
            .map(Id::new)
            .collect();

        let exclude_frame_table: HashSet<FrameIdx> = self
            .frame_table
            .func
            .inner
            .iter()
            .positions(|id| exclude_func_table.contains(id))
            .map(Id::new)
            .collect();

        let exclude_stack_table: HashSet<StackIdx> = self
            .stack_table
            .frame
            .inner
            .iter()
            .positions(|id| exclude_frame_table.contains(id))
            .map(Id::new)
            .collect();

        self.stack_table.exclude(&exclude_stack_table);

        self.reattribute_samples(&exclude_stack_table);
    }

    /// Samples that point to an excluded stack entry needs to be reassigned to its parent.
    ///
    /// Run this after stack.exclude to prevent reassing the sample to another excluded stack entry
    fn reattribute_samples(&mut self, excluded: &HashSet<StackIdx>) {
        for s in &mut self.samples.stack.inner {
            if excluded.iter().contains(s) {
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

        let exclude_string_table: HashSet<StringIdx> = self
            .shared
            .string_array
            .inner
            .iter()
            .positions(|string| r.is_match(string))
            .map(Id::new)
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

//TODO remove inners

impl SampleTable {
    fn total_weight(&self) -> usize {
        match &self.weight {
            Some(weights) => weights.inner.iter().sum(),
            // Weights is assumed to be 1
            None => self.stack.inner.len(),
        }
    }
}

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

// TODO make this a macro?
// Better naming
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy, Hash)]
struct IndexSampleTable;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy, Hash)]
struct IndexStackTable;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy, Hash)]
struct IndexFrameTable;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy, Hash)]
struct IndexFuncTable;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy, Hash)]
struct IndexStringTable;

// type SampleIdx = Id<IndexSampleTable>;
type StackIdx = Id<IndexStackTable>;
type FrameIdx = Id<IndexFrameTable>;
type FuncIdx = Id<IndexFuncTable>;
type StringIdx = Id<IndexStringTable>;

type SampleVec<T> = TypedVec<IndexSampleTable, T>;
type StackVec<T> = TypedVec<IndexStackTable, T>;
type FrameVec<T> = TypedVec<IndexFrameTable, T>;
type FuncVec<T> = TypedVec<IndexFuncTable, T>;
type StringVec<T> = TypedVec<IndexStringTable, T>;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SampleTable {
    stack: SampleVec<StackIdx>,
    weight: Option<SampleVec<usize>>,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StackTable {
    prefix: StackVec<Option<StackIdx>>,
    frame: StackVec<FrameIdx>,
    length: usize,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct FrameTable {
    func: FrameVec<FuncIdx>,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FuncTable {
    name: FuncVec<StringIdx>,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProfileSharedData {
    #[serde(rename = "stringArray")]
    string_array: StringVec<String>,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

// From I to T
// TODO find name of what kind of structure this is.
// two typed
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TypedVec<I, T> {
    inner: Vec<T>,
    _marker: PhantomData<I>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy, Eq, Hash)]
struct Id<I> {
    idx: usize,
    _marker: PhantomData<I>,
}

impl<I> Id<I> {
    fn new(id: usize) -> Id<I> {
        Id {
            idx: id,
            _marker: PhantomData,
        }
    }
}

impl<I, T> Index<Id<I>> for TypedVec<I, T> {
    type Output = T;

    fn index(&self, index: Id<I>) -> &Self::Output {
        &self.inner[index.idx]
    }
}

impl<I, T> IndexMut<Id<I>> for TypedVec<I, T> {
    fn index_mut(&mut self, index: Id<I>) -> &mut Self::Output {
        &mut self.inner[index.idx]
    }
}
