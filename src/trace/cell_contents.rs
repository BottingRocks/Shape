//! What each shared memory cell can hold, across every VM function of a
//! seed.
//!
//! `vmCaptureThread` hands a closure the creating function's
//! `memory.slots[slot]` objects for the slots its descriptor captures, so a
//! cell is shared by its owner (the function whose own slot it is) and every
//! closure, transitively, that captures it. A compiled program never makes a
//! bytecode address a JavaScript value, so an address in a cell is one some
//! holder's own code wrote there as a jump setup. Each walk reports what it
//! put in each cell (`PrefixTraceReport::cell_writes`): the addresses, and
//! whether it wrote JavaScript values. A cell's content is the union over
//! all its holders, and the walks run again with the contents until they no
//! longer change. A walk runs again only if a cell it read now reads
//! differently (`CellReads::same`): another value set where it asked a
//! read's value set, or data where it asked whether a read was data.
//! Otherwise it would trace the same, and stands. Only a function that
//! needs contents is given any (see [`CellGraph::needing`]): one whose walk
//! stopped, and the holders that write the cells such a function reads. A
//! walk that completed with its cells unknown traced code that holds
//! whatever they hold, and stands. A content is only given when it is complete: every holder
//! was walked to completion and knew what it put there. A VM jump through
//! the cell then dispatches on its addresses, and one through a cell that
//! holds only JavaScript values left the code the VM runs.

use std::collections::{BTreeMap, BTreeSet};

use super::corpus::Function;
use crate::{CellContent, PrefixStopReason, PrefixTraceReport};

/// The most rounds of walking again before giving up on the contents.
pub const MAX_ROUNDS: usize = 6;

/// The contents each function's cells get, by slot.
pub type CellContents = BTreeMap<Function, BTreeMap<u32, CellContent>>;

/// What one walk tells about cells: whether it completed, the functions it
/// creates and what it put in each cell.
#[derive(Debug, Clone)]
pub struct Summary {
    pub complete: bool,
    pub creates: Vec<Function>,
    pub writes: BTreeMap<u32, Option<CellContent>>,
}

impl Summary {
    pub fn of(result: &PrefixTraceReport, creates: Vec<Function>) -> Self {
        Self {
            complete: result.stop == PrefixStopReason::Completed,
            creates,
            writes: result.cell_writes.clone(),
        }
    }

    /// A function that could not be walked: it holds nothing known.
    pub fn failed() -> Self {
        Self {
            complete: false,
            creates: Vec::new(),
            writes: BTreeMap::new(),
        }
    }
}

/// A cell: its owner and slot, or one whose owner is not known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Cell {
    Owned(Function, u32),
    Unknown,
}

/// What each function's cells can hold, from `summaries` of every function
/// reachable from the entry; `captured(f)` is the slots `f`'s descriptor
/// captures (and does not initialize). `strict` requires every holder of a
/// cell to have completed; without it a holder that stopped gives what it
/// put there so far, a guess that lets the walk that needs the content
/// complete, to be checked strictly once the contents settle.
#[cfg(test)]
pub fn cell_contents(
    summaries: &BTreeMap<Function, Summary>,
    captured: impl Fn(Function) -> Vec<u32>,
    strict: bool,
) -> CellContents {
    CellGraph::of(summaries, captured).contents(summaries, strict)
}

/// Which cells each function's captured slots can be, and who holds each
/// cell: it depends only on which function creates which and what each
/// captures, not on what the walks wrote, so a round whose functions create
/// what they did before reuses it.
pub struct CellGraph {
    /// The functions each function creates, which the graph is of.
    creates: BTreeMap<Function, Vec<Function>>,
    cells: BTreeMap<(Function, u32), BTreeSet<Cell>>,
    holders: BTreeMap<Cell, BTreeSet<(Function, u32)>>,
}

impl CellGraph {
    /// Whether this is the graph of `summaries`' functions.
    pub fn is_of(&self, summaries: &BTreeMap<Function, Summary>) -> bool {
        self.creates.len() == summaries.len()
            && self
                .creates
                .iter()
                .zip(summaries)
                .all(|((a, creates), (b, summary))| a == b && *creates == summary.creates)
    }

    pub fn of(
        summaries: &BTreeMap<Function, Summary>,
        captured: impl Fn(Function) -> Vec<u32>,
    ) -> Self {
        let mut creators: BTreeMap<Function, Vec<Function>> = BTreeMap::new();
        for (&function, summary) in summaries {
            for &created in &summary.creates {
                creators.entry(created).or_default().push(function);
            }
        }
        let captured: BTreeMap<Function, BTreeSet<u32>> = summaries
            .keys()
            .map(|&function| (function, captured(function).into_iter().collect()))
            .collect();
        // The cells each function's captured slot can be, to a fixpoint: a
        // creator's own slot, or what the creator's captured slot can be.
        let mut cells: BTreeMap<(Function, u32), BTreeSet<Cell>> = BTreeMap::new();
        for (&function, slots) in &captured {
            for &slot in slots {
                let entry = cells.entry((function, slot)).or_default();
                if creators.get(&function).is_none_or(Vec::is_empty) {
                    entry.insert(Cell::Unknown);
                }
            }
        }
        loop {
            let mut changed = false;
            for (&function, slots) in &captured {
                for &slot in slots {
                    let mut found = BTreeSet::new();
                    for &creator in creators.get(&function).into_iter().flatten() {
                        if captured[&creator].contains(&slot) {
                            found
                                .extend(cells.get(&(creator, slot)).into_iter().flatten().copied());
                        } else {
                            found.insert(Cell::Owned(creator, slot));
                        }
                    }
                    let entry = cells.entry((function, slot)).or_default();
                    for cell in found {
                        changed |= entry.insert(cell);
                    }
                }
            }
            if !changed {
                break;
            }
        }
        // The holders of each owned cell: its owner and every function whose
        // captured slot can be it.
        let mut holders: BTreeMap<Cell, BTreeSet<(Function, u32)>> = BTreeMap::new();
        for (&(function, slot), owners) in &cells {
            for &cell in owners {
                if let Cell::Owned(owner, _) = cell {
                    holders.entry(cell).or_default().insert((owner, slot));
                }
                holders.entry(cell).or_default().insert((function, slot));
            }
        }
        let creates = summaries
            .iter()
            .map(|(&function, summary)| (function, summary.creates.clone()))
            .collect();
        Self {
            creates,
            cells,
            holders,
        }
    }

    /// The cells `function`'s slot `slot` can be: its own cell, or those
    /// its captured slot can be.
    fn cells_of(&self, function: Function, slot: u32) -> Vec<Cell> {
        match self.cells.get(&(function, slot)) {
            Some(cells) => cells.iter().copied().collect(),
            None => vec![Cell::Owned(function, slot)],
        }
    }

    /// The functions that need cell contents: each one whose walk stopped,
    /// and every holder that writes a cell a function that needs them read
    /// (`read(f)`, the slots `f`'s walk read), since what it writes there
    /// may come from what it reads. A function that completed and writes
    /// no such cell traced code that holds for whatever its cells hold.
    pub fn needing<'a>(
        &self,
        summaries: &BTreeMap<Function, Summary>,
        read: impl Fn(Function) -> Option<&'a BTreeSet<u32>>,
    ) -> BTreeSet<Function> {
        let mut needing: BTreeSet<Function> = summaries
            .iter()
            .filter(|(_, summary)| !summary.complete)
            .map(|(&function, _)| function)
            .collect();
        let mut pending: Vec<Function> = needing.iter().copied().collect();
        let mut seen = BTreeSet::new();
        while let Some(function) = pending.pop() {
            for &slot in read(function).into_iter().flatten() {
                for cell in self.cells_of(function, slot) {
                    if !seen.insert(cell) {
                        continue;
                    }
                    for &(holder, slot) in self.holders.get(&cell).into_iter().flatten() {
                        let writes = summaries
                            .get(&holder)
                            .is_some_and(|summary| summary.writes.contains_key(&slot));
                        if writes && needing.insert(holder) {
                            pending.push(holder);
                        }
                    }
                }
            }
        }
        needing
    }

    /// What each function's cells can hold (see [`cell_contents`]).
    pub fn contents(
        &self,
        summaries: &BTreeMap<Function, Summary>,
        strict: bool,
    ) -> CellContents {
        let (cells, holders) = (&self.cells, &self.holders);
        let content = |cell: Cell| -> Option<CellContent> {
            let Cell::Owned(..) = cell else {
                return None;
            };
            let mut all = CellContent::default();
            for &(holder, slot) in holders.get(&cell).into_iter().flatten() {
                let summary = summaries.get(&holder)?;
                if strict && !summary.complete {
                    return None;
                }
                match summary.writes.get(&slot) {
                    None => {}
                    Some(None) => return None,
                    Some(Some(written)) => all.merge(written),
                }
            }
            (!all.addresses.is_empty() || all.data).then_some(all)
        };
        // Each cell's content, once: many captured slots can be the same cell.
        let content: BTreeMap<Cell, Option<CellContent>> =
            holders.keys().map(|&cell| (cell, content(cell))).collect();
        let content = |cell: Cell| content.get(&cell).cloned().flatten();
        let mut contents: CellContents = BTreeMap::new();
        // A captured slot holds what any of the cells it can be holds.
        for (&(function, slot), owners) in cells {
            let mut all = CellContent::default();
            let mut known = !owners.is_empty();
            for &cell in owners {
                match content(cell) {
                    Some(content) => all.merge(&content),
                    None => known = false,
                }
            }
            if known {
                contents.entry(function).or_default().insert(slot, all);
            }
        }
        // An owned cell holds what its holders put there.
        for &cell in holders.keys() {
            if let (Cell::Owned(owner, slot), Some(all)) = (cell, content(cell)) {
                contents.entry(owner).or_default().insert(slot, all);
            }
        }
        contents
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function(descriptor: usize) -> Function {
        Function {
            descriptor: Some(descriptor),
            pc: descriptor,
            state: 0,
        }
    }

    fn content(addresses: &[usize], data: bool) -> Option<CellContent> {
        Some(CellContent {
            addresses: addresses.iter().copied().collect(),
            data,
        })
    }

    fn summary(creates: Vec<Function>, writes: &[(u32, Option<CellContent>)]) -> Summary {
        Summary {
            complete: true,
            creates,
            writes: writes.iter().cloned().collect(),
        }
    }

    #[test]
    fn a_captured_cell_holds_what_every_holder_puts_there() {
        // The owner (1) writes address 7 into slot 5; the closure it creates
        // (2) captures slot 5 and writes a JavaScript value; so does its
        // closure (3), which writes address 9.
        let (owner, child, grandchild) = (function(1), function(2), function(3));
        let summaries = BTreeMap::from([
            (owner, summary(vec![child], &[(5, content(&[7], false))])),
            (child, summary(vec![grandchild], &[(5, content(&[], true))])),
            (grandchild, summary(vec![], &[(5, content(&[9], false))])),
        ]);
        let captured = |f: Function| if f == owner { vec![] } else { vec![5] };
        let contents = cell_contents(&summaries, captured, true);
        for f in [owner, child, grandchild] {
            assert_eq!(contents[&f][&5], content(&[7, 9], true).unwrap(), "{f:?}");
        }
    }

    #[test]
    fn only_a_stopped_walk_and_the_writers_of_what_it_reads_need_contents() {
        // The owner (1) creates a jumper (2) and a copier (3), and an
        // unrelated closure (4). The copier reads slot 6 and writes slot 5;
        // the jumper stopped, having read slot 5; the owner writes slot 6.
        let (owner, jumper, copier, other) = (function(1), function(2), function(3), function(4));
        let mut stopped = summary(vec![], &[]);
        stopped.complete = false;
        let summaries = BTreeMap::from([
            (
                owner,
                summary(vec![jumper, copier, other], &[(6, content(&[7], false))]),
            ),
            (jumper, stopped),
            (copier, summary(vec![], &[(5, content(&[9], false))])),
            (other, summary(vec![], &[(8, content(&[], true))])),
        ]);
        let captured = |f: Function| if f == owner { vec![] } else { vec![5, 6, 8] };
        let graph = CellGraph::of(&summaries, captured);
        let reads = BTreeMap::from([
            (jumper, BTreeSet::from([5])),
            (copier, BTreeSet::from([6])),
            (other, BTreeSet::from([8])),
        ]);
        let needing = graph.needing(&summaries, |f| reads.get(&f));
        assert_eq!(needing, BTreeSet::from([owner, jumper, copier]));
        // With every walk complete, none does.
        let mut complete = summaries.clone();
        complete.get_mut(&jumper).unwrap().complete = true;
        assert!(graph.needing(&complete, |f| reads.get(&f)).is_empty());
    }

    #[test]
    fn an_unknown_write_an_incomplete_holder_or_an_unknown_creator_gives_nothing() {
        let (owner, child) = (function(1), function(2));
        let captured = |f: Function| if f == owner { vec![] } else { vec![5] };
        let mut summaries = BTreeMap::from([
            (owner, summary(vec![child], &[(5, content(&[7], false))])),
            (child, summary(vec![], &[(5, None)])),
        ]);
        assert!(cell_contents(&summaries, captured, true).is_empty());
        summaries.get_mut(&child).unwrap().writes.clear();
        summaries.get_mut(&child).unwrap().complete = false;
        assert!(cell_contents(&summaries, captured, true).is_empty());
        assert!(!cell_contents(&summaries, captured, false).is_empty());
        let orphan = BTreeMap::from([(child, summary(vec![], &[(5, content(&[1], false))]))]);
        assert!(cell_contents(&orphan, |_| vec![5], true).is_empty());
    }
}
