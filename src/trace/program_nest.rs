//! The traced program as one tree: each VM function nested where the
//! function that creates it does, the way it was written.
//!
//! Every function is emitted in program mode (see
//! [`crate::emit_program_function`]): it takes no parameters, reads its
//! inputs as `arguments[K]`, `arguments`, `this` and its own name
//! [`PROGRAM_SELF`], keeps memory slot K in the variable `$KEY_K`, reads the
//! slots it shares with its creator as the creator's `$KEY_K` through scope,
//! and writes each function it creates as an empty `function
//! $THREAD_<state>_<pc>() {}`. Nesting is then placement only: the created
//! function's code goes in that empty function, under its name. A function
//! that creates itself makes a fresh instance of the same code with the same
//! shared cells, so there is no second body to write: that creation refers to
//! the function itself. Nothing is renamed, so no binding can hide another;
//! [`check`] holds each function to the names it may read from outside.

use std::collections::{BTreeMap, BTreeSet};

use super::corpus::Function;
use crate::{ProgramFacts, ProgramThread, PROGRAM_SELF};

/// What the program has of one VM function.
pub enum Part<'a> {
    /// Its program-mode code and facts, or why it has none.
    Traced(&'a Result<(String, ProgramFacts), String>),
    /// Why it was not traced.
    Stopped(&'a str),
}

/// The name a function is placed under where it is created.
pub fn thread_name(function: Function) -> String {
    format!("$THREAD_{}_{}", function.state, function.pc)
}

/// Check one function's program-mode code, given the names it reads and
/// does not declare (`free`) and every name it declares (`declared`): the
/// only generated names it reads from outside are the `$KEY_K` of the slots
/// it shares, and it declares none of those, so each one is its creator's.
pub fn check(
    facts: &ProgramFacts,
    free: &[String],
    declared: &[String],
) -> Result<(), String> {
    let shared: BTreeSet<String> = facts
        .captured
        .iter()
        .map(|slot| format!("$KEY_{slot}"))
        .collect();
    if let Some(name) = free
        .iter()
        .find(|name| is_generated(name) && !shared.contains(*name))
    {
        return Err(format!("it reads {name}, which nothing gives it"));
    }
    if let Some(name) = declared.iter().find(|name| shared.contains(*name)) {
        return Err(format!(
            "it declares {name}, a slot it shares with its creator"
        ));
    }
    Ok(())
}

/// Whether the emitter could have made `name`, rather than it being a
/// kernel global.
fn is_generated(name: &str) -> bool {
    name.starts_with('$')
        || ["trace_value_", "vm_sub_", "vm_loop_"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// The program: the entry function with every function nested where it is
/// created, then a call of the entry. After it, what runs in the VM Worker
/// the program may start: each further root (a Worker's bootstrap, or a
/// function the program posts there) that no function creates, every one
/// with the functions it creates nested in it, and none called here.
pub fn build(
    roots: &[Function],
    parts: &BTreeMap<Function, Part<'_>>,
) -> Result<String, String> {
    let (&entry, others) = roots.split_first().ok_or("a program needs an entry")?;
    let name = thread_name(entry);
    let mut code =
        String::from("// Traced from a JavaScript VM capture by `shape trace program`.\n\n");
    place(
        entry,
        &name,
        &BTreeSet::new(),
        parts,
        &mut Vec::new(),
        &mut Out::new(&mut code, String::new()),
    )?;
    code.push_str(&format!("\n\n{name}();\n"));
    let mut placed = nested(entry, parts);
    // A function some function creates is nested in it: only those none
    // creates start a top-level function.
    let created: BTreeSet<Function> = parts
        .keys()
        .flat_map(|function| {
            let mut created = nested(*function, parts);
            created.remove(function);
            created
        })
        .collect();
    let rest = others
        .iter()
        .copied()
        .filter(|function| !placed.contains(function) && !created.contains(function))
        .collect::<Vec<_>>();
    let mut worker = false;
    for function in rest {
        if placed.contains(&function) {
            continue;
        }
        if !std::mem::replace(&mut worker, true) {
            code.push_str("\n// Run in the VM Worker the program starts: its bootstrap, and the\n// functions the program posts to it.\n");
        }
        code.push('\n');
        place(
            function,
            &thread_name(function),
            &BTreeSet::new(),
            parts,
            &mut Vec::new(),
            &mut Out::new(&mut code, String::new()),
        )?;
        code.push('\n');
        placed.extend(nested(function, parts));
    }
    Ok(code)
}

/// `function` and every function nested in it where it is created.
fn nested(function: Function, parts: &BTreeMap<Function, Part<'_>>) -> BTreeSet<Function> {
    let mut found = BTreeSet::from([function]);
    let mut pending = vec![function];
    while let Some(function) = pending.pop() {
        let Some(Part::Traced(Ok((_, facts)))) = parts.get(&function) else {
            continue;
        };
        for thread in &facts.threads {
            let child = created(thread);
            if found.insert(child) {
                pending.push(child);
            }
        }
    }
    found
}

/// The function `thread` creates.
fn created(thread: &ProgramThread) -> Function {
    Function {
        descriptor: Some(thread.descriptor),
        pc: thread.pc,
        state: thread.state,
    }
}

/// Whether `function` was traced and shares no cells with the function that
/// creates it: every instance of it runs alike.
fn shares_no_cells(function: Function, parts: &BTreeMap<Function, Part<'_>>) -> bool {
    matches!(parts.get(&function), Some(Part::Traced(Ok((_, facts)))) if facts.captured.is_empty())
}

/// `marker`'s range in `code`, which writes it exactly once at the start of
/// its line.
fn marker_range(code: &str, marker: &str) -> Result<std::ops::Range<usize>, String> {
    let mut found = code.match_indices(marker);
    let (Some((start, _)), None) = (found.next(), found.next()) else {
        return Err(format!("{marker} is not written exactly once"));
    };
    let line = code[..start].rfind('\n').map_or(0, |end| end + 1);
    if !code[line..start].chars().all(|c| c == '\t' || c == ' ') {
        return Err(format!("{marker} does not start its line"));
    }
    Ok(start..start + marker.len())
}

/// Each empty function `function NAME() {}` in `code`, the marker of a
/// function placed there: where it starts, and how many times it is
/// written. One scan for all of a function's markers, which
/// [`marker_range`] would each scan the code for.
fn marker_index(code: &str) -> std::collections::HashMap<&str, (usize, usize)> {
    let mut index = std::collections::HashMap::new();
    for (start, _) in code.match_indices("function ") {
        let name = start + "function ".len();
        let Some(open) = code[name..].find('(').map(|at| name + at) else {
            continue;
        };
        if code[open..].starts_with("() {}") {
            let entry = index
                .entry(&code[start..open + "() {}".len()])
                .or_insert((start, 0));
            entry.1 += 1;
        }
    }
    index
}

/// [`marker_range`], looked up in `index` (see [`marker_index`]).
fn indexed_marker_range(
    code: &str,
    index: &std::collections::HashMap<&str, (usize, usize)>,
    marker: &str,
) -> Result<std::ops::Range<usize>, String> {
    let Some(&(start, 1)) = index.get(marker) else {
        return Err(format!("{marker} is not written exactly once"));
    };
    let line = code[..start].rfind('\n').map_or(0, |end| end + 1);
    if !code[line..start].chars().all(|c| c == '\t' || c == ' ') {
        return Err(format!("{marker} does not start its line"));
    }
    Ok(start..start + marker.len())
}

struct Out<'o> {
    text: &'o mut String,
    indent: String,
    /// A line just ended: the next one, unless empty, is indented.
    pending: bool,
}

impl<'o> Out<'o> {
    fn new(text: &'o mut String, indent: String) -> Self {
        Self {
            text,
            indent,
            pending: false,
        }
    }

    fn push(&mut self, piece: &str) {
        for (index, line) in piece.split('\n').enumerate() {
            if index > 0 {
                self.text.push('\n');
                self.pending = true;
            }
            if !line.is_empty() {
                if self.pending {
                    self.text.push_str(&self.indent);
                }
                self.pending = false;
                self.text.push_str(line);
            }
        }
    }
}

/// Write `function`'s code as `function <name>() { … }`, each function it
/// creates placed in its empty one. One it creates as itself is this
/// function, so its creation is a reference, not a body; so is one that
/// shares no cells, made inside a placement of itself. `given` holds the
/// slots its creator passes it, `path` the functions it is nested in, with
/// the names they are placed under.
fn place(
    function: Function,
    name: &str,
    given: &BTreeSet<u32>,
    parts: &BTreeMap<Function, Part<'_>>,
    path: &mut Vec<(Function, String)>,
    out: &mut Out<'_>,
) -> Result<(), String> {
    let fail = |what: String| format!("{}: {what}", function.key());
    let (code, facts) = match parts.get(&function) {
        Some(Part::Traced(Ok((code, facts)))) => (code, facts),
        Some(Part::Traced(Err(error))) => return Err(fail(error.clone())),
        Some(Part::Stopped(reason)) => {
            let message = format!("VM function {} was not traced: {reason}", function.key());
            out.push(&format!(
                "function {name}() {{\n\tthrow new Error({message:?});\n}}"
            ));
            return Ok(());
        }
        None => return Err(fail("created but not walked".into())),
    };
    // The printer ends the code with a line break; what follows the placed
    // function is its creator's.
    let code = code.trim_end_matches('\n');
    // A function that shares cells, made again inside itself, is another
    // instance with its own cells: it is placed again, as long as the way
    // back to it passes one that shares none, whose next creation refers to
    // its placement (see below). Placing ends there.
    if let Some(last) = path.iter().rposition(|(placed, _)| *placed == function) {
        if !path[last + 1..]
            .iter()
            .any(|(placed, _)| shares_no_cells(*placed, parts))
        {
            return Err(fail("it is nested in itself".into()));
        }
    }
    if let Some(slot) = facts.captured.iter().find(|slot| !given.contains(slot)) {
        let creator = path.last().map_or_else(
            || "nothing (it is placed at the top level)".into(),
            |(creator, _)| creator.key(),
        );
        return Err(fail(format!(
            "it shares slot {slot}, which the function creating it ({creator}) does not pass"
        )));
    }
    // Its own name is the name it is placed under.
    let uses = code.matches(PROGRAM_SELF).count();
    if uses != facts.self_names + 1 {
        return Err(fail(format!(
            "{PROGRAM_SELF} is written {uses} times, not the {} it names itself",
            facts.self_names + 1
        )));
    }
    if facts
        .threads
        .iter()
        .any(|thread| thread.name == name && created(thread) != function)
    {
        return Err(fail(format!(
            "a function it creates would hide its own name {name}"
        )));
    }
    // A function that creates itself makes a fresh instance of the same code
    // from its own shared cells. That instance is this function, so there is
    // no second body to place: the creation refers to this one.
    let mut code = code.to_string();
    for thread in facts
        .threads
        .iter()
        .filter(|thread| created(thread) == function)
    {
        if let Some(slot) = facts
            .captured
            .iter()
            .find(|slot| !thread.cells.contains(slot))
        {
            return Err(fail(format!(
                "it shares slot {slot}, which creating itself does not pass"
            )));
        }
        let marker = format!("function {}() {{}}", thread.name);
        let range = marker_range(&code, &marker).map_err(|reason| fail(reason))?;
        let after = &code[range.end..];
        if !after.split('\n').next().unwrap_or(after).trim().is_empty() {
            return Err(fail(format!("{marker} is not alone on its line")));
        }
        if thread.name == name {
            // Its references are this function's own name; the line goes.
            let line = code[..range.start].rfind('\n').map_or(0, |end| end + 1);
            let through = if after.is_empty() {
                range.end
            } else {
                range.end + 1
            };
            code.replace_range(line..through, "");
        } else {
            // It is bound under the name it writes, as this function.
            code.replace_range(range, &format!("let {} = {name};", thread.name));
        }
    }
    // One that shares no cells, made inside a placement of itself: any
    // instance of it is that one, whose name is in scope here.
    let placed_above = |thread: &ProgramThread| {
        let child = created(thread);
        (child != function && shares_no_cells(child, parts))
            .then(|| path.iter().rev().find(|(placed, _)| *placed == child))
            .flatten()
            .map(|(_, placed)| placed.clone())
    };
    for thread in &facts.threads {
        let Some(placed) = placed_above(thread) else {
            continue;
        };
        let marker = format!("function {}() {{}}", thread.name);
        let range = marker_range(&code, &marker).map_err(|reason| fail(reason))?;
        if thread.name == placed {
            // Its references are already that function's name.
            let line = code[..range.start].rfind('\n').map_or(0, |end| end + 1);
            let through = if code[range.end..].is_empty() {
                range.end
            } else {
                range.end + 1
            };
            code.replace_range(line..through, "");
        } else {
            code.replace_range(range, &format!("let {} = {placed};", thread.name));
        }
    }
    // Each other function it creates, at its empty function.
    let mut sites = Vec::with_capacity(facts.threads.len());
    let index = marker_index(&code);
    for thread in facts
        .threads
        .iter()
        .filter(|thread| created(thread) != function && placed_above(thread).is_none())
    {
        let marker = format!("function {}() {{}}", thread.name);
        let range = indexed_marker_range(&code, &index, &marker).map_err(|reason| fail(reason))?;
        let start = range.start;
        let line = code[..start].rfind('\n').map_or(0, |end| end + 1);
        let indent = &code[line..start];
        sites.push((start, range.end, indent, thread, created(thread)));
    }
    sites.sort_by_key(|&(start, ..)| start);
    let mut cursor = 0;
    path.push((function, name.to_string()));
    for (start, end, indent, thread, child) in sites {
        out.push(&code[cursor..start].replace(PROGRAM_SELF, name));
        let given: BTreeSet<u32> = thread.cells.iter().copied().collect();
        let mut nested = Out::new(out.text, format!("{}{indent}", out.indent));
        let placed = place(child, &thread.name, &given, parts, path, &mut nested);
        if placed.is_err() {
            path.pop();
        }
        placed?;
        out.pending = false;
        cursor = end;
    }
    path.pop();
    out.push(&code[cursor..].replace(PROGRAM_SELF, name));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProgramThread;

    #[test]
    fn the_marker_index_finds_each_empty_function_as_marker_range_does() {
        let code = "let a;\n\tfunction $T_1() {}\nfunction $T_2(x) {}\nfunction $T_11() {}\nfunction $T_3() {}\nfunction $T_3() {}\nx; function $T_4() {}";
        let index = marker_index(code);
        for marker in [
            "function $T_1() {}",
            "function $T_11() {}",
            "function $T_2() {}",
            "function $T_3() {}",
            "function $T_4() {}",
            "function $T_5() {}",
        ] {
            assert_eq!(
                indexed_marker_range(code, &index, marker),
                marker_range(code, marker),
                "{marker}"
            );
        }
    }

    fn function(descriptor: Option<usize>, pc: usize, state: usize) -> Function {
        Function {
            descriptor,
            pc,
            state,
        }
    }

    type Site<'s> = (&'s str, usize, usize, usize, &'s [u32]);

    fn facts(threads: &[Site<'_>], captured: &[u32], self_names: usize) -> ProgramFacts {
        ProgramFacts {
            threads: threads
                .iter()
                .map(|&(name, descriptor, pc, state, cells)| ProgramThread {
                    name: name.into(),
                    descriptor,
                    pc,
                    state,
                    cells: cells.to_vec(),
                })
                .collect(),
            captured: captured.iter().copied().collect(),
            self_names,
            this_names: 0,
        }
    }

    type Traced = Result<(String, ProgramFacts), String>;

    fn built(parts: &[(Function, Traced)]) -> Result<String, String> {
        let map = parts
            .iter()
            .map(|(function, traced)| (*function, Part::Traced(traced)))
            .collect();
        build(&[parts[0].0], &map)
    }

    fn traced(code: &str, facts: ProgramFacts) -> Traced {
        Ok((code.into(), facts))
    }

    #[test]
    fn a_function_is_placed_in_the_empty_function_where_it_is_created() {
        let code = built(&[
            (
                function(None, 10, 1),
                traced(
                    "function $THREAD_SELF() {\n\tlet $KEY_3;\n\t$KEY_3 = 1;\n\tfunction $THREAD_2_100() {}\n\treturn $THREAD_2_100;\n}",
                    facts(&[("$THREAD_2_100", 7, 100, 2, &[3])], &[], 0),
                ),
            ),
            (
                function(Some(7), 100, 2),
                traced(
                    "function $THREAD_SELF() {\n\treturn [$KEY_3, $THREAD_SELF, arguments[0]];\n}",
                    facts(&[], &[3], 1),
                ),
            ),
        ])
        .unwrap();
        assert_eq!(
            code,
            "// Traced from a JavaScript VM capture by `shape trace program`.\n\nfunction $THREAD_1_10() {\n\tlet $KEY_3;\n\t$KEY_3 = 1;\n\tfunction $THREAD_2_100() {\n\t\treturn [$KEY_3, $THREAD_2_100, arguments[0]];\n\t}\n\treturn $THREAD_2_100;\n}\n\n$THREAD_1_10();\n"
        );
    }

    #[test]
    fn a_function_that_was_not_traced_throws_where_it_is_made() {
        let entry = function(None, 10, 1);
        let parent = traced(
            "function $THREAD_SELF() {\n\tfunction $THREAD_2_100() {}\n}",
            facts(&[("$THREAD_2_100", 7, 100, 2, &[])], &[], 0),
        );
        let parts = BTreeMap::from([
            (entry, Part::Traced(&parent)),
            (function(Some(7), 100, 2), Part::Stopped("a reason")),
        ]);
        let code = build(&[entry], &parts).unwrap();
        assert!(code.contains("function $THREAD_2_100() {\n\t\tthrow new Error(\"VM function d7@100/2 was not traced: a reason\");\n\t}"), "{code}");
    }

    #[test]
    fn a_function_sharing_a_slot_its_creator_does_not_pass_fails() {
        let error = built(&[
            (
                function(None, 10, 1),
                traced(
                    "function $THREAD_SELF() {\n\tfunction $THREAD_2_100() {}\n}",
                    facts(&[("$THREAD_2_100", 7, 100, 2, &[])], &[], 0),
                ),
            ),
            (
                function(Some(7), 100, 2),
                traced(
                    "function $THREAD_SELF() {\n\treturn $KEY_3;\n}",
                    facts(&[], &[3], 0),
                ),
            ),
        ])
        .unwrap_err();
        assert!(error.contains("shares slot 3"), "{error}");
    }

    #[test]
    fn its_own_name_written_other_than_counted_fails() {
        let error = built(&[(
            function(None, 10, 1),
            traced(
                "function $THREAD_SELF() {\n\treturn \"$THREAD_SELF\";\n}",
                facts(&[], &[], 0),
            ),
        )])
        .unwrap_err();
        assert!(error.contains("written 2 times"), "{error}");
    }

    #[test]
    fn a_creation_missing_its_empty_function_fails() {
        let error = built(&[(
            function(None, 10, 1),
            traced(
                "function $THREAD_SELF() {\n\treturn 1;\n}",
                facts(&[("$THREAD_2_100", 7, 100, 2, &[])], &[], 0),
            ),
        )])
        .unwrap_err();
        assert!(error.contains("not written exactly once"), "{error}");
    }

    #[test]
    fn a_function_that_creates_itself_is_placed_once() {
        let code = built(&[
            (
                function(None, 10, 1),
                traced(
                    "function $THREAD_SELF() {\n\tlet $KEY_3;\n\t$KEY_3 = 1;\n\tfunction $THREAD_2_100() {}\n\treturn $THREAD_2_100;\n}",
                    facts(&[("$THREAD_2_100", 7, 100, 2, &[3])], &[], 0),
                ),
            ),
            (
                function(Some(7), 100, 2),
                traced(
                    "function $THREAD_SELF() {\n\tfunction $THREAD_2_100() {}\n\treturn [$KEY_3, $THREAD_SELF, $THREAD_2_100, arguments[0]];\n}",
                    facts(&[("$THREAD_2_100", 7, 100, 2, &[3])], &[3], 1),
                ),
            ),
        ])
        .unwrap();
        assert_eq!(
            code,
            "// Traced from a JavaScript VM capture by `shape trace program`.\n\nfunction $THREAD_1_10() {\n\tlet $KEY_3;\n\t$KEY_3 = 1;\n\tfunction $THREAD_2_100() {\n\t\treturn [$KEY_3, $THREAD_2_100, $THREAD_2_100, arguments[0]];\n\t}\n\treturn $THREAD_2_100;\n}\n\n$THREAD_1_10();\n"
        );
    }

    #[test]
    fn a_self_creation_under_another_name_refers_to_the_function() {
        let code = built(&[
            (
                function(None, 10, 1),
                traced(
                    "function $THREAD_SELF() {\n\tfunction $THREAD_2_100() {}\n\treturn $THREAD_2_100;\n}",
                    facts(&[("$THREAD_2_100", 7, 100, 2, &[])], &[], 0),
                ),
            ),
            (
                function(Some(7), 100, 2),
                traced(
                    "function $THREAD_SELF() {\n\tfunction $THREAD_2_100_2() {}\n\treturn [$THREAD_SELF, $THREAD_2_100_2];\n}",
                    facts(&[("$THREAD_2_100_2", 7, 100, 2, &[])], &[], 1),
                ),
            ),
        ])
        .unwrap();
        assert!(
            code.contains("\t\tlet $THREAD_2_100_2 = $THREAD_2_100;\n\t\treturn [$THREAD_2_100, $THREAD_2_100_2];"),
            "{code}"
        );
    }

    #[test]
    fn a_function_creating_itself_without_its_cells_fails() {
        let error = built(&[
            (
                function(None, 10, 1),
                traced(
                    "function $THREAD_SELF() {\n\tfunction $THREAD_2_100() {}\n}",
                    facts(&[("$THREAD_2_100", 7, 100, 2, &[3])], &[], 0),
                ),
            ),
            (
                function(Some(7), 100, 2),
                traced(
                    "function $THREAD_SELF() {\n\tfunction $THREAD_2_100() {}\n\treturn $KEY_3;\n}",
                    facts(&[("$THREAD_2_100", 7, 100, 2, &[])], &[3], 0),
                ),
            ),
        ])
        .unwrap_err();
        assert!(
            error.contains("shares slot 3, which creating itself does not pass"),
            "{error}"
        );
    }

    #[test]
    fn a_created_function_that_would_hide_its_creators_name_fails() {
        let error = built(&[(
            function(None, 10, 1),
            traced(
                "function $THREAD_SELF() {\n\tfunction $THREAD_1_10() {}\n}",
                facts(&[("$THREAD_1_10", 7, 10, 1, &[])], &[], 0),
            ),
        )])
        .unwrap_err();
        assert!(error.contains("would hide its own name"), "{error}");
    }

    #[test]
    fn a_function_sharing_no_cells_made_inside_itself_refers_to_its_placement() {
        let code = built(&[
            (
                function(None, 10, 1),
                traced(
                    "function $THREAD_SELF() {\n\tfunction $THREAD_2_100() {}\n}",
                    facts(&[("$THREAD_2_100", 7, 100, 2, &[])], &[], 0),
                ),
            ),
            (
                function(Some(7), 100, 2),
                traced(
                    "function $THREAD_SELF() {\n\tfunction $THREAD_9_300() {}\n}",
                    facts(&[("$THREAD_9_300", 8, 300, 9, &[])], &[], 0),
                ),
            ),
            (
                function(Some(8), 300, 9),
                traced(
                    "function $THREAD_SELF() {\n\tfunction $THREAD_2_100() {}\n\treturn $THREAD_2_100;\n}",
                    facts(&[("$THREAD_2_100", 7, 100, 2, &[])], &[], 0),
                ),
            ),
        ])
        .unwrap();
        assert_eq!(
            code,
            "// Traced from a JavaScript VM capture by `shape trace program`.\n\nfunction $THREAD_1_10() {\n\tfunction $THREAD_2_100() {\n\t\tfunction $THREAD_9_300() {\n\t\t\treturn $THREAD_2_100;\n\t\t}\n\t}\n}\n\n$THREAD_1_10();\n"
        );
    }

    #[test]
    fn a_cycle_of_functions_that_all_share_cells_fails() {
        let error = built(&[
            (
                function(None, 10, 1),
                traced(
                    "function $THREAD_SELF() {\n\tlet $KEY_3;\n\tfunction $THREAD_2_100() {}\n}",
                    facts(&[("$THREAD_2_100", 7, 100, 2, &[3])], &[], 0),
                ),
            ),
            (
                function(Some(7), 100, 2),
                traced(
                    "function $THREAD_SELF() {\n\tfunction $THREAD_9_300() {}\n\treturn $KEY_3;\n}",
                    facts(&[("$THREAD_9_300", 8, 300, 9, &[3])], &[3], 0),
                ),
            ),
            (
                function(Some(8), 300, 9),
                traced(
                    "function $THREAD_SELF() {\n\tfunction $THREAD_2_100() {}\n\treturn $KEY_3;\n}",
                    facts(&[("$THREAD_2_100", 7, 100, 2, &[3])], &[3], 0),
                ),
            ),
        ])
        .unwrap_err();
        assert!(error.contains("nested in itself"), "{error}");
    }

    #[test]
    fn the_check_fails_a_read_of_what_it_was_not_given() {
        let shares_3 = facts(&[], &[3], 0);
        let error = check(&shares_3, &["$KEY_4".into(), "host".into()], &[]).unwrap_err();
        assert!(error.contains("$KEY_4"), "{error}");
        assert!(check(&shares_3, &["$KEY_3".into(), "host".into()], &[]).is_ok());
    }

    #[test]
    fn the_check_fails_a_declaration_of_a_shared_slot() {
        let error = check(&facts(&[], &[3], 0), &[], &["$KEY_3".into()]).unwrap_err();
        assert!(error.contains("declares $KEY_3"), "{error}");
    }
}
