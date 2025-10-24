use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Write};

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

#[derive(Debug)]
struct CallGraphNode {
    name: String,
    self_count: usize,
    total_count: usize,
}

#[derive(Debug)]
struct CallGraphEdge {
    from_func: String,
    to_func: String,
    count: usize,
}

struct CallGraph {
    nodes: HashMap<String, CallGraphNode>,
    edges: Vec<CallGraphEdge>,
}

impl Profile {
    fn build_call_graph(&self) -> Result<CallGraph> {
        let string_array = &self.shared.stringArray;
        let mut nodes: HashMap<String, CallGraphNode> = HashMap::new();
        let mut edges: HashMap<(String, String), usize> = HashMap::new();

        println!("Building call graph...");

        for thread in &self.threads {
            let thread_name = thread
                .other_fields
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            println!("Processing thread: {}", thread_name);

            // Process each sample
            for sample_stack_opt in &thread.samples.stack {
                if let Some(stack_idx) = sample_stack_opt {
                    if *stack_idx < thread.stackTable.length {
                        // Walk the stack from leaf to root
                        let mut current_stack_idx = Some(*stack_idx);
                        let mut stack_func_names = Vec::new();

                        // Collect all function names in this stack
                        while let Some(stack_idx) = current_stack_idx {
                            if stack_idx >= thread.stackTable.length {
                                break;
                            }

                            let frame_idx = thread.stackTable.frame[stack_idx];
                            if frame_idx < thread.frameTable.length {
                                let func_idx = thread.frameTable.func[frame_idx];
                                if func_idx < thread.funcTable.length {
                                    let name_idx = thread.funcTable.name[func_idx];
                                    if name_idx < string_array.len() {
                                        let func_name = string_array[name_idx].clone();
                                        stack_func_names.push(func_name);
                                    }
                                }
                            }

                            current_stack_idx = thread.stackTable.prefix[stack_idx];
                        }

                        // Update counts for all functions in the stack
                        // Use a HashSet to track which functions we've already counted for this sample
                        let mut functions_in_sample = std::collections::HashSet::new();

                        for (i, func_name) in stack_func_names.iter().enumerate() {
                            let node =
                                nodes
                                    .entry(func_name.clone())
                                    .or_insert_with(|| CallGraphNode {
                                        name: func_name.clone(),
                                        self_count: 0,
                                        total_count: 0,
                                    });

                            // Only count each function once per sample (deduplicate recursive calls)
                            if functions_in_sample.insert(func_name.clone()) {
                                node.total_count += 1;
                            }

                            // Self count only for the leaf (innermost function)
                            if i == 0 {
                                node.self_count += 1;
                            }

                            // Record edge from caller to callee
                            if i > 0 {
                                let caller_func = &stack_func_names[i];
                                let callee_func = &stack_func_names[i - 1];
                                *edges
                                    .entry((caller_func.clone(), callee_func.clone()))
                                    .or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
        }

        // Convert edges map to vec
        let edges_vec: Vec<CallGraphEdge> = edges
            .into_iter()
            .map(|((from, to), count)| CallGraphEdge {
                from_func: from,
                to_func: to,
                count,
            })
            .collect();

        println!(
            "Call graph built: {} nodes, {} edges",
            nodes.len(),
            edges_vec.len()
        );

        Ok(CallGraph {
            nodes,
            edges: edges_vec,
        })
    }

    fn generate_graphviz(&self, output_path: &str, min_samples: usize) -> Result<()> {
        let call_graph = self.build_call_graph()?;

        let mut output = File::create(output_path)
            .context(format!("Failed to create output file: {}", output_path))?;

        writeln!(output, "digraph profile {{")?;
        writeln!(output, "  node [shape=box, style=filled];")?;
        writeln!(output, "  rankdir=TB;")?;
        writeln!(output)?;

        // Find max counts for scaling
        let max_total = call_graph
            .nodes
            .values()
            .map(|n| n.total_count)
            .max()
            .unwrap_or(1);

        let max_self = call_graph
            .nodes
            .values()
            .map(|n| n.self_count)
            .max()
            .unwrap_or(1);

        // Helper to create a sanitized node ID from function name
        let sanitize_node_id = |name: &str| -> String {
            name.chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        };

        // Write nodes
        writeln!(output, "  // Nodes")?;
        for (func_name, node) in &call_graph.nodes {
            if node.total_count >= min_samples {
                // Calculate color intensity based on self time
                let intensity = if max_self > 0 {
                    (node.self_count as f64 / max_self as f64 * 0.8 + 0.2).min(1.0)
                } else {
                    0.2
                };

                // Color scale: light red to dark red based on self time
                let red = 1.0;
                let green = 1.0 - intensity;
                let blue = 1.0 - intensity;
                let color = format!(
                    "#{:02x}{:02x}{:02x}",
                    (red * 255.0) as u8,
                    (green * 255.0) as u8,
                    (blue * 255.0) as u8
                );

                // Escape special characters in function name
                let escaped_name = node
                    .name
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n");

                // Calculate percentages
                let self_pct = (node.self_count as f64 / max_total as f64) * 100.0;
                let total_pct = (node.total_count as f64 / max_total as f64) * 100.0;

                let node_id = sanitize_node_id(func_name);
                writeln!(
                    output,
                    "  \"{}\" [label=\"{}\\nself: {} ({:.1}%)\\ntotal: {} ({:.1}%)\", fillcolor=\"{}\"];",
                    node_id,
                    escaped_name,
                    node.self_count,
                    self_pct,
                    node.total_count,
                    total_pct,
                    color
                )?;
            }
        }

        writeln!(output)?;
        writeln!(output, "  // Edges")?;

        // Write edges
        let max_edge_weight = call_graph.edges.iter().map(|e| e.count).max().unwrap_or(1);

        for edge in &call_graph.edges {
            // Only include edges where both nodes are significant
            if let (Some(from_node), Some(to_node)) = (
                call_graph.nodes.get(&edge.from_func),
                call_graph.nodes.get(&edge.to_func),
            ) {
                if from_node.total_count >= min_samples
                    && to_node.total_count >= min_samples
                    && edge.count >= min_samples
                {
                    // Edge weight for visualization
                    let weight = (edge.count as f64 / max_edge_weight as f64 * 4.0 + 0.5).max(0.5);
                    let penwidth = weight;

                    let edge_pct = (edge.count as f64 / max_total as f64) * 100.0;

                    let from_id = sanitize_node_id(&edge.from_func);
                    let to_id = sanitize_node_id(&edge.to_func);

                    writeln!(
                        output,
                        "  \"{}\" -> \"{}\" [label=\"{} ({:.1}%)\", penwidth={:.2}];",
                        from_id, to_id, edge.count, edge_pct, penwidth
                    )?;
                }
            }
        }

        writeln!(output, "}}")?;

        println!("Graphviz DOT file written to: {}", output_path);
        Ok(())
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let input_file = if args.len() > 1 {
        &args[1]
    } else {
        "profile_flattened.json"
    };

    let output_file = if args.len() > 2 {
        &args[2]
    } else {
        "profile.dot"
    };

    let min_samples = if args.len() > 3 {
        args[3].parse::<usize>().unwrap_or(10)
    } else {
        10
    };

    println!("Reading profile from: {}", input_file);
    println!("Minimum samples threshold: {}", min_samples);

    let file =
        File::open(input_file).context(format!("Failed to open input file: {}", input_file))?;
    let reader = BufReader::new(file);

    println!("Parsing JSON...");
    let profile: Profile =
        serde_json::from_reader(reader).context("Failed to parse profile JSON")?;

    println!("Generating graphviz visualization...");
    profile.generate_graphviz(output_file, min_samples)?;

    println!("\nDone!");
    println!("To render the graph, run:");
    println!("  dot -Tpdf {} -o profile.pdf", output_file);
    println!("  dot -Tpng {} -o profile.png", output_file);
    println!("  dot -Tsvg {} -o profile.svg", output_file);

    Ok(())
}
