use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
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
    pub fn total_samples(&self) -> usize {
        self.samples.total_weight()
    }
}

impl ProfileSharedData {
    pub fn path(&self, id: StackIdx) -> Vec<StringIdx> {
        let stack = &self.stack_table;
        let frame = &self.frame_table;
        let mut p = match stack.prefix[id] {
            Some(prefix_id) => self.path(prefix_id),
            None => Vec::new(),
        };
        let frame_id = stack.frame[id];
        p.push(self.func_table.name[frame.func[frame_id]]);
        p
    }

    pub fn sample_count(
        &self,
        samples: &SampleTable,
    ) -> (
        HashMap<StringIdx, usize>, /*own */
        HashMap<StringIdx, usize>, /*cumulative */
    ) {
        let mut cumulative = HashMap::new();
        let mut own = HashMap::new();
        for (id, stack) in samples.stack.inner.iter().enumerate() {
            // Shouldn't have to check the existence of weight everytime
            let add = match &samples.weight {
                Some(weight_vec) => weight_vec[Id::new(id)],
                None => 1,
            };
            let path: Vec<_> = self.path(*stack);

            if let Some(last) = path.last().copied() {
                *own.entry(last).or_insert(0) += add
            }
            // Only count a function once in a sample. Recursion should not lead to multiple counts
            let path: HashSet<StringIdx> = HashSet::from_iter(path);
            for func in path {
                *cumulative.entry(func).or_insert(0) += add
            }
        }
        (own, cumulative)
    }

    pub fn excluded_stacks(&self, exclude_string_table: &HashSet<StringIdx>) -> HashSet<StackIdx> {
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

        self.stack_table
            .frame
            .inner
            .iter()
            .positions(|id| exclude_frame_table.contains(id))
            .map(Id::new)
            .collect()
    }
    /// Samples that point to an excluded stack entry needs to be reassigned to its parent.
    ///
    /// Run this after stack.exclude to prevent reassigning the sample to another excluded stack entry
    pub fn reattribute_samples(&self, samples: &mut SampleTable, excluded: &HashSet<StackIdx>) {
        for s in &mut samples.stack.inner {
            if excluded.iter().contains(s) {
                if let Some(prefix) = self.stack_table.prefix[*s] {
                    *s = prefix;
                }
            }
        }
    }
}

impl Profile {
    // Search across all threads
    pub fn reverse_search(&self, string_idx: StringIdx) -> HashMap<Vec<StringIdx>, usize> {
        // Add weights later
        let mut traces = HashMap::new();
        for thread in &self.threads {
            for (id, stack) in thread.samples.stack.inner.iter().enumerate() {
                let weight = match &thread.samples.weight {
                    Some(weight_vec) => weight_vec[Id::new(id)],
                    None => 1,
                };
                let path = self.shared.path(*stack);
                if path.contains(&string_idx) {
                    *traces.entry(path).or_insert(0) += weight;
                }
            }
        }

        traces
    }

    pub fn exclude_function(&mut self, regex: &str) {
        let r = Regex::new(regex).expect("Invalid regex");

        let excluded = {
            let shared = &self.shared;
            let exclude_string_table: HashSet<StringIdx> = shared
                .string_array
                .inner
                .iter()
                .positions(|string| r.is_match(string))
                .map(Id::new)
                .collect();
            shared.excluded_stacks(&exclude_string_table)
        };

        self.shared.stack_table.exclude(&excluded);
        for thread in &mut self.threads {
            self.shared
                .reattribute_samples(&mut thread.samples, &excluded);
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
    pub fn total_weight(&self) -> usize {
        match &self.weight {
            Some(weights) => weights.inner.iter().sum(),
            // Weights is assumed to be 1
            None => self.stack.inner.len(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub threads: Vec<Thread>,
    pub shared: ProfileSharedData,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Thread {
    pub samples: SampleTable,
    pub name: String,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

// TODO make this a macro?
// Better naming
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy, Hash)]
pub struct IndexSampleTable;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy, Hash)]
pub struct IndexStackTable;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy, Hash)]
pub struct IndexFrameTable;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy, Hash)]
pub struct IndexFuncTable;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy, Hash)]
pub struct IndexStringTable;

// type SampleIdx = Id<IndexSampleTable>;
pub type StackIdx = Id<IndexStackTable>;
pub type FrameIdx = Id<IndexFrameTable>;
pub type FuncIdx = Id<IndexFuncTable>;
pub type StringIdx = Id<IndexStringTable>;

pub type SampleVec<T> = TypedVec<IndexSampleTable, T>;
pub type StackVec<T> = TypedVec<IndexStackTable, T>;
pub type FrameVec<T> = TypedVec<IndexFrameTable, T>;
pub type FuncVec<T> = TypedVec<IndexFuncTable, T>;
pub type StringVec<T> = TypedVec<IndexStringTable, T>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SampleTable {
    pub stack: SampleVec<StackIdx>,
    pub weight: Option<SampleVec<usize>>,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StackTable {
    pub prefix: StackVec<Option<StackIdx>>,
    pub frame: StackVec<FrameIdx>,
    length: usize,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrameTable {
    pub func: FrameVec<FuncIdx>,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FuncTable {
    pub name: FuncVec<StringIdx>,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileSharedData {
    #[serde(rename = "stringArray")]
    pub string_array: StringVec<String>,
    #[serde(rename = "stackTable")]
    pub stack_table: StackTable,
    #[serde(rename = "frameTable")]
    pub frame_table: FrameTable,
    #[serde(rename = "funcTable")]
    pub func_table: FuncTable,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

// From I to T
// TODO find name of what kind of structure this is.
// two typed
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TypedVec<I, T> {
    pub inner: Vec<T>,
    _marker: PhantomData<I>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy, Eq, Hash)]
#[serde(transparent)]
pub struct Id<I> {
    idx: usize,
    _marker: PhantomData<I>,
}

impl<I> fmt::Display for Id<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({})", &self.idx)
    }
}

impl<I> Id<I> {
    pub fn new(id: usize) -> Id<I> {
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
