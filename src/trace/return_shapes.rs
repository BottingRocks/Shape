//! What a call of a VM function returns, from the function's own walk.
//!
//! A walk that calls a VM function it knows (one made here, or a variable
//! every value of which is one) asks whether every return of that function
//! gives a VM function the callee makes: a call of it is then a function, to
//! tests (see [`crate::tracing::symbolic::ReturnsHook`]). The answer is what
//! the callee's own walk found: a function is a maker when its walk completed
//! and every return gives a VM function made.
//!
//! The walks go in the order the code runs: a walk that asks of a function
//! not walked yet walks it there, before it goes on, and that walk is the
//! function's own; one asked of while another thread walks it waits for that
//! walk (see [`Round::walk`]): no function is walked twice in a round. A walk
//! whose wait would come back to itself (a function that calls itself,
//! through others or not) is told what the round before found; a walk told
//! an answer that its callee's walk does not give runs again in the next
//! round. The rounds end when every walk was told what the walks found.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Condvar, Mutex, MutexGuard,
    },
    thread::ThreadId,
};

use super::cell_contents::{CellContents, CellGraph, Summary};
use super::corpus::{
    captured_slots, created_functions, invoke_announcing, setup, Announce, Function,
};
use crate::{
    tracing::symbolic::{CellTypesHook, ReturnsHook},
    CellContent, CellReads, DescriptorRecord, PrefixPreparation,
    PrefixStopReason, PrefixTraceReport, TraceNodeKind, TraceStatementKind,
};

/// The functions every return of which gives a VM function made.
pub type Makers = BTreeSet<Function>;

/// The answers one walk was given, by the function asked of.
pub type Answers = BTreeMap<Function, bool>;

/// What one walk was told the closures it made put in a cell (see
/// [`CellTypesHook`]), by closure and slot.
pub type CellAnswers = BTreeMap<(Function, u32), Option<u16>>;

/// The most rounds of walking again for answers a call of itself got. A
/// round's walks only cut what earlier answers left open, so the makers
/// settle within a few rounds; past this the last round's walks stand.
pub const MAX_ROUNDS: usize = 8;

/// One function's walk in a round.
pub struct Walked {
    /// The walk, or why the function could not be set up.
    pub report: Result<PrefixTraceReport, String>,
    /// What it was told of the functions it called.
    pub answers: Answers,
    /// What it was told the closures it made put in their cells.
    pub cell_answers: CellAnswers,
    /// Whether every return gives a VM function made.
    pub makes: bool,
    /// The cell contents it was walked with.
    pub contents: BTreeMap<u32, crate::CellContent>,
}

impl Walked {
    /// Whether this walk is the one contents `given` would give: see
    /// [`stands_for`].
    fn stands_for(&self, given: &BTreeMap<u32, CellContent>) -> bool {
        let read = self
            .report
            .as_ref()
            .ok()
            .filter(|report| report.stop == PrefixStopReason::Completed)
            .map(|report| &report.cell_reads);
        stands_for(&self.contents, read, given)
    }
}

/// Whether a walk made with contents `used` is the walk contents `given`
/// would make: the same contents, or a walk that completed (`read`, what
/// it read of the cells) told the same wherever it asked (see
/// [`CellReads::same`]). A walk that stopped is walked again with any new
/// contents: those are what it may have needed.
fn stands_for(
    used: &BTreeMap<u32, CellContent>,
    read: Option<&CellReads>,
    given: &BTreeMap<u32, CellContent>,
) -> bool {
    used == given || read.is_some_and(|read| read.same(used, given))
}

/// What every walk of a round shares: how to set one up, the makers the
/// round before found and the walks so far.
pub struct Round {
    pub prepared: Arc<PrefixPreparation>,
    pub descriptors: Arc<[DescriptorRecord]>,
    pub initial_slots: Arc<[u32]>,
    pub captures: Arc<[Vec<u32>]>,
    pub steps: usize,
    pub contents: Arc<CellContents>,
    /// What the round before found: the answer for a function asked of
    /// while its walk is further up the thread.
    pub makers: Arc<Makers>,
    book: Mutex<Book>,
    /// Told each time a walk is done.
    done: Condvar,
    /// The walks this round made itself, not taken from the kept ones.
    walked: AtomicUsize,
    /// The functions walked again with what their ancestors put in their
    /// cells (see [`Round::ancestors_contents`]).
    given: AtomicUsize,
    /// Branches the user decides, in every walk.
    overrides: Arc<crate::BranchOverrides>,
}

/// The walks of a round: done, being walked and waited for.
#[derive(Default)]
struct Book {
    walks: BTreeMap<Function, Arc<Walked>>,
    /// The thread walking each function being walked.
    running: BTreeMap<Function, ThreadId>,
    /// The function each waiting thread waits for.
    waiting: HashMap<ThreadId, Function>,
    /// The functions that create each function, as walks announce them.
    creators: BTreeMap<Function, BTreeSet<Function>>,
}

impl Book {
    /// Whether `thread` waiting for the walk `owner` runs comes back to it:
    /// `owner` is it, or waits, through others, for a walk it runs.
    fn waits_for(&self, mut owner: ThreadId, thread: ThreadId) -> bool {
        for _ in 0..=self.waiting.len() {
            if owner == thread {
                return true;
            }
            let Some(next) = self
                .waiting
                .get(&owner)
                .and_then(|function| self.running.get(function))
            else {
                return false;
            };
            owner = *next;
        }
        true
    }
}

/// A claimed walk: told done, or given up, when dropped (a walk that
/// panicked leaves no thread waiting for it).
struct Claim<'a> {
    round: &'a Round,
    function: Function,
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        if let Ok(mut book) = self.round.book.lock() {
            book.running.remove(&self.function);
        }
        self.round.done.notify_all();
    }
}

impl Round {
    /// A round told `makers`, with the walks of `kept` that stand for it:
    /// walked with the same contents and told what `makers` tells.
    pub fn new(
        prepared: Arc<PrefixPreparation>,
        descriptors: Arc<[DescriptorRecord]>,
        initial_slots: Arc<[u32]>,
        captures: Arc<[Vec<u32>]>,
        steps: usize,
        contents: Arc<CellContents>,
        makers: Arc<Makers>,
        kept: &BTreeMap<Function, Arc<Walked>>,
    ) -> Self {
        let walks = kept
            .iter()
            .filter(|(function, walked)| {
                walked.stands_for(&contents.get(function).cloned().unwrap_or_default())
                    && stand(&walked.answers, &makers)
            })
            .map(|(&function, walked)| (function, walked.clone()))
            .collect();
        let walks = standing_cell_answers(walks, &descriptors);
        Self {
            prepared,
            descriptors,
            initial_slots,
            captures,
            steps,
            contents,
            makers,
            book: Mutex::new(Book {
                walks,
                ..Book::default()
            }),
            done: Condvar::new(),
            walked: AtomicUsize::new(0),
            given: AtomicUsize::new(0),
            overrides: Arc::default(),
        }
    }

    /// Walk every function with these branches decided by the user (see
    /// [`crate::BranchOverrides`]).
    #[must_use]
    pub fn with_overrides(mut self, overrides: Arc<crate::BranchOverrides>) -> Self {
        self.overrides = overrides;
        self
    }

    /// How many functions this round walked again with what their
    /// ancestors put in their cells.
    pub fn given(&self) -> usize {
        self.given.load(Ordering::Relaxed)
    }

    /// How many functions this round walked itself: the others' walks
    /// were kept from before.
    pub fn walked(&self) -> usize {
        self.walked.load(Ordering::Relaxed)
    }

    /// The function's walk this round: the one done, the one another
    /// thread is doing, or one done now.
    pub fn walk(self: &Arc<Self>, function: Function, announce: &Announce) -> Arc<Walked> {
        loop {
            if let Some(walked) = self.walk_unless_waiting_on_itself(function, announce) {
                return walked;
            }
        }
    }

    /// [`Self::walk`], or `None` where waiting for the walk comes back to
    /// this thread.
    fn walk_unless_waiting_on_itself(
        self: &Arc<Self>,
        function: Function,
        announce: &Announce,
    ) -> Option<Arc<Walked>> {
        let thread = std::thread::current().id();
        let mut book = self.book.lock().ok()?;
        loop {
            if let Some(walked) = book.walks.get(&function) {
                return Some(walked.clone());
            }
            match book.running.get(&function).copied() {
                None => {
                    book.running.insert(function, thread);
                    break;
                }
                Some(owner) if book.waits_for(owner, thread) => return None,
                Some(_) => {
                    book.waiting.insert(thread, function);
                    book = self.wait(book)?;
                    book.waiting.remove(&thread);
                }
            }
        }
        drop(book);
        let claim = Claim {
            round: self,
            function,
        };
        let walked = Arc::new(self.walk_now(function, announce));
        self.book
            .lock()
            .ok()?
            .walks
            .insert(function, walked.clone());
        drop(claim);
        Some(walked)
    }

    fn wait<'a>(&self, book: MutexGuard<'a, Book>) -> Option<MutexGuard<'a, Book>> {
        self.done.wait(book).ok()
    }

    fn walk_now(self: &Arc<Self>, function: Function, announce: &Announce) -> Walked {
        self.walked.fetch_add(1, Ordering::Relaxed);
        // What the walk creates, as it creates it: the creators of a
        // function are where its cells come from.
        let announce: Announce = {
            let (round, announce) = (self.clone(), announce.clone());
            Arc::new(move |created: &[Function]| {
                if let Ok(mut book) = round.book.lock() {
                    for &child in created {
                        book.creators.entry(child).or_default().insert(function);
                    }
                }
                announce(created);
            })
        };
        let walk = |contents: &BTreeMap<u32, CellContent>| {
            let answers = Arc::new(Mutex::new(Answers::new()));
            let cell_answers = Arc::new(Mutex::new(CellAnswers::new()));
            let report = setup(function, &self.descriptors, &self.initial_slots, self.steps)
                .and_then(|mut setup| {
                    setup.captures = self.captures.clone();
                    setup.overrides = self.overrides.clone();
                    setup.cell_contents = contents.clone();
                    setup.returns = Some(self.hook(answers.clone(), &announce));
                    setup.cell_types = Some(self.cell_types_hook(cell_answers.clone(), &announce));
                    invoke_announcing(&self.prepared, function, &setup, Some(&announce))
                });
            let answers = answers
                .lock()
                .map(|given| given.clone())
                .unwrap_or_default();
            let cell_answers = cell_answers
                .lock()
                .map(|given| given.clone())
                .unwrap_or_default();
            (report, (answers, cell_answers))
        };
        let mut contents = self.contents.get(&function).cloned().unwrap_or_default();
        let (mut report, mut answers) = walk(&contents);
        // A walk that stopped after reading shared cells is walked again
        // with what its ancestors, walked first, put in them: a VM jump
        // through a cell its creator set up. Again while what it puts there
        // itself changes what it asked. A round given contents (see
        // `crate::trace::cell_contents`) walks with those only.
        if self.contents.is_empty() {
            for _ in 0..crate::trace::cell_contents::MAX_ROUNDS {
                let Ok(result) = &report else {
                    break;
                };
                let stopped = result.stop != PrefixStopReason::Completed;
                if (!stopped && contents.is_empty()) || result.cell_reads.cells.is_empty() {
                    break;
                }
                let found = self.ancestors_contents(function, result, &announce);
                let again = if stopped {
                    found != contents
                } else {
                    !result.cell_reads.same(&contents, &found)
                };
                if !again {
                    break;
                }
                if contents.is_empty() {
                    self.given.fetch_add(1, Ordering::Relaxed);
                }
                contents = found;
                (report, answers) = walk(&contents);
            }
        }
        let makes = report.as_ref().is_ok_and(makes_functions);
        let (answers, cell_answers) = answers;
        Walked {
            report,
            answers,
            cell_answers,
            makes,
            contents,
        }
    }

    /// What `function`'s cells hold, from the walks of the round so far and
    /// its own (`own`), once the walks of its ancestors are done: what the
    /// functions that hold each cell put there (see [`CellGraph`]). A
    /// holder not walked yet (a sibling, a function created later) is not
    /// counted: the walks are checked against every holder once all are
    /// done.
    fn ancestors_contents(
        self: &Arc<Self>,
        function: Function,
        own: &PrefixTraceReport,
        announce: &Announce,
    ) -> BTreeMap<u32, CellContent> {
        let mut ancestors = BTreeSet::new();
        let mut pending = vec![function];
        while let Some(next) = pending.pop() {
            let creators = self
                .book
                .lock()
                .map(|book| book.creators.get(&next).cloned().unwrap_or_default())
                .unwrap_or_default();
            for creator in creators {
                if creator != function && ancestors.insert(creator) {
                    // Done, or waited for; not when it waits for this walk.
                    let _ = self.walk_unless_waiting_on_itself(creator, announce);
                    pending.push(creator);
                }
            }
        }
        let mut summaries: BTreeMap<Function, Summary> = self
            .walks()
            .into_iter()
            .filter_map(|(walked, walk)| {
                let report = walk.report.as_ref().ok()?;
                Some((walked, Summary::of(report, created_functions(report))))
            })
            .collect();
        summaries.insert(function, Summary::of(own, created_functions(own)));
        CellGraph::of(&summaries, |walked| {
            captured_slots(walked, &self.descriptors)
        })
        .contents(&summaries, false)
        .remove(&function)
        .unwrap_or_default()
    }

    /// Every walk of the round.
    pub fn walks(&self) -> BTreeMap<Function, Arc<Walked>> {
        self.book
            .lock()
            .map(|book| book.walks.clone())
            .unwrap_or_default()
    }

    /// A hook for a walk outside the round's own (one function walked
    /// alone, say): it walks what it asks of as the round's walks do.
    pub fn asking(self: &Arc<Self>) -> ReturnsHook {
        let announce: Announce = Arc::new(|_: &[Function]| {});
        self.hook(Arc::new(Mutex::new(Answers::new())), &announce)
    }

    /// A cell-types hook for a walk outside the round's own (see
    /// [`Self::asking`]).
    pub fn asking_cell_types(self: &Arc<Self>) -> CellTypesHook {
        let announce: Announce = Arc::new(|_: &[Function]| {});
        self.cell_types_hook(Arc::default(), &announce)
    }

    /// What a closure made here puts in a cell it captures, asked of its
    /// walk (see [`CellTypesHook`]): walked first, as a call's callee is.
    /// Each answer is recorded in `answers`: a walk kept for a later round
    /// stands only while the closures' walks there give the same (see
    /// [`standing_cell_answers`]).
    fn cell_types_hook(
        self: &Arc<Self>,
        answers: Arc<Mutex<CellAnswers>>,
        announce: &Announce,
    ) -> CellTypesHook {
        let (round, announce) = (self.clone(), announce.clone());
        Arc::new(move |descriptor, pc, state, slot| {
            let function = Function {
                descriptor: Some(descriptor),
                pc,
                state,
            };
            let answer = round.cell_types(function, slot, &announce, &mut BTreeSet::new());
            if let Ok(mut answers) = answers.lock() {
                answers.insert((function, slot), answer);
            }
            answer
        })
    }

    /// The types `function` puts in its captured cell of `slot`, with every
    /// function it makes that captures the slot (the same cell): `None` when
    /// a walk of one stopped, or waits for the walk asking.
    fn cell_types(
        self: &Arc<Self>,
        function: Function,
        slot: u32,
        announce: &Announce,
        seen: &mut BTreeSet<Function>,
    ) -> Option<u16> {
        if !seen.insert(function) {
            return Some(0);
        }
        let walked = self.walk_unless_waiting_on_itself(function, announce)?;
        let report = walked.report.as_ref().ok()?;
        if report.stop != PrefixStopReason::Completed {
            return None;
        }
        let mut types = report.cell_types.get(&slot).copied().unwrap_or(0);
        for created in created_functions(report) {
            if captured_slots(created, &self.descriptors).contains(&slot) {
                types |= self.cell_types(created, slot, announce, seen)?;
            }
        }
        Some(types)
    }

    /// The hook one walk asks with, recording its answers in `answers`.
    fn hook(self: &Arc<Self>, answers: Arc<Mutex<Answers>>, announce: &Announce) -> ReturnsHook {
        let (round, announce) = (self.clone(), announce.clone());
        Arc::new(move |descriptor, pc, state| {
            let function = Function {
                descriptor: Some(descriptor),
                pc,
                state,
            };
            let answer = match round.walk_unless_waiting_on_itself(function, &announce) {
                Some(walked) => walked.makes,
                None => round.makers.contains(&function),
            };
            if let Ok(mut answers) = answers.lock() {
                answers.insert(function, answer);
            }
            answer
        })
    }
}

/// The walks of `walks` whose cell-type answers (see [`CellAnswers`]) the
/// others give: a walk told what a closure puts in a cell stands only while
/// that closure's walk, and those of the closures it makes holding the
/// cell, are kept and give the same. One that does not is walked again in
/// the round, asking the closures' walks of that round, so a walk never
/// decides on what a closure's earlier walk put in a cell.
pub fn standing_cell_answers(
    mut walks: BTreeMap<Function, Arc<Walked>>,
    descriptors: &[DescriptorRecord],
) -> BTreeMap<Function, Arc<Walked>> {
    loop {
        let fallen: Vec<Function> = walks
            .iter()
            .filter(|(_, walked)| {
                walked.cell_answers.iter().any(|(&(closure, slot), &answer)| {
                    kept_cell_types(&walks, descriptors, closure, slot, &mut BTreeSet::new())
                        != Some(answer)
                })
            })
            .map(|(&function, _)| function)
            .collect();
        if fallen.is_empty() {
            return walks;
        }
        for function in fallen {
            walks.remove(&function);
        }
    }
}

/// [`Round::cell_types`] from the walks of `walks` alone: `None` (the outer
/// one) when a walk it needs is not there.
fn kept_cell_types(
    walks: &BTreeMap<Function, Arc<Walked>>,
    descriptors: &[DescriptorRecord],
    function: Function,
    slot: u32,
    seen: &mut BTreeSet<Function>,
) -> Option<Option<u16>> {
    if !seen.insert(function) {
        return Some(Some(0));
    }
    let report = match &walks.get(&function)?.report {
        Ok(report) if report.stop == PrefixStopReason::Completed => report,
        _ => return Some(None),
    };
    let mut types = report.cell_types.get(&slot).copied().unwrap_or(0);
    for created in created_functions(report) {
        if captured_slots(created, descriptors).contains(&slot) {
            match kept_cell_types(walks, descriptors, created, slot, seen)? {
                Some(more) => types |= more,
                None => return Some(None),
            }
        }
    }
    Some(Some(types))
}

/// A hook telling only `makers`, walking nothing.
pub fn told(makers: Arc<Makers>) -> ReturnsHook {
    Arc::new(move |descriptor, pc, state| {
        makers.contains(&Function {
            descriptor: Some(descriptor),
            pc,
            state,
        })
    })
}

/// The makers `walks` found.
pub fn makers(walks: &BTreeMap<Function, Arc<Walked>>) -> Makers {
    walks
        .iter()
        .filter(|(_, walked)| walked.makes)
        .map(|(&function, _)| function)
        .collect()
}

/// Whether a walk given `answers` is given the same by `makers`.
pub fn stand(answers: &Answers, makers: &Makers) -> bool {
    answers
        .iter()
        .all(|(function, &answer)| makers.contains(function) == answer)
}

/// Whether the walk completed, and every return gives a VM function made.
pub fn makes_functions(result: &PrefixTraceReport) -> bool {
    if result.stop != crate::PrefixStopReason::Completed {
        return false;
    }
    let arena = &result.trace.arena;
    let lets: BTreeMap<&str, crate::TraceNodeId> = arena
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            TraceStatementKind::Let { name, value } => Some((name.as_str(), *value)),
            _ => None,
        })
        .collect();
    let made = |mut node: crate::TraceNodeId| {
        for _ in 0..64 {
            match arena.nodes.get(node as usize).map(|node| &node.kind) {
                Some(TraceNodeKind::VmFunction { .. }) => return true,
                Some(TraceNodeKind::Identifier { name }) => match lets.get(name.as_str()) {
                    Some(&value) => node = value,
                    None => return false,
                },
                _ => return false,
            }
        }
        false
    };
    // A VM subroutine's `return` is the subroutine's: its value goes back
    // to the call inside the VM function, not out of the VM function.
    let mut subroutine = BTreeSet::new();
    let mut pending: Vec<crate::TraceStatementId> = arena
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            TraceStatementKind::Function { body, .. } => Some(body.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    while let Some(id) = pending.pop() {
        if !subroutine.insert(id) {
            continue;
        }
        match arena.statements.get(id as usize).map(|statement| &statement.kind) {
            Some(TraceStatementKind::If {
                taken,
                fall_through,
                ..
            }) => pending.extend(taken.iter().chain(fall_through)),
            Some(
                TraceStatementKind::Loop { body, .. } | TraceStatementKind::Function { body, .. },
            ) => pending.extend(body),
            Some(TraceStatementKind::Try {
                body,
                handler,
                finalizer,
                ..
            }) => pending.extend(body.iter().chain(handler).chain(finalizer.iter().flatten())),
            _ => {}
        }
    }
    let mut returns = arena
        .statements
        .iter()
        .filter(|statement| !subroutine.contains(&statement.id))
        .filter_map(|statement| match &statement.kind {
            TraceStatementKind::Return { value } => Some(*value),
            _ => None,
        });
    let mut any = false;
    returns.all(|value| {
        any = true;
        made(value)
    }) && any
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

    fn contents(cells: &[(u32, &[usize], bool)]) -> BTreeMap<u32, CellContent> {
        cells
            .iter()
            .map(|&(slot, addresses, data)| {
                (
                    slot,
                    CellContent {
                        addresses: addresses.iter().copied().collect(),
                        data,
                    },
                )
            })
            .collect()
    }

    fn reads(cells: &[u32], data: &[u32], sets: &[u32]) -> CellReads {
        CellReads {
            cells: cells.iter().copied().collect(),
            data: data.iter().copied().collect(),
            sets: sets.iter().copied().collect(),
        }
    }

    /// A walk that stopped (no cell types), told `cell_answers`.
    fn stopped(cell_answers: &[(usize, u32, Option<u16>)]) -> Arc<Walked> {
        Arc::new(Walked {
            report: Err("stopped".into()),
            answers: Answers::new(),
            cell_answers: cell_answers
                .iter()
                .map(|&(closure, slot, answer)| ((function(closure), slot), answer))
                .collect(),
            makes: false,
            contents: BTreeMap::new(),
        })
    }

    #[test]
    fn a_walk_told_what_a_closure_puts_in_a_cell_falls_with_the_closures_walk() {
        // 1 was told what 2 puts in cell 3; 2 is walked again (not kept).
        let walks = BTreeMap::from([(function(1), stopped(&[(2, 3, Some(1))]))]);
        assert!(standing_cell_answers(walks, &[]).is_empty());
    }

    #[test]
    fn a_walk_told_what_a_kept_closure_puts_in_a_cell_stands() {
        // 2's kept walk stopped: what it puts in cell 3 is not known, as 1
        // was told.
        let walks = BTreeMap::from([
            (function(1), stopped(&[(2, 3, None)])),
            (function(2), stopped(&[])),
        ]);
        assert_eq!(standing_cell_answers(walks, &[]).len(), 2);
    }

    #[test]
    fn a_walk_falls_with_a_closure_walk_that_falls() {
        // 2 was told of 4, walked again; so 2 is, and 1 told of 2 with it.
        let walks = BTreeMap::from([
            (function(1), stopped(&[(2, 3, None)])),
            (function(2), stopped(&[(4, 3, None)])),
            (function(5), stopped(&[])),
        ]);
        let standing = standing_cell_answers(walks, &[]);
        assert_eq!(standing.keys().copied().collect::<Vec<_>>(), vec![function(5)]);
    }

    #[test]
    fn a_walk_stands_for_the_contents_it_was_walked_with() {
        let used = contents(&[(3, &[10], false)]);
        assert!(stands_for(&used, None, &used));
    }

    #[test]
    fn a_completed_walk_stands_where_it_read_nothing_that_changed() {
        let read = reads(&[3], &[3], &[3]);
        let used = contents(&[(3, &[10], false)]);
        // Only cell 5, which it never read, has contents now.
        let given = contents(&[(3, &[10], false), (5, &[20], true)]);
        assert!(stands_for(&used, Some(&read), &given));
    }

    #[test]
    fn a_completed_walk_is_walked_again_when_a_set_it_read_changes() {
        let read = reads(&[3], &[], &[3]);
        let used = BTreeMap::new();
        let given = contents(&[(3, &[10, 11], false)]);
        assert!(!stands_for(&used, Some(&read), &given));
    }

    #[test]
    fn a_completed_walk_is_walked_again_when_a_read_turns_out_data() {
        let read = reads(&[3], &[3], &[]);
        let used = BTreeMap::new();
        let given = contents(&[(3, &[], true)]);
        assert!(!stands_for(&used, Some(&read), &given));
    }

    #[test]
    fn a_stopped_walk_is_walked_again_with_any_new_contents() {
        // It read no cell, yet it stopped: the contents may be what it lacked.
        let used = BTreeMap::new();
        let given = contents(&[(5, &[20], false)]);
        assert!(!stands_for(&used, None, &given));
        assert!(stands_for(&used, Some(&reads(&[], &[], &[])), &given));
    }

    #[test]
    fn a_told_hook_answers_from_its_makers() {
        let hook = told(Arc::new(Makers::from([function(1)])));
        assert!(hook(1, 1, 0));
        assert!(!hook(2, 2, 0));
    }

    #[test]
    fn a_walk_stands_while_its_answers_do() {
        let answers = Answers::from([(function(1), true), (function(2), false)]);
        assert!(stand(&answers, &Makers::from([function(1), function(3)])));
        assert!(!stand(&answers, &Makers::from([function(1), function(2)])));
        assert!(!stand(&answers, &Makers::new()));
    }
}
