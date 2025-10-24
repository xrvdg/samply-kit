use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};

#[derive(Debug, Deserialize, Serialize)]
struct Profile {
    meta: serde_json::Value,
    libs: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pages: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    counters: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profilerOverhead: Option<Vec<serde_json::Value>>,
    shared: SharedData,
    threads: Vec<Thread>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profilingLog: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profileGatheringLog: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
struct SharedData {
    stringArray: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sources: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Thread {
    #[serde(flatten)]
    other_fields: HashMap<String, serde_json::Value>,
    stackTable: StackTable,
    frameTable: FrameTable,
    funcTable: FuncTable,
    samples: SamplesTable,
}

#[derive(Debug, Deserialize, Serialize)]
struct StackTable {
    frame: Vec<usize>,
    prefix: Vec<Option<usize>>,
    length: usize,
}

#[derive(Debug, Deserialize, Serialize)]
struct FrameTable {
    #[serde(flatten)]
    fields: HashMap<String, serde_json::Value>,
    func: Vec<usize>,
    length: usize,
}

#[derive(Debug, Deserialize, Serialize)]
struct FuncTable {
    #[serde(flatten)]
    fields: HashMap<String, serde_json::Value>,
    name: Vec<usize>,
    length: usize,
}

#[derive(Debug, Deserialize, Serialize)]
struct SamplesTable {
    #[serde(flatten)]
    fields: HashMap<String, serde_json::Value>,
    stack: Vec<Option<usize>>,
    length: usize,
}

impl Profile {
    fn flatten_rayon_frames(&mut self) {
        let string_array = &self.shared.stringArray;

        for thread in &mut self.threads {
            // Find frames that reference rayon functions
            let rayon_frames = thread.find_rayon_frames(string_array);

            if rayon_frames.is_empty() {
                continue;
            }

            println!(
                "Thread '{}': Found {} rayon frames to flatten",
                thread
                    .other_fields
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown"),
                rayon_frames.len()
            );

            // Build a mapping from old stack indices to new stack indices
            thread.flatten_stacks(&rayon_frames);
        }
    }
}

impl Thread {
    fn find_rayon_frames(&self, string_array: &[String]) -> Vec<usize> {
        let mut rayon_frames = Vec::new();

        for (frame_idx, &func_idx) in self.frameTable.func.iter().enumerate() {
            if func_idx < self.funcTable.name.len() {
                let name_idx = self.funcTable.name[func_idx];
                if name_idx < string_array.len() {
                    let name = &string_array[name_idx];
                    if name.contains("rayon") {
                        rayon_frames.push(frame_idx);
                        println!("  Frame {}: {}", frame_idx, name);
                    }
                }
            }
        }

        rayon_frames
    }

    fn flatten_stacks(&mut self, rayon_frames: &[usize]) {
        // Build a set for quick lookup
        let rayon_frame_set: std::collections::HashSet<usize> =
            rayon_frames.iter().copied().collect();

        // Helper function to find the non-rayon ancestor of a stack
        let find_non_rayon_ancestor = |mut stack_idx: usize| -> Option<usize> {
            loop {
                let frame_idx = self.stackTable.frame[stack_idx];
                if !rayon_frame_set.contains(&frame_idx) {
                    return Some(stack_idx);
                }
                match self.stackTable.prefix[stack_idx] {
                    Some(prefix) => stack_idx = prefix,
                    None => return None, // Root stack is a rayon frame
                }
            }
        };

        // Create a new stack table without rayon frames
        let mut old_to_new: HashMap<usize, usize> = HashMap::new();
        let mut new_frames = Vec::new();
        let mut new_prefixes_old_idx: Vec<Option<usize>> = Vec::new();

        // First pass: copy non-rayon stacks and track mapping
        for old_idx in 0..self.stackTable.length {
            let frame_idx = self.stackTable.frame[old_idx];

            if !rayon_frame_set.contains(&frame_idx) {
                let new_idx = new_frames.len();
                old_to_new.insert(old_idx, new_idx);
                new_frames.push(frame_idx);
                new_prefixes_old_idx.push(self.stackTable.prefix[old_idx]);
            }
        }

        // Second pass: fix up prefixes, skipping over rayon frames
        let mut new_prefixes = Vec::new();
        for old_prefix_opt in new_prefixes_old_idx {
            let new_prefix = match old_prefix_opt {
                Some(old_prefix_idx) => {
                    // Find the first non-rayon ancestor
                    find_non_rayon_ancestor(old_prefix_idx)
                        .and_then(|ancestor_old_idx| old_to_new.get(&ancestor_old_idx).copied())
                }
                None => None,
            };
            new_prefixes.push(new_prefix);
        }

        // Update samples
        for sample_stack in &mut self.samples.stack {
            *sample_stack = sample_stack.and_then(|old_idx| {
                find_non_rayon_ancestor(old_idx)
                    .and_then(|ancestor_old_idx| old_to_new.get(&ancestor_old_idx).copied())
            });
        }

        // Update the stack table
        let old_length = self.stackTable.length;
        let new_length = new_prefixes.len();
        self.stackTable.frame = new_frames;
        self.stackTable.prefix = new_prefixes;
        self.stackTable.length = new_length;

        println!("  Reduced stacks from {} to {}", old_length, new_length);
    }
}

fn main() -> Result<()> {
    println!("Reading profile.json...");
    let input_file = File::open("profile.json").context("Failed to open profile.json")?;
    let reader = BufReader::new(input_file);

    println!("Parsing JSON...");
    let mut profile: Profile =
        serde_json::from_reader(reader).context("Failed to parse profile.json")?;

    println!("Flattening rayon frames...");
    profile.flatten_rayon_frames();

    println!("Writing profile_flattened.json...");
    let output_file =
        File::create("profile_flattened.json").context("Failed to create output file")?;
    let writer = BufWriter::new(output_file);

    serde_json::to_writer(writer, &profile).context("Failed to write output JSON")?;

    println!("Done! Output written to profile_flattened.json");
    Ok(())
}
