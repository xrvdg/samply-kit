# Samply Kit (**Alpha Release**)

A toolkit for analyzing and manipulating [Samply](https://github.com/mstange/samply)/[Firefox Profile](https://profiler.firefox.com/) profiling data. This project provides utilities for filtering, aggregating sample counts per function, and in the future visualizing CPU profiling.

## Overview

Samply Kit is a Rust-based collection of tools designed to work with profiling data captured by Samply, a command-line sampling profiler for macOS and Linux. This toolkit allows you to:

- Filter out specific functions from profiling data using regex patterns
- Analyze profiling statistics (self time, cumulative time)
- Perform reverse lookup to find all traces containing a specific function

## Usage

### `preprocess`
Filter out unwanted frames from a profile.

**Usage:**
```bash
cargo run --bin preprocess -- <input.json> <output.json> <regex>
```

**Example:**
```bash
# Remove all frames matching "rayon" from the profile
cargo run --bin preprocess -- profile.json filtered_profile.json "rayon"
```

This tool reads a profile, excludes all functions matching the given regex pattern, and writes the filtered profile to the output file.

### `pprof`
Analyze profiling data by aggregating sample counts per function (pprof-style sample attribution). This is useful for finding functions that are called all over the place but in regular flamegraph show up as a small contributor even though in aggregate they are a large contributor.

**Usage:**
```bash
# Show statistics
cargo run --bin pprof -- <profile.json> stat

# Lookup traces containing a specific function
cargo run --bin pprof -- <profile.json> lookup <function_id>
```

**Commands:**

- `stat`: Display top 25 functions by self-time and cumulative time for the main thread and first worker thread. With Rayon
- `lookup <id>`: Show all call traces containing the function sorted by sample count with the given function id. The function ID is the id listed by `stat`.

**Example:**`
```bash
# View profiling statistics
cargo run --bin pprof -- profile.json stat

# Find all traces containing function with ID 42
cargo run --bin pprof -- profile.json lookup 42
```
