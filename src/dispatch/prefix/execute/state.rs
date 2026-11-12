use super::*;

#[derive(Debug, Clone)]
pub(super) struct Cursor {
    pub(super) pc: usize,
    pub(super) state: usize,
    /// Shared until a patch writes it.
    pub(super) image: Arc<Vec<u8>>,
    /// Bytecode bytes the histories merged into this path may not agree
    /// on: a fork's sides patched them differently. The image is one
    /// representative; reading one of these stops the walk. A patch every
    /// history makes clears it.
    pub(super) split_image: BTreeSet<usize>,
    pub(super) visited: Visited,
}

/// The positions a path has passed. Those before its forks are frozen,
/// shared layers, so forking copies no positions and a join merges only what
/// each side visited since the fork.
#[derive(Debug, Clone, Default)]
pub(super) struct Visited {
    pub(super) layers: Vec<Arc<Positions>>,
    pub(super) local: Positions,
}

/// Positions in the order they were first passed.
#[derive(Debug, Clone, Default)]
pub(super) struct Positions {
    /// `None` where a position was forgotten since.
    pub(super) order: Vec<Option<VisitKey>>,
    /// Each position and its index in `order`.
    pub(super) set: BTreeMap<VisitKey, usize>,
}

impl Positions {
    pub(super) fn insert(&mut self, key: VisitKey) -> bool {
        if self.set.contains_key(&key) {
            return false;
        }
        self.set.insert(key.clone(), self.order.len());
        self.order.push(Some(key));
        true
    }

    pub(super) fn contains(&self, key: &VisitKey) -> bool {
        self.set.contains_key(key)
    }

    /// Whether `key` is among the first `count` positions.
    pub(super) fn contains_before(&self, count: usize, key: &VisitKey) -> bool {
        self.set.get(key).is_some_and(|&index| index < count)
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = &VisitKey> {
        self.order.iter().flatten()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.set.is_empty()
    }
}

/// The records of the walk in the order they were made, some of them
/// dropped since: a side cut back to an earlier step dropped what it made
/// after it, while the other side's records, made in between, stay.
#[derive(Debug)]
pub(super) struct Log<T> {
    pub(super) items: Vec<T>,
    /// Sorted, disjoint index ranges of dropped items.
    pub(super) dropped: Vec<std::ops::Range<usize>>,
    /// How many items are kept: neither dropped nor truncated.
    pub(super) live: usize,
}

impl<T> Default for Log<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            dropped: Vec::new(),
            live: 0,
        }
    }
}

impl<T> Log<T> {
    pub(super) fn len(&self) -> usize {
        self.items.len()
    }

    pub(super) fn push(&mut self, item: T) {
        self.items.push(item);
        self.live += 1;
    }

    /// How many items are kept.
    pub(super) fn live(&self) -> usize {
        self.live
    }

    /// How many items of `range` are kept.
    pub(super) fn live_in(&self, range: std::ops::Range<usize>) -> usize {
        let first = self
            .dropped
            .partition_point(|dropped| dropped.end <= range.start);
        let dropped: usize = self.dropped[first..]
            .iter()
            .take_while(|dropped| dropped.start < range.end)
            .map(|dropped| dropped.end.min(range.end) - dropped.start.max(range.start))
            .sum();
        range.len() - dropped
    }

    pub(super) fn truncate(&mut self, len: usize) {
        if len < self.items.len() {
            self.live -= self.live_in(len..self.items.len());
        }
        self.items.truncate(len);
        self.dropped.retain_mut(|range| {
            range.end = range.end.min(len);
            range.start < range.end
        });
    }

    pub(super) fn clear(&mut self) {
        self.truncate(0);
    }

    /// Take the kept items from `at` on out of the log.
    pub(super) fn split_off(&mut self, at: usize) -> Vec<T> {
        self.live -= self.live_in(at..self.items.len());
        let first = self.dropped.partition_point(|dropped| dropped.end <= at);
        let dropped = self.dropped.split_off(first);
        // A dropped range across `at` stays dropped below it.
        if let Some(range) = dropped.first().filter(|range| range.start < at) {
            self.dropped.push(range.start..at);
        }
        let mut dropped = dropped.into_iter().peekable();
        self.items
            .drain(at..)
            .enumerate()
            .filter(|(offset, _)| {
                let index = at + offset;
                while dropped.peek().is_some_and(|range| range.end <= index) {
                    dropped.next();
                }
                !dropped.peek().is_some_and(|range| range.contains(&index))
            })
            .map(|(_, item)| item)
            .collect()
    }

    pub(super) fn drop_range(&mut self, range: std::ops::Range<usize>) {
        let range = range.start..range.end.min(self.items.len());
        if range.start >= range.end {
            return;
        }
        self.live -= self.live_in(range.clone());
        // Merged with the ranges it touches, so they stay disjoint.
        let first = self
            .dropped
            .partition_point(|dropped| dropped.end < range.start);
        let last = self
            .dropped
            .partition_point(|dropped| dropped.start <= range.end);
        let merged = self.dropped[first..last]
            .iter()
            .fold(range, |merged, dropped| {
                merged.start.min(dropped.start)..merged.end.max(dropped.end)
            });
        self.dropped.splice(first..last, [merged]);
    }

    pub(super) fn into_kept(self) -> Vec<T> {
        let mut dropped = self.dropped.into_iter().peekable();
        self.items
            .into_iter()
            .enumerate()
            .filter(|(index, _)| {
                while dropped.peek().is_some_and(|range| range.end <= *index) {
                    dropped.next();
                }
                !dropped.peek().is_some_and(|range| range.contains(index))
            })
            .map(|(_, item)| item)
            .collect()
    }
}

impl<T> std::ops::Index<usize> for Log<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        &self.items[index]
    }
}

impl<T> std::ops::IndexMut<usize> for Log<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.items[index]
    }
}

/// How far a path had come: its frozen layers and the positions after them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VisitedMark {
    pub(super) layers: usize,
    pub(super) local: usize,
}

impl Visited {
    pub(super) fn contains(&self, key: &VisitKey) -> bool {
        self.local.contains(key) || self.layers.iter().any(|layer| layer.contains(key))
    }

    /// Record a position; false when it was already passed.
    pub(super) fn insert(&mut self, key: VisitKey) -> bool {
        !self.contains(&key) && self.local.insert(key)
    }

    /// Forget that the path passed `key` since its last fork.
    pub(super) fn forget(&mut self, key: &VisitKey) {
        if let Some(index) = self.local.set.remove(key) {
            self.local.order[index] = None;
        }
    }

    /// Freeze the positions so far into a shared layer, before the path is
    /// copied into the sides of a fork.
    pub(super) fn freeze(&mut self) {
        if !self.local.is_empty() {
            self.layers.push(Arc::new(std::mem::take(&mut self.local)));
        }
    }

    /// Whether the position was passed after the first `base` layers.
    pub(super) fn contains_since(&self, base: usize, key: &VisitKey) -> bool {
        self.local.contains(key)
            || self
                .layers
                .iter()
                .skip(base)
                .any(|layer| layer.contains(key))
    }

    /// Whether the position had been passed by `mark`.
    pub(super) fn contains_at(&self, mark: VisitedMark, key: &VisitKey) -> bool {
        self.contains_since_until(0, mark, key)
    }

    /// [`Self::contains_since`], counting only what was passed by `mark`.
    pub(super) fn contains_since_until(
        &self,
        base: usize,
        mark: VisitedMark,
        key: &VisitKey,
    ) -> bool {
        let below = self.layers.iter().take(mark.layers).skip(base);
        if below.into_iter().any(|layer| layer.contains(key)) {
            return true;
        }
        match self.layers.get(mark.layers) {
            Some(frozen) => frozen.contains_before(mark.local, key),
            None => self.local.contains_before(mark.local, key),
        }
    }

    /// The positions passed after `key`, in order.
    pub(super) fn after(&self, key: &VisitKey) -> Vec<VisitKey> {
        let all: Vec<&Positions> = self
            .layers
            .iter()
            .map(|layer| &**layer)
            .chain(std::iter::once(&self.local))
            .collect();
        let Some((at, index)) = all
            .iter()
            .enumerate()
            .find_map(|(at, positions)| positions.set.get(key).map(|&index| (at, index)))
        else {
            return Vec::new();
        };
        all[at].order[index + 1..]
            .iter()
            .chain(
                all[at + 1..]
                    .iter()
                    .flat_map(|positions| positions.order.iter()),
            )
            .flatten()
            .cloned()
            .collect()
    }

    pub(super) fn mark(&self) -> VisitedMark {
        VisitedMark {
            layers: self.layers.len(),
            local: self.local.order.len(),
        }
    }

    /// Back to where the path stood at `mark`: what it passed since is
    /// forgotten. A layer frozen since holds the positions the mark counts.
    pub(super) fn rewind(&mut self, mark: VisitedMark) {
        if self.layers.len() > mark.layers {
            let frozen = self.layers[mark.layers].clone();
            let mut local = Positions::default();
            for entry in frozen.order.iter().take(mark.local) {
                match entry {
                    Some(key) => {
                        local.set.insert(key.clone(), local.order.len());
                        local.order.push(Some(key.clone()));
                    }
                    None => local.order.push(None),
                }
            }
            self.local = local;
            self.layers.truncate(mark.layers);
            return;
        }
        for key in self.local.order.drain(mark.local..).flatten() {
            self.local.set.remove(&key);
        }
    }

    /// `base` (frozen) plus everything either side visited since.
    pub(super) fn join(base: &Self, sides: [Self; 2]) -> Self {
        let mut joined = base.clone();
        for side in sides {
            for layer in side.layers.into_iter().skip(base.layers.len()) {
                for key in layer.keys() {
                    joined.local.insert(key.clone());
                }
            }
            for key in side.local.keys() {
                joined.local.insert(key.clone());
            }
        }
        joined
    }
}

/// Where an enclosing fork's stepping side would meet the other: its known
/// meeting point, or else any position the other side passed since the fork
/// (`other` past its first `base` layers).
pub(super) struct Target {
    pub(super) meet: Option<VisitKey>,
    pub(super) other: Visited,
    pub(super) base: usize,
    /// Only what the other side had passed by this mark counts.
    pub(super) until: Option<VisitedMark>,
    /// The fork's `if` statement.
    pub(super) statement: Option<TraceStatementId>,
}

impl Target {
    pub(super) fn reaches(&self, at: &VisitKey) -> bool {
        match &self.meet {
            Some(meet) => meet == at,
            None => match self.until {
                Some(mark) => self.other.contains_since_until(self.base, mark, at),
                None => self.other.contains_since(self.base, at),
            },
        }
    }
}

/// One successor of a fork: its cursor, its trace path, and whether it has
/// ended.
pub(super) struct Side {
    pub(super) cursor: Cursor,
    pub(super) path: TracePath,
    pub(super) ended: bool,
    /// Where the side stood before each of its steps, while a fork steps
    /// it: what it can be cut back to.
    pub(super) history: Vec<SideStep>,
    /// The side stands at a fork of its own, waiting for the other to come
    /// to a meeting point or a fork (see [`Walk::patient`]).
    pub(super) held: bool,
}

/// What a walk learned: see [`Walk::learned`].
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Learned {
    /// Positions known to be loop headers.
    pub(super) headers: BTreeSet<VisitKey>,
    /// Where the two sides of each conditional jump meet, once found, by the
    /// jump's fall-through position. Kept across re-traces and restarts.
    pub(super) meets: BTreeMap<VisitKey, VisitKey>,
    /// The cells found to change in each loop, by header, and in each `try`
    /// body, by entry position. A re-trace of an enclosing loop or `try`
    /// starts from them.
    pub(super) widened: BTreeMap<VisitKey, BTreeSet<crate::TraceCell>>,
    /// The shared cells each loop's iterations write a VM position or a
    /// value to, by header (see `TraceSession::loop_cell_changes`).
    pub(super) loop_cells: BTreeMap<VisitKey, BTreeMap<u32, crate::tracing::symbolic::CellMark>>,
    /// The shared cells each loop's iterations write at all, by header (see
    /// `TraceSession::loop_cell_writes`): what the path concretely put there
    /// before the loop is not what a later iteration finds.
    pub(super) loop_written: BTreeMap<VisitKey, BTreeSet<u32>>,
    /// The made objects each loop's iterations write or hand on, by header
    /// (see `TraceSession::loop_object_writes`).
    pub(super) loop_objects: BTreeMap<VisitKey, BTreeSet<crate::TraceNodeId>>,
    /// The shared cells each `try` body writes, by entry position: what the
    /// path concretely put there before is not what the catch finds.
    pub(super) try_cells: BTreeMap<VisitKey, BTreeSet<u32>>,
    /// Cells each fork's sides must agree on before they join: they held
    /// different concrete values that later decided a VM jump or index,
    /// with where that value was read. Once both sides have passed every
    /// such read, the cell no longer keeps them apart.
    pub(super) apart: BTreeMap<VisitKey, BTreeMap<TraceCell, BTreeSet<VisitKey>>>,
    /// Bytecode bytes each fork's sides patched differently that a later
    /// step read: the sides are kept apart while their images differ there.
    /// Real code patches no byte differently on the two sides of an `if`,
    /// so each side runs on its own image until one of them shows it is no
    /// code that runs.
    pub(super) apart_bytes: BTreeMap<VisitKey, BTreeSet<usize>>,
    /// Where paths left each loop, by header, in the attempts so far.
    pub(super) breaks: BTreeMap<VisitKey, BTreeSet<VisitKey>>,
    /// Positions that are not loop exits after all: a path that left the
    /// loop there came back to its header.
    pub(super) no_break: BTreeSet<(VisitKey, VisitKey)>,
    /// Forks whose sides do not join by lining their stacks up from the top
    /// (see `TraceSession::pad_stack`): a later step counted a position from
    /// the bottom, or a later join failed, on the padded path.
    pub(super) unpadded: BTreeSet<VisitKey>,
    /// The positions each loop's body passed, by header.
    pub(super) bodies: BTreeMap<VisitKey, BTreeSet<VisitKey>>,
    /// How often each loop header was found passed by a stale trace.
    pub(super) stale_headers: BTreeMap<VisitKey, usize>,
    /// Forks a side of which is known fake (true: the taken one), and why.
    pub(super) forced: BTreeMap<VisitKey, (bool, String)>,
    /// The values each loop variable was seen to hold at a `continue`, by
    /// header: `None` when they are not a known set of concrete values.
    pub(super) loop_values: BTreeMap<VisitKey, BTreeMap<TraceCell, Option<Vec<SymbolicValue>>>>,
    /// The types each loop variable was seen to hold at a `continue`.
    pub(super) loop_types: BTreeMap<VisitKey, BTreeMap<TraceCell, u16>>,
    /// The values each `try` body was seen to assign its `try` variables, by
    /// the `try`'s entry position.
    pub(super) try_values: BTreeMap<VisitKey, BTreeMap<TraceCell, Option<Vec<SymbolicValue>>>>,
    /// The types each `try` body was seen to assign its `try` variables.
    pub(super) try_types: BTreeMap<VisitKey, BTreeMap<TraceCell, u16>>,
    /// Whether a throw each `try`'s catch takes may come while a `try`
    /// variable still holds its entry value (see
    /// `TraceSession::try_entry_exposed`); once `true`, it stays.
    pub(super) try_entry_exposed: BTreeMap<VisitKey, BTreeMap<TraceCell, bool>>,
    /// Where the body and catch of each `try` meet, by the `try`'s entry
    /// position, once the catch reached code the body had passed.
    pub(super) try_meets: BTreeMap<VisitKey, VisitKey>,
    /// `try`s whose body is walked to its close before the catch steps: the
    /// catch needed a VM value the body may write, and the body had not
    /// written it yet. The VM runs the body first, so the catch starts from
    /// the entry with what the body wrote laid over it.
    pub(super) body_first: BTreeSet<VisitKey>,
    /// Call sites traced inline after all: a test or VM value depended on
    /// what the call returned.
    pub(super) inline_calls: BTreeSet<VisitKey>,
}

impl Learned {
    /// What two sides that started from `start` learned between them,
    /// whichever stepped first: what either added, less what either took
    /// out; where both changed one entry, the sets join and a position is
    /// the lower one.
    pub(super) fn merged(start: &Arc<Self>, a: Arc<Self>, b: Arc<Self>) -> Arc<Self> {
        if Arc::ptr_eq(start, &a) {
            return b;
        }
        if Arc::ptr_eq(start, &b) {
            return a;
        }
        let set = |s: Option<&BTreeSet<VisitKey>>,
                   a: Option<&BTreeSet<VisitKey>>,
                   b: Option<&BTreeSet<VisitKey>>| {
            let empty = BTreeSet::new();
            Some(merge_sets(
                s.unwrap_or(&empty),
                a.unwrap_or(&empty),
                b.unwrap_or(&empty),
            ))
            .filter(|set| !set.is_empty())
        };
        let lower = |_: Option<&VisitKey>, a: Option<&VisitKey>, b: Option<&VisitKey>| {
            a.into_iter().chain(b).min().cloned()
        };
        Arc::new(Self {
            headers: merge_sets(&start.headers, &a.headers, &b.headers),
            meets: merge_maps(&start.meets, &a.meets, &b.meets, lower),
            widened: merge_maps(&start.widened, &a.widened, &b.widened, |s, a, b| {
                let empty = BTreeSet::new();
                Some(merge_sets(
                    s.unwrap_or(&empty),
                    a.unwrap_or(&empty),
                    b.unwrap_or(&empty),
                ))
            }),
            loop_cells: merge_maps(
                &start.loop_cells,
                &a.loop_cells,
                &b.loop_cells,
                |s, a, b| {
                    let empty = BTreeMap::new();
                    Some(merge_maps(
                        s.unwrap_or(&empty),
                        a.unwrap_or(&empty),
                        b.unwrap_or(&empty),
                        |_, a, b| match (a, b) {
                            (Some(a), Some(b)) => Some(a.join(*b)),
                            (a, b) => a.or(b).copied(),
                        },
                    ))
                },
            ),
            loop_written: merge_maps(
                &start.loop_written,
                &a.loop_written,
                &b.loop_written,
                |s, a, b| {
                    let empty = BTreeSet::new();
                    Some(merge_sets(
                        s.unwrap_or(&empty),
                        a.unwrap_or(&empty),
                        b.unwrap_or(&empty),
                    ))
                },
            ),
            loop_objects: merge_maps(
                &start.loop_objects,
                &a.loop_objects,
                &b.loop_objects,
                |s, a, b| {
                    let empty = BTreeSet::new();
                    Some(merge_sets(
                        s.unwrap_or(&empty),
                        a.unwrap_or(&empty),
                        b.unwrap_or(&empty),
                    ))
                },
            ),
            try_cells: merge_maps(&start.try_cells, &a.try_cells, &b.try_cells, |s, a, b| {
                let empty = BTreeSet::new();
                Some(merge_sets(
                    s.unwrap_or(&empty),
                    a.unwrap_or(&empty),
                    b.unwrap_or(&empty),
                ))
            }),
            apart_bytes: merge_maps(
                &start.apart_bytes,
                &a.apart_bytes,
                &b.apart_bytes,
                |s, a, b| {
                    let empty = BTreeSet::new();
                    Some(merge_sets(
                        s.unwrap_or(&empty),
                        a.unwrap_or(&empty),
                        b.unwrap_or(&empty),
                    ))
                },
            ),
            apart: merge_maps(&start.apart, &a.apart, &b.apart, |s, a, b| {
                let empty = BTreeMap::new();
                Some(merge_maps(
                    s.unwrap_or(&empty),
                    a.unwrap_or(&empty),
                    b.unwrap_or(&empty),
                    set,
                ))
            }),
            breaks: merge_maps(&start.breaks, &a.breaks, &b.breaks, set),
            no_break: merge_sets(&start.no_break, &a.no_break, &b.no_break),
            unpadded: merge_sets(&start.unpadded, &a.unpadded, &b.unpadded),
            bodies: merge_maps(&start.bodies, &a.bodies, &b.bodies, set),
            stale_headers: merge_maps(
                &start.stale_headers,
                &a.stale_headers,
                &b.stale_headers,
                |_, a, b| a.into_iter().chain(b).max().copied(),
            ),
            forced: merge_maps(&start.forced, &a.forced, &b.forced, |_, a, b| {
                a.into_iter().chain(b).min().cloned()
            }),
            loop_values: merge_maps(&start.loop_values, &a.loop_values, &b.loop_values, values),
            loop_types: merge_maps(&start.loop_types, &a.loop_types, &b.loop_types, types),
            try_values: merge_maps(&start.try_values, &a.try_values, &b.try_values, values),
            try_types: merge_maps(&start.try_types, &a.try_types, &b.try_types, types),
            try_entry_exposed: merge_maps(
                &start.try_entry_exposed,
                &a.try_entry_exposed,
                &b.try_entry_exposed,
                exposures,
            ),
            try_meets: merge_maps(&start.try_meets, &a.try_meets, &b.try_meets, lower),
            body_first: merge_sets(&start.body_first, &a.body_first, &b.body_first),
            inline_calls: merge_sets(&start.inline_calls, &a.inline_calls, &b.inline_calls),
        })
    }
}

pub(super) type CellValues = BTreeMap<TraceCell, Option<Vec<SymbolicValue>>>;

/// Two sides' learned entry exposures of the same variables: exposed on
/// either side is exposed.
pub(super) fn exposures<K: Ord + Copy>(
    _: Option<&BTreeMap<K, bool>>,
    a: Option<&BTreeMap<K, bool>>,
    b: Option<&BTreeMap<K, bool>>,
) -> Option<BTreeMap<K, bool>> {
    let mut exposed = a.cloned().unwrap_or_default();
    for (cell, seen) in b.into_iter().flatten() {
        *exposed.entry(*cell).or_default() |= seen;
    }
    Some(exposed)
}

/// Two sides' learned types of the same variables: each cell's union.
pub(super) fn types<K: Ord + Copy>(
    _: Option<&BTreeMap<K, u16>>,
    a: Option<&BTreeMap<K, u16>>,
    b: Option<&BTreeMap<K, u16>>,
) -> Option<BTreeMap<K, u16>> {
    let mut types = a.cloned().unwrap_or_default();
    for (cell, bits) in b.into_iter().flatten() {
        *types.entry(*cell).or_default() |= bits;
    }
    Some(types)
}

/// Whether `values` adds to the value sets `known` holds: a cell it has no
/// set for, a value its set lacks, or a set that becomes unknown. Asked
/// before the learned facts are copied to change them.
pub(super) fn adds_values<'a>(
    known: Option<&CellValues>,
    values: impl IntoIterator<Item = (&'a TraceCell, &'a Option<Vec<SymbolicValue>>)>,
) -> bool {
    values.into_iter().any(|(cell, values)| {
        match (known.and_then(|known| known.get(cell)), values) {
            (Some(None), _) => false,
            (Some(Some(set)), Some(values)) => values.iter().any(|value| !set.contains(value)),
            (Some(Some(_)), None) | (None, _) => true,
        }
    })
}

/// Join `values` into the value sets `known` holds.
pub(super) fn join_values(
    known: &mut CellValues,
    values: impl IntoIterator<Item = (TraceCell, Option<Vec<SymbolicValue>>)>,
) {
    for (cell, values) in values {
        match (known.get_mut(&cell), values) {
            (Some(None), _) => {}
            (Some(Some(set)), Some(values)) => {
                for value in values {
                    if !set.contains(&value) {
                        set.push(value);
                    }
                }
            }
            (Some(Some(_)), None) => {
                known.insert(cell, None);
            }
            (None, values) => {
                known.insert(cell, values);
            }
        }
    }
}

/// The values two sides saw each cell hold, both having added some: an
/// unknown set stays unknown; otherwise the values `start` had, then those
/// either side added, in one order whichever side came first.
pub(super) fn values(
    start: Option<&CellValues>,
    a: Option<&CellValues>,
    b: Option<&CellValues>,
) -> Option<CellValues> {
    let empty = BTreeMap::new();
    let (start, a, b) = (
        start.unwrap_or(&empty),
        a.unwrap_or(&empty),
        b.unwrap_or(&empty),
    );
    Some(merge_maps(start, a, b, |start, a, b| {
        let (Some(Some(a)), Some(Some(b))) = (a, b) else {
            return Some(None);
        };
        let mut values: Vec<SymbolicValue> = start.cloned().flatten().unwrap_or_default();
        let mut added: Vec<&SymbolicValue> = a
            .iter()
            .chain(b)
            .filter(|value| !values.contains(value))
            .collect();
        added.sort_by_cached_key(|value| format!("{value:?}"));
        added.dedup();
        values.extend(added.into_iter().cloned());
        Some(Some(values))
    }))
}

/// `start` with what `a` and `b` each added, less what either took out.
pub(super) fn merge_sets<T: Ord + Clone>(
    start: &BTreeSet<T>,
    a: &BTreeSet<T>,
    b: &BTreeSet<T>,
) -> BTreeSet<T> {
    let mut merged: BTreeSet<T> = start
        .iter()
        .filter(|item| a.contains(item) && b.contains(item))
        .cloned()
        .collect();
    merged.extend(a.difference(start).cloned());
    merged.extend(b.difference(start).cloned());
    merged
}

/// `start` with each entry as the one side that changed it left it, or as
/// `both` makes it of the two (`None`: no entry).
pub(super) fn merge_maps<K: Ord + Clone, V: PartialEq + Clone>(
    start: &BTreeMap<K, V>,
    a: &BTreeMap<K, V>,
    b: &BTreeMap<K, V>,
    both: impl Fn(Option<&V>, Option<&V>, Option<&V>) -> Option<V>,
) -> BTreeMap<K, V> {
    let keys: BTreeSet<&K> = start.keys().chain(a.keys()).chain(b.keys()).collect();
    keys.into_iter()
        .filter_map(|key| {
            let (was, left, right) = (start.get(key), a.get(key), b.get(key));
            let value = if left == was {
                right.cloned()
            } else if right == was || left == right {
                left.cloned()
            } else {
                both(was, left, right)
            };
            value.map(|value| (key.clone(), value))
        })
        .collect()
}

/// What a fork knew at the start of one of its turns.
#[derive(Clone)]
pub(super) struct TurnState {
    pub(super) meet: Option<VisitKey>,
    pub(super) unmergeable: BTreeSet<VisitKey>,
    pub(super) apart_since: Option<(usize, Cursor, String)>,
    pub(super) stepped_apart: bool,
    pub(super) short_side: Option<bool>,
    pub(super) prestep: usize,
}

/// One step of a fork's side: where it stood and how it was before, and
/// what the step made, which cutting the side back to here drops.
pub(super) struct SideStep {
    pub(super) at: VisitKey,
    pub(super) path: TracePath,
    pub(super) block_length: usize,
    pub(super) pc: usize,
    pub(super) state: usize,
    pub(super) split_image: BTreeSet<usize>,
    pub(super) visited: VisitedMark,
    /// The fork's turn the step ran in.
    pub(super) turn: usize,
    /// The session's stop state before the step: a step that stopped is
    /// cut back past its stop.
    pub(super) stops: StopMarks,
    pub(super) made: Made,
}

/// Where the trace's statements, events, records and patches stood, or the
/// ones a step made.
#[derive(Debug, Clone, Default)]
pub(super) struct Made {
    pub(super) statements: std::ops::Range<usize>,
    pub(super) events: std::ops::Range<usize>,
    pub(super) records: std::ops::Range<usize>,
    pub(super) patches: std::ops::Range<usize>,
}

/// What one dispatch step did.
pub(super) enum Step {
    Continue,
    /// A conditional jump on a program value whose test is undecided, and
    /// the index of its record. The cursor is at the fall-through.
    Branch(PendingBranch, usize),
    /// The VM function returned, or the handler threw.
    End,
    /// The handler entered this `try`. The cursor is at the body.
    Try(TryFrame),
}

/// How a VM subroutine is traced at its calls.
#[derive(Clone)]
pub(super) enum Subroutine {
    /// Once, as a function each call calls, traced over this image, with
    /// the records of tracing it.
    Function(
        SubroutineSummary,
        Arc<Vec<u8>>,
        Arc<[PrefixExecutionRecord]>,
    ),
    /// Inline at every call: it does more than a function can, or it is
    /// being traced as a function right now.
    Inline,
}

/// Whether a path is still running after a step. A path ends by returning
/// or by continuing a loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Flow {
    Running,
    Ended,
}

/// Why a walk did not finish.
pub(super) enum Halt {
    Stop(PrefixStopReason),
    /// A path came back to a position it passed: a loop header the walk did
    /// not know yet (now in `headers`). The innermost fork, loop or `try`
    /// entered before the header was first passed traces itself again.
    NewLoop(VisitKey),
    /// A VM value (a jump target, a state, an index) was a variable the
    /// join of the fork at this position made from two different concrete
    /// values: the paths set up different jumps. The fork's sides are now
    /// kept apart while those cells differ; the innermost region entered
    /// before the fork traces itself again, as for `NewLoop`.
    Split(VisitKey),
    /// The catch path of a `try` stands where the body, past the `try`,
    /// already stepped: that is where they meet. The `try` traces itself
    /// again with the body waiting there.
    TryMeet(VisitKey),
    /// A subroutine being traced as a function branched or entered a
    /// `try`: it stays inline.
    NotFunction,
    /// An iteration of the loop at this header changed cells that are not
    /// loop variables yet: the attempt is traced again with them widened.
    Rewiden(VisitKey),
    /// A created closure captures these slots, which are not shared cells
    /// yet: the whole walk starts over with them as cells.
    Cells(BTreeSet<u32>),
    /// The side (taken when true) of the closed fork at this jump carried a
    /// program value out to a VM jump: the walk starts over with that side
    /// known fake, for this reason.
    ForceFake(VisitKey, bool, String),
    /// A step reached the holes of a join already undone: a path that
    /// kept them survived the undo (one a loop or fork saved before it).
    /// The whole walk starts over, knowing that join undone.
    Restart,
    /// The side reached a fork of its own while the walk is patient: the
    /// enclosing fork undoes the step and steps the other side.
    Held,
}

/// What the walk knows about VM memory cells shared between functions:
/// the slots each thread descriptor captures (`None`: unknown, closures get
/// no cells), and the incoming value of each slot this thread captured.
#[derive(Clone, Default)]
pub struct CellSharing {
    pub captures: Option<Arc<[Vec<u32>]>>,
    /// The slots each thread descriptor initializes to `undefined`.
    pub starts: Option<Arc<[Vec<(u32, bool)>]>>,
    pub captured: BTreeMap<u32, SymbolicValue>,
    /// What each shared cell of this function can hold, from every function
    /// that holds it, when the driver knows.
    pub contents: BTreeMap<u32, crate::CellContent>,
    /// Slots an earlier walk of this function found to be cells.
    pub cells: BTreeSet<u32>,
    /// Told each VM function the walk creates, in any attempt: the driver
    /// may start it while the walk goes on.
    pub created: Option<crate::tracing::symbolic::CreatedHook>,
    /// See [`crate::tracing::symbolic::ReturnsHook`].
    pub returns: Option<crate::tracing::symbolic::ReturnsHook>,
    /// See [`crate::tracing::symbolic::CellTypesHook`].
    pub cell_types: Option<crate::tracing::symbolic::CellTypesHook>,
    /// A walk that only checks a function's first instruction (see
    /// [`Walk::junk_entry`]): it checks no function it makes.
    pub probe: bool,
    /// Branches the user decides.
    pub overrides: Arc<crate::BranchOverrides>,
}

impl From<PrefixStopReason> for Halt {
    fn from(stop: PrefixStopReason) -> Self {
        Self::Stop(stop)
    }
}

/// Positions numbered in the order first met: the tags of joins and calls.
#[derive(Default)]
pub(super) struct Tags {
    pub(super) keys: Vec<VisitKey>,
    pub(super) index: BTreeMap<VisitKey, usize>,
}

impl Tags {
    /// The tag of `key`, a new one when it is new.
    pub(super) fn tag(&mut self, key: VisitKey) -> usize {
        if let Some(&tag) = self.index.get(&key) {
            return tag;
        }
        self.keys.push(key.clone());
        self.index.insert(key, self.keys.len() - 1);
        self.keys.len() - 1
    }

    pub(super) fn get(&self, tag: usize) -> Option<VisitKey> {
        self.keys.get(tag).cloned()
    }

    pub(super) fn len(&self) -> usize {
        self.keys.len()
    }

    pub(super) fn truncate(&mut self, len: usize) {
        for key in self.keys.drain(len.min(self.keys.len())..) {
            self.index.remove(&key);
        }
    }
}

/// An open loop: its header position and its trace.
pub(super) struct LoopFrame {
    pub(super) header: VisitKey,
    pub(super) trace: TraceLoop,
    /// A jump in this attempt dispatched on one of the loop's variables, so
    /// a value it gains later means the attempt missed a case.
    pub(super) dispatched: bool,
    /// The variables and cells whose types a decision in this attempt read
    /// (see `TraceSession::take_used_sets`): one that gains a type later may
    /// have had a test decided on too few.
    pub(super) used: BTreeSet<String>,
}

pub(super) struct Walk<'p> {
    pub(super) preparation: &'p PrefixPreparation,
    pub(super) session: TraceSession,
    /// Whether this walk is a [`Self::junk_entry`] check.
    pub(super) probe: bool,
    /// The slots each thread descriptor initializes (see
    /// [`CellSharing::starts`]): a VM function made here starts with them.
    pub(super) starts: Option<Arc<[Vec<(u32, bool)>]>>,
    /// Each function entry checked, and whether it is junk.
    pub(super) entries: BTreeMap<(usize, usize, usize), bool>,
    /// The junk entry the last stop was for (see [`Self::junk_entry`]).
    pub(super) last_junk: Option<(usize, usize, usize)>,
    pub(super) records: Log<PrefixExecutionRecord>,
    pub(super) patches: Log<ImagePatchRecord>,
    /// Dispatch steps taken across every path.
    pub(super) steps: usize,
    /// What the walk learned so far, shared until written: each side of a
    /// fork starts a turn from the same, and what both learned is merged
    /// after it.
    pub(super) learned: Arc<Learned>,
    /// The loops the running path is inside, innermost last.
    pub(super) loops: Vec<LoopFrame>,
    /// The fork positions whose joins are tagged, by tag.
    pub(super) joins: Tags,
    /// The VM subroutines met so far, by entry: traced once as a function,
    /// or left inline at every call.
    pub(super) subroutines: BTreeMap<(usize, usize), Subroutine>,
    /// The jump of each fork's `if` statement, by statement.
    pub(super) fork_jumps: BTreeMap<TraceStatementId, VisitKey>,
    /// The `try`s being traced, innermost last: each one's frame, statement
    /// and how many fork targets there were when its paths started (see
    /// `targets`).
    pub(super) open_tries: Vec<(crate::tracing::symbolic::TryFrame, TraceStatementId, usize)>,
    /// The handlers found to be `finally`s a `return` skips (see
    /// `TraceSession::skip_finally`).
    pub(super) finally_handlers: BTreeSet<(usize, usize)>,
    /// The `finally` handlers some path ran inline that a `return` skipped
    /// or the body joined in: emitted as `finally { }` they would run twice,
    /// so their `try`s keep the handler a `catch` that rethrows, and every
    /// path runs them inline.
    pub(super) plain_finally: BTreeSet<(usize, usize)>,
    /// How many times a path entered each `try`'s `finally` from its body
    /// with no `return` to skip it, by statement.
    pub(super) finally_inline: BTreeMap<TraceStatementId, usize>,
    /// The memory slots a `return` that skipped each `try`'s `finally` read
    /// after it, by statement: the `finally` must not write them.
    pub(super) finally_reads: BTreeMap<TraceStatementId, BTreeSet<u32>>,
    /// The memory slots each `try`'s handler wrote before rethrowing, by
    /// statement (see `TraceSession::finally_writes`).
    pub(super) finally_writes: BTreeMap<TraceStatementId, BTreeSet<u32>>,
    /// Variables whose value sets a VM value was taken from.
    pub(super) used_sets: BTreeSet<String>,
    /// The session's stop state before the running handler, to undo a stop
    /// the walk recovers from by dispatching.
    pub(super) last_marks: Option<StopMarks>,
    /// What made the walk trace work again.
    pub(super) retraces: PrefixRetraces,
    /// With `PrefixLimits::self_check`, every shortcut that gave a
    /// different result than tracing without it.
    pub(super) self_check: Vec<String>,
    /// The call sites of outlined calls, by tag.
    pub(super) calls: Tags,
    /// How many subroutines are being traced as functions around the
    /// running step.
    pub(super) summarizing: usize,
    /// A branch the last step decided fake by test: where its other side
    /// goes, and the branch's position, for [`PrefixLimits::audit_fakes`].
    pub(super) pending_audit: Option<((usize, usize), String)>,
    /// While a fake branch's side is audited, the step count it stops at:
    /// one audit step may trace whole regions.
    pub(super) audit_until: Option<usize>,
    /// The positions an audited side ran, to tell a loop with no exit from
    /// straight code at the bound.
    pub(super) audit_seen: std::collections::HashMap<(usize, usize), usize>,
    /// How many times the audited side started again after a retrace.
    pub(super) audit_restarts: usize,
    /// See [`PrefixTraceReport::fake_audits`].
    pub(super) fake_audits: BTreeMap<String, BTreeSet<String>>,
    /// See [`PrefixTraceReport::wasted_steps`].
    pub(super) wasted: BTreeMap<&'static str, usize>,
    /// The records of the VM subroutine just traced as a function, for its
    /// entry in `subroutines`.
    pub(super) summary_records: Vec<PrefixExecutionRecord>,
    /// How many `catch` paths the running step is inside.
    pub(super) catch_depth: usize,
    /// The `try` statement whose catch the path came through when the walk
    /// last stopped reading the stack a throw left.
    pub(super) catch_stop_try: Option<TraceStatementId>,
    /// How many fork sides the running step is inside that met the other
    /// side with fewer stack values and stepped on apart.
    pub(super) short_sides: usize,
    /// Branches the user decides (see [`crate::BranchOverrides`]).
    pub(super) overrides: Arc<crate::BranchOverrides>,
    /// A side of a branch the user made real reached a state real code
    /// never reaches: no branch takes evidence after, so the walk stops
    /// there.
    pub(super) no_evidence: bool,
    /// The forks whose sides a join lined up under a loop variable's stack
    /// cell (see `crate::StackPadding::loop_variable`).
    pub(super) loop_variable_joins: BTreeSet<VisitKey>,
    /// The forks whose sides a join lined up in the shape of a deficit.
    pub(super) deficit_joins: BTreeSet<VisitKey>,
    /// The enclosing forks' meeting targets, outermost first, while one of
    /// their sides steps.
    pub(super) targets: Vec<Target>,
    /// How many forks the running path is inside whose other side still
    /// runs too.
    pub(super) fork_depth: usize,
    /// Forks reached [`MAX_FORK_DEPTH`](super::MAX_FORK_DEPTH): a side that
    /// passed the meeting point goes on into the next fork inside this one,
    /// and so on down a run of `x = c && f()`, before the side with the
    /// longer arm can step. From then on the walk holds a side at a fork of
    /// its own until the other catches up.
    pub(super) patient: bool,
    /// The next step may hold its side at a fork (see [`Halt::Held`]).
    pub(super) may_hold: bool,
    /// The fork whose join left each bytecode byte its sides patched
    /// differently (see [`Learned::apart_bytes`]).
    pub(super) image_splits: BTreeMap<usize, VisitKey>,
    /// What each side of that fork left in such a byte: (taken, fall-through).
    pub(super) split_bytes: BTreeMap<usize, (Option<u8>, Option<u8>)>,
    /// Each operand value an instruction read from bytes that include one
    /// of those, with the byte.
    pub(super) patched_operands: BTreeMap<i64, usize>,
    /// Where each `try` the walk entered goes when it catches.
    pub(super) try_handlers: Vec<DispatchStart>,
    /// A function made at an entry outside the image, read from a byte a
    /// fork's sides patched differently: that fork, whether its taken side is
    /// the one that patched it, and why (see `Walk::patched_junk_entry`).
    pub(super) patched_junk: Option<(VisitKey, bool, String)>,
    /// The forks whose sides joined lined up from the top (see
    /// `TraceSession::pad_stack`), most recent last, each with what the walk
    /// had learned when they joined (undoing the join goes back to it) and
    /// whether the taken side was the shorter.
    pub(super) padded: Vec<(VisitKey, Arc<Learned>, bool)>,
    /// The byte the last step read that the merged histories patched
    /// differently, if that is why it stopped.
    pub(super) split_read_at: Option<usize>,
}
