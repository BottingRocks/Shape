//! Shared per-step dispatch mechanics.
//!
//! Both dispatch drivers walk the same proven image/table and decode the same
//! operand recipes. This module owns that mechanics. It deliberately reports
//! faults as an internal [`StepFault`]: each driver maps the fault to its own
//! stop type, diagnostic code prefix and message, so the drivers keep their
//! distinct policy and observable output.

use std::collections::{BTreeMap, BTreeSet};

use crate::tracing::symbolic::CallFrame;

/// A visited dispatch position: PC, dispatch state and the VM call stack.
pub(super) type VisitKey = (usize, usize, Vec<CallFrame>);

use crate::{
    bounded_decode_operands, DecodeOperandsError, DecodedInstruction, DecodedOperand, DispatchCell,
    HandlerTemplate, HandlerTemplateReport, JsOperandSeparationReport, LiftedHandler,
    OperandEntryCoordinate, OperandSpan, SymbolicValue,
};

/// The recipe identity carried by every terminal completion record.
pub(crate) const COMPLETION_RECIPE_IDENTITY: &str = "dispatch-completion";

/// Dense selector indexes may allocate at most this many slots. Selectors are
/// arbitrary `u32` table keys, so a sparse set such as `{u32::MAX}` must fall
/// back to a map instead of requesting a multi-gigabyte vector.
const DENSE_INDEX_MAX_SLOTS: usize = 1 << 16;

/// Dense indexes may be at most this many times larger than the handler count.
/// Selector sets that are much sparser than this use a `BTreeMap`.
const DENSE_INDEX_GROWTH: usize = 4;

/// Selector-to-handler-position lookup built once during preparation.
///
/// Positions are indexes into the prepared handler list. The index is immutable
/// after construction and preserves the old linear-scan behavior for duplicate
/// selectors: the first handler with a selector wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SelectorIndex {
    Dense(Vec<Option<usize>>),
    Sparse(BTreeMap<u32, usize>),
}

impl SelectorIndex {
    /// Build an index over handler selectors in list order. A duplicate selector
    /// keeps its first position, matching `iter().find(..)`.
    pub(super) fn from_handler_indices<I>(handler_indices: I) -> Self
    where
        I: IntoIterator<Item = u32>,
    {
        let selectors = handler_indices.into_iter().collect::<Vec<_>>();
        let Some(max) = selectors.iter().copied().max() else {
            return Self::Dense(Vec::new());
        };
        let dense_slots = selectors
            .len()
            .saturating_mul(DENSE_INDEX_GROWTH)
            .min(DENSE_INDEX_MAX_SLOTS);
        if u64::from(max) < dense_slots as u64 {
            let mut slots = vec![None; max as usize + 1];
            for (position, selector) in selectors.into_iter().enumerate() {
                let slot = &mut slots[selector as usize];
                if slot.is_none() {
                    *slot = Some(position);
                }
            }
            Self::Dense(slots)
        } else {
            let mut positions = BTreeMap::new();
            for (position, selector) in selectors.into_iter().enumerate() {
                positions.entry(selector).or_insert(position);
            }
            Self::Sparse(positions)
        }
    }

    /// The first handler position for `selector`, if the report contains one.
    pub(super) fn get(&self, selector: u32) -> Option<usize> {
        match self {
            Self::Dense(slots) => slots.get(selector as usize).copied().flatten(),
            Self::Sparse(positions) => positions.get(&selector).copied(),
        }
    }
}

/// One fetched dispatch cell and the opcode that selected it.
pub(super) struct Fetched<'a> {
    pub opcode: u8,
    pub cell: &'a DispatchCell,
    pub next_state: usize,
}

/// One decoded handler step.
pub(super) struct Decoded {
    /// The coordinate the operands were read from (`AtOpcode` keeps the
    /// dispatch PC; `AfterOpcode` advances past the opcode).
    pub operand_entry_pc: usize,
    pub operands: Vec<DecodedOperand>,
    pub actual_next_pc: usize,
    pub instruction: DecodedInstruction,
}

/// A step-level failure that both drivers classify according to their own
/// policy. Variants carry only the data both drivers need to build their own
/// stop reason, diagnostic code and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StepFault {
    Repeated {
        pc: usize,
        state: usize,
    },
    TruncatedImage {
        pc: usize,
        image_len: usize,
    },
    MissingState {
        state: usize,
    },
    MissingOpcode {
        state: usize,
        opcode: u8,
    },
    InvalidNextState {
        next_state: usize,
    },
    PcOverflow {
        pc: usize,
        image_len: usize,
    },
    TruncatedOperands {
        /// The operand position whose bytes are incomplete.
        position: u32,
        operand_entry_pc: usize,
        required_end: usize,
        image_len: usize,
    },
    UnsupportedOperands {
        selector: u32,
        reason: String,
    },
    NextPcOverflow {
        pc: usize,
        image_len: usize,
    },
    MissingOperandHandler {
        selector: u32,
    },
}

/// Fetch the opcode byte, the table cell it selects and that cell's next state
/// for `(pc, state)`, without the driver's revisit or next-state-bounds policy.
///
/// The bounded drivers' [`fetch_step`] adds the revisit check before this
/// lookup and the next-state bounds check after it.
pub(super) fn fetch_cell<'a>(
    image: &[u8],
    table: &'a [Vec<DispatchCell>],
    pc: usize,
    state: usize,
) -> Result<Fetched<'a>, StepFault> {
    let opcode = image.get(pc).copied().ok_or(StepFault::TruncatedImage {
        pc,
        image_len: image.len(),
    })?;
    let row = table.get(state).ok_or(StepFault::MissingState { state })?;
    let cell = row
        .get(usize::from(opcode))
        .ok_or(StepFault::MissingOpcode { state, opcode })?;
    Ok(Fetched {
        opcode,
        cell,
        next_state: cell.next_state as usize,
    })
}

/// Mark `(pc, state)` as visited and fetch its opcode, table cell and next
/// state. The revisit check comes first so a repeated coordinate is reported
/// before any image or table lookup; the next-state bounds check follows the
/// cell lookup.
pub(super) fn fetch_step<'a>(
    image: &[u8],
    table: &'a [Vec<DispatchCell>],
    pc: usize,
    state: usize,
    frames: &[CallFrame],
    visited: &mut BTreeSet<VisitKey>,
) -> Result<Fetched<'a>, StepFault> {
    // The VM call stack is part of the position: calling the same subroutine
    // from two sites legitimately revisits its (pc, state).
    if !visited.insert((pc, state, frames.to_vec())) {
        return Err(StepFault::Repeated { pc, state });
    }
    let fetched = fetch_cell(image, table, pc, state)?;
    if fetched.next_state >= table.len() {
        return Err(StepFault::InvalidNextState {
            next_state: fetched.next_state,
        });
    }
    Ok(fetched)
}

/// The PC immediately after the opcode byte.
pub(super) fn checked_pc_increment(pc: usize, image_len: usize) -> Result<usize, StepFault> {
    pc.checked_add(1)
        .ok_or(StepFault::PcOverflow { pc, image_len })
}

/// The terminal completion-selector step coordinates: the operand entry and the
/// successor PC both advance past the opcode byte, and the record carries no
/// operands.
pub(super) struct CompletionCoordinates {
    pub operand_entry_pc: usize,
    pub actual_next_pc: usize,
    /// The proven completion source span the record must carry.
    pub source_span: OperandSpan,
}

/// Derive the completion-selector coordinates for `pc`. The opcode byte is the
/// only byte a completion consumes, so the operand entry and successor PCs
/// coincide at `pc + 1`.
pub(super) fn completion_coordinates(
    pc: usize,
    image_len: usize,
    source_span: OperandSpan,
) -> Result<CompletionCoordinates, StepFault> {
    let next_pc = checked_pc_increment(pc, image_len)?;
    Ok(CompletionCoordinates {
        operand_entry_pc: next_pc,
        actual_next_pc: next_pc,
        source_span,
    })
}

/// Find the operand-lifted handler for `selector` through the prepared index.
pub(super) fn find_operand_handler<'a>(
    operands: &'a JsOperandSeparationReport,
    index: &SelectorIndex,
    selector: u32,
) -> Result<&'a LiftedHandler, StepFault> {
    index
        .get(selector)
        .and_then(|position| operands.handlers.get(position))
        .ok_or(StepFault::MissingOperandHandler { selector })
}

/// Find the operation-table template for `selector` through the prepared index.
pub(super) fn find_template<'a>(
    templates: &'a HandlerTemplateReport,
    index: &SelectorIndex,
    selector: u32,
) -> Option<&'a HandlerTemplate> {
    index
        .get(selector)
        .and_then(|position| templates.handlers.get(position))
}

/// Select the operand entry coordinate, decode the handler operands, compute
/// the successor PC and build the instruction.
pub(super) fn decode_step(
    image: &[u8],
    pc: usize,
    lifted: &LiftedHandler,
) -> Result<Decoded, StepFault> {
    let operand_entry_pc = match lifted.entry_coordinate {
        OperandEntryCoordinate::AfterOpcode => checked_pc_increment(pc, image.len())?,
        OperandEntryCoordinate::AtOpcode => pc,
    };
    let operands = match bounded_decode_operands(image, operand_entry_pc, lifted) {
        Ok(operands) => operands,
        Err(DecodeOperandsError::Truncated {
            position,
            required_end,
            image_len,
        }) => {
            return Err(StepFault::TruncatedOperands {
                position,
                operand_entry_pc,
                required_end,
                image_len,
            });
        }
        Err(DecodeOperandsError::Unsupported(reason)) => {
            return Err(StepFault::UnsupportedOperands {
                selector: lifted.handler_index,
                reason,
            });
        }
    };
    let actual_next_pc = operand_entry_pc
        .checked_add(lifted.pc_delta as usize)
        .ok_or(StepFault::NextPcOverflow {
            pc,
            image_len: image.len(),
        })?;
    let instruction = DecodedInstruction::new(
        lifted.handler_index,
        operands
            .iter()
            .map(|operand: &DecodedOperand| SymbolicValue::Number(operand.value as f64))
            .collect(),
    );
    Ok(Decoded {
        operand_entry_pc,
        operands,
        actual_next_pc,
        instruction,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_index_prefers_the_first_duplicate_selector() {
        let index = SelectorIndex::from_handler_indices([1, 1, 2]);
        assert!(matches!(index, SelectorIndex::Dense(_)));
        assert_eq!(index.get(1), Some(0));
        assert_eq!(index.get(2), Some(2));
        assert_eq!(index.get(3), None);
    }

    #[test]
    fn sparse_index_prefers_the_first_duplicate_selector() {
        let index = SelectorIndex::from_handler_indices([u32::MAX, u32::MAX, 7]);
        assert!(matches!(index, SelectorIndex::Sparse(_)));
        assert_eq!(index.get(u32::MAX), Some(0));
        assert_eq!(index.get(7), Some(2));
    }

    #[test]
    fn sparse_selectors_do_not_allocate_a_giant_dense_table() {
        let index = SelectorIndex::from_handler_indices([1_000_000]);
        assert!(matches!(index, SelectorIndex::Sparse(_)));
        assert_eq!(index.get(1_000_000), Some(0));

        let empty = SelectorIndex::from_handler_indices(std::iter::empty());
        assert_eq!(empty.get(0), None);
    }

    fn frame(return_byte_offset: usize) -> CallFrame {
        CallFrame {
            return_byte_offset,
            return_dispatch_state: 0,
        }
    }

    /// A subroutine reached from two call sites revisits its (pc, state) under
    /// a different call stack; that is progress, not a loop.
    #[test]
    fn the_call_stack_distinguishes_repeated_positions() {
        let image = [0u8, 0];
        let table = vec![vec![DispatchCell {
            next_state: 0,
            selector: 1,
        }]];
        let mut visited = BTreeSet::new();
        assert!(fetch_step(&image, &table, 1, 0, &[], &mut visited).is_ok());
        assert!(fetch_step(&image, &table, 1, 0, &[frame(10)], &mut visited).is_ok());
        assert!(fetch_step(&image, &table, 1, 0, &[frame(20)], &mut visited).is_ok());
        assert!(fetch_step(&image, &table, 1, 0, &[frame(10), frame(20)], &mut visited).is_ok());

        // The same position under the same stack is a real loop.
        assert_eq!(
            fetch_step(&image, &table, 1, 0, &[frame(10)], &mut visited).err(),
            Some(StepFault::Repeated { pc: 1, state: 0 })
        );
        assert_eq!(
            fetch_step(&image, &table, 1, 0, &[], &mut visited).err(),
            Some(StepFault::Repeated { pc: 1, state: 0 })
        );
    }

    /// A repeated position is reported before the cell is fetched, so even a
    /// position outside the image is a loop on its second visit.
    #[test]
    fn repetition_is_checked_before_the_fetch() {
        let table = vec![vec![DispatchCell {
            next_state: 0,
            selector: 1,
        }]];
        let mut visited = BTreeSet::new();
        assert_eq!(
            fetch_step(&[], &table, 5, 0, &[], &mut visited).err(),
            Some(StepFault::TruncatedImage {
                pc: 5,
                image_len: 0
            })
        );
        assert_eq!(
            fetch_step(&[], &table, 5, 0, &[], &mut visited).err(),
            Some(StepFault::Repeated { pc: 5, state: 0 })
        );
    }
}
