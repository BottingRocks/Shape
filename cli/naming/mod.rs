//! `shape naming`: function naming, collector identification and inspection.
//!
//! `inventory` lists every function and the whole-program role graph; `plan`
//! prints the unified naming plan with its proofs and refusals as JSON;
//! `threads` and `collectors` provide focused edits and preserve readable
//! names; `corpus` runs the whole naming chain per seed. No command executes
//! recovered JavaScript.

mod corpus;
mod threads;

use std::path::Path;
use std::process::ExitCode;

use shape::naming::semantic_names::plan_semantic_names;

use super::{parse_show, read_source, usage_error};

const NAMING_USAGE: &str = "\
usage: shape naming <threads|collectors|inventory|plan|corpus> ... [options]

commands:
  threads <readable.js> [--map-out <file>] [--rename --out <file>] [--labels <file>]
      name remaining generated THREAD functions with small static evaluators.
      Keep readable names; remove only proven unreferenced empty scoped stubs.
      Reference mode: threads <unpruned-readable.js> --original <trace.js> --save-labels <new.json>

  collectors <program.js> [--map-out <file>] [--rename --out <file>]
      report original function identities and proven collector routing;
      optionally rename collector bindings only, preserving other source bytes.
      This focused path skips general naming and signal grammar extraction.

  inventory <program.js> [--json]
      list every function (fn@start..end ids, kinds, call edges) and the MAIN
      bindings, with the whole-program role graph. --json prints the function
      index and role graph reports.

  plan <program.js>
      print the unified naming plan as JSON: each proposal's entity, editable
      binding, role, evidence and confidence, plus every ambiguity and refusal
      with its reason and spans. A name asserts a structurally proven role,
      not complete behavior understanding.

  corpus <source>... [options]
      run the whole naming chain for every seed (trace a raw capture into a
      linked program, plan, rename, optionally annotate), compare a versioned
      fingerprint baseline, and compare seeds or vendors. See
      `shape naming corpus --help` and docs/naming/corpus.md.

No command executes the recovered JavaScript.";

const INVENTORY_USAGE: &str = "usage: shape naming inventory <program.js> [--json]";
const PLAN_USAGE: &str = "usage: shape naming plan <program.js>";

/// Run `shape naming` from the root command's remaining arguments.
pub fn run(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(command) = args.next() else {
        eprintln!("shape naming: missing command\n{NAMING_USAGE}");
        return ExitCode::from(2);
    };
    match command.as_str() {
        "-h" | "--help" => {
            println!("{NAMING_USAGE}");
            ExitCode::SUCCESS
        }
        "collectors" => threads::run(args.collect()),
        "threads" => threads::run_threads(args.collect()),
        "inventory" => run_inventory(args.collect()),
        "plan" => run_plan(args.collect()),
        "annotate" | "rename" => {
            eprintln!("shape naming: command {command} was removed; use naming threads <input.js> --rename --out <new.js> [--map-out <map.json>]");
            ExitCode::from(2)
        }
        "corpus" => corpus::run_corpus(args.collect()),
        other => {
            eprintln!("shape naming: unknown command {other}\n{NAMING_USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run_inventory(args: Vec<String>) -> ExitCode {
    let parsed = match parse_show(args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => {
            println!("{INVENTORY_USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => return usage_error("shape naming inventory", &error, INVENTORY_USAGE),
    };
    let source = match read_source(Path::new(&parsed.input)) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("shape naming inventory: {error}");
            return ExitCode::from(1);
        }
    };
    let index = match shape::naming::function_index::extract_function_index(&source.text) {
        Ok(index) => index,
        Err(error) => {
            eprintln!("shape naming inventory: {error}");
            return ExitCode::from(1);
        }
    };
    let graph =
        match shape::naming::role_graph::extract_program_role_graph(&source.text) {
            Ok(graph) => graph,
            Err(error) => {
                eprintln!("shape naming inventory: {error}");
                return ExitCode::from(1);
            }
        };
    let rendered = if parsed.json {
        match serde_json::to_string_pretty(&serde_json::json!({
            "function_index": index,
            "role_graph": graph,
        })) {
            Ok(json) => json,
            Err(error) => {
                eprintln!("shape naming inventory: cannot serialize report: {error}");
                return ExitCode::from(1);
            }
        }
    } else {
        inventory_summary(&index, &graph)
    };
    match crate::cli::write_stdout_line(&rendered) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("shape naming inventory: failed to write stdout: {error}");
            ExitCode::from(1)
        }
    }
}

fn run_plan(args: Vec<String>) -> ExitCode {
    let parsed = match parse_show(args) {
        Ok(Some(parsed)) if !parsed.json => parsed,
        Ok(Some(_)) => {
            return usage_error("shape naming plan", "unknown option --json", PLAN_USAGE)
        }
        Ok(None) => {
            println!("{PLAN_USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => return usage_error("shape naming plan", &error, PLAN_USAGE),
    };
    let source = match read_source(Path::new(&parsed.input)) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("shape naming plan: {error}");
            return ExitCode::from(1);
        }
    };
    let plan = match plan_semantic_names(&source.text) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("shape naming plan: {error}");
            return ExitCode::from(1);
        }
    };
    let rendered = match serde_json::to_string_pretty(&plan) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("shape naming plan: cannot serialize the plan: {error}");
            return ExitCode::from(1);
        }
    };
    match crate::cli::write_stdout_line(&rendered) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("shape naming plan: failed to write stdout: {error}");
            ExitCode::from(1)
        }
    }
}

fn inventory_summary(
    index: &shape::naming::function_index::FunctionIndex,
    graph: &shape::naming::role_graph::ProgramRoleGraph,
) -> String {
    let coverage = &index.coverage;
    let mut out = format!(
        "source sha256={} bytes={}\n",
        index.source.sha256, index.source.bytes
    );
    out.push_str(&format!(
        "functions: {} (named={} anonymous={} declarations={} expressions={} arrows={} methods={})\n",
        coverage.function_count,
        coverage.named_count,
        coverage.anonymous_count,
        coverage.declaration_count,
        coverage.expression_count,
        coverage.arrow_count,
        coverage.method_count,
    ));
    out.push_str(&format!(
        "main: {} bindings={} call_edges={} unresolved_calls={}\n",
        index
            .main
            .as_ref()
            .map(|main| main.function_id.as_str())
            .unwrap_or("<none>"),
        coverage.main_binding_count,
        coverage.call_edge_count,
        coverage.unresolved_call_count,
    ));
    out.push_str(&format!(
        "role graph: nodes={} functions={} main_bindings={} unresolved={}\n",
        graph.nodes.len(),
        graph.coverage.function_node_count,
        graph.coverage.main_variable_node_count,
        graph.unresolved.len(),
    ));
    for function in &index.functions {
        out.push_str(&format!(
            "  {} {:?} {}{}\n",
            function.id,
            function.kind,
            function
                .label
                .as_deref()
                .or(function.binding_label.as_deref())
                .unwrap_or("<anonymous>"),
            function
                .role
                .as_deref()
                .map(|role| format!(" role={role}"))
                .unwrap_or_default(),
        ));
    }
    out
}
