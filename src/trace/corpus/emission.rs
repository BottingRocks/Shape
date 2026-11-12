use super::*;

pub fn check_emission(
    result: &PrefixTraceReport,
    parameters: &[String],
    created: &[Function],
    emitting: &Emitting<'_>,
) -> (Vec<String>, Vec<String>, Option<String>) {
    let mut undeclared = Vec::new();
    let mut problems = Vec::new();
    let mut external_parameters = parameters.to_vec();
    external_parameters.extend(result.trace.external_inputs.iter().cloned());
    let options = EmitOptions {
        external_parameters,
        simplify_bindings: true,
        ..EmitOptions::default()
    };
    let emitted = match emit_javascript(&result.trace, &options) {
        Ok(emitted) => emitted,
        Err(error) => return (undeclared, vec![format!("cannot emit: {error}")], None),
    };
    let mut check = |stage: &str, code: &str, undeclared: &mut Vec<String>| {
        match crate::tracing::scope::undeclared_generated_names(code) {
            Ok(names) => {
                undeclared.extend(names.into_iter().map(|name| format!("{name} ({stage})")))
            }
            Err(error) => problems.push(format!("{stage}: {error}")),
        }
        let emitted = emitted_threads(code);
        for function in created {
            if !emitted.contains(function) {
                problems.push(format!("{stage}: {} is not created", function.key()));
            }
        }
    };
    check("emitted", &emitted.code, &mut undeclared);
    let traced = emitted.code.clone();
    if !emitting.cleanup {
        return (undeclared, problems, Some(traced));
    }
    let cleaned = emitting
        .artifact
        .fold_emitted_vm_strings(emitted)
        .map_err(|error| format!("string folding: {error}"))
        .and_then(|folded| {
            simplify_emitted_property_writes(&folded.emitted)
                .map_err(|error| format!("property writes: {error}"))
        })
        .and_then(|emitted| {
            simplify_cleaned_emitted_property_writes(&emitted)
                .map_err(|error| format!("array constructions: {error}"))
        });
    match cleaned {
        Ok(cleaned) => check("cleaned", &cleaned.code, &mut undeclared),
        Err(error) => problems.push(format!("cleanup failed: {error}")),
    }
    (undeclared, problems, Some(traced))
}

/// The VM functions `code` creates: every `vmCreateThread(d, pc, state)`
/// with numeric literal arguments (in any spelling, such as `98e3`), and
/// every function it posts to the VM Worker, `vmPostToWorker(state, pc,
/// argument, d, callback)` (see `created_functions`).
pub fn emitted_threads(code: &str) -> BTreeSet<Function> {
    let number = |part: &str| {
        let number: f64 = part.trim().parse().ok()?;
        (number >= 0.0 && number.fract() == 0.0).then_some(number as usize)
    };
    let mut threads = BTreeSet::new();
    let name = crate::operations::templates::POST_TO_WORKER;
    for (start, _) in code.match_indices(&format!("{name}(")) {
        let arguments = call_arguments(&code[start + name.len() + 1..]);
        if let [state, pc, _, descriptor, ..] = arguments[..] {
            if let (Some(state), Some(pc), Some(descriptor)) =
                (number(state), number(pc), number(descriptor))
            {
                threads.insert(Function {
                    descriptor: Some(descriptor),
                    pc,
                    state,
                });
            }
        }
    }
    for (start, _) in code.match_indices("vmCreateThread(") {
        let rest = &code[start + "vmCreateThread(".len()..];
        // The three numbers, then the closure's cells (`{ … }`) when it
        // captures any.
        let Some(end) = rest.find([')', '{']) else {
            continue;
        };
        let numbers: Vec<Option<usize>> = rest[..end]
            .split(',')
            .filter(|part| !part.trim().is_empty())
            .map(number)
            .collect();
        if let [Some(descriptor), Some(pc), Some(state)] = numbers[..] {
            threads.insert(Function {
                descriptor: Some(descriptor),
                pc,
                state,
            });
        }
    }
    threads
}

/// The arguments of the call whose argument list starts `rest`, split at
/// its top-level commas: an argument may hold calls, arrays, objects and
/// strings of its own.
fn call_arguments(rest: &str) -> Vec<&str> {
    let mut arguments = Vec::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut start = 0;
    for (index, character) in rest.char_indices() {
        if let Some(open) = quote {
            match character {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                _ if character == open => quote = None,
                _ => {}
            }
            continue;
        }
        match character {
            '"' | '\'' | '`' => quote = Some(character),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            ')' => {
                arguments.push(&rest[start..index]);
                return arguments;
            }
            ',' if depth == 0 => {
                arguments.push(&rest[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    Vec::new()
}
