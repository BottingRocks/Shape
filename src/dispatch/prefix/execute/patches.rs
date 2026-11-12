use super::*;

pub(super) fn wasm_bound(
    machine: &crate::dispatch::prefix::wasm::WasmMachine,
    session: &TraceSession,
) -> Option<(String, bool)> {
    let (tries, records) = session.exception_depths();
    let calls = session.call_frames().len();
    if tries > machine.try_frames {
        return Some((
            format!(
                "{tries} try frames: the WASM interpreter traps past {}",
                machine.try_frames
            ),
            true,
        ));
    }
    if records > machine.error_records {
        return Some((
            format!(
                "{records} error records: the WASM interpreter traps past {}",
                machine.error_records
            ),
            true,
        ));
    }
    if calls > machine.call_frames {
        return Some((
            "a VM call inside a VM call: the WASM interpreter keeps one return address".into(),
            false,
        ));
    }
    None
}

/// Record the diagnostic for a shared step fault and return the prefix
/// driver's stop reason.
pub(super) fn halt(session: &mut TraceSession, fault: &StepFault) -> PrefixStopReason {
    let (stop, code, message) = prefix_fault(fault);
    session.stop_prefix(code, message, None);
    stop
}

/// Map a shared step fault to the prefix driver's observable stop reason,
/// diagnostic code and message.
pub(super) fn prefix_fault(fault: &StepFault) -> (PrefixStopReason, TraceStopCode, String) {
    match fault {
        StepFault::Repeated { pc, state } => (
            PrefixStopReason::RepeatedPcState {
                pc: *pc,
                state: *state,
            },
            TraceStopCode::PrefixRepeatedPcState,
            format!("prefix revisited pc {pc} state {state} without progress"),
        ),
        StepFault::TruncatedImage { pc, image_len } => (
            PrefixStopReason::TruncatedImage {
                pc: *pc,
                image_len: *image_len,
            },
            TraceStopCode::PrefixTruncatedImage,
            format!("dispatch pc {pc} is outside the {image_len} byte image"),
        ),
        StepFault::MissingState { state } => (
            PrefixStopReason::MissingState { state: *state },
            TraceStopCode::PrefixMissingState,
            format!("dispatch state {state} has no proven table row"),
        ),
        StepFault::MissingOpcode { state, opcode } => (
            PrefixStopReason::MissingOpcode {
                state: *state,
                opcode: *opcode,
            },
            TraceStopCode::PrefixMissingOpcode,
            format!("dispatch state {state} opcode {opcode} has no proven cell"),
        ),
        StepFault::InvalidNextState { next_state } => (
            PrefixStopReason::MissingState { state: *next_state },
            TraceStopCode::PrefixInvalidNextState,
            format!("dispatch cell targets state {next_state} outside the proven table"),
        ),
        StepFault::PcOverflow { pc, image_len } => (
            PrefixStopReason::TruncatedImage {
                pc: *pc,
                image_len: *image_len,
            },
            TraceStopCode::PrefixPcOverflow,
            "dispatch PC increment overflowed the address space".into(),
        ),
        StepFault::TruncatedOperands {
            operand_entry_pc,
            required_end,
            image_len,
            ..
        } => (
            PrefixStopReason::TruncatedOperand {
                operand_entry_pc: *operand_entry_pc,
                needed: required_end.saturating_sub(*operand_entry_pc),
                available: image_len.saturating_sub(*operand_entry_pc),
            },
            TraceStopCode::PrefixTruncatedOperands,
            "operand image is truncated before the handler operands are complete".into(),
        ),
        StepFault::UnsupportedOperands { selector, reason } => (
            PrefixStopReason::UnsupportedHandler {
                selector: *selector,
                reason: reason.clone(),
            },
            TraceStopCode::PrefixUnsupportedOperands,
            reason.clone(),
        ),
        StepFault::NextPcOverflow { pc, image_len } => (
            PrefixStopReason::TruncatedImage {
                pc: *pc,
                image_len: *image_len,
            },
            TraceStopCode::PrefixNextPcOverflow,
            "handler PC delta overflowed the address space".into(),
        ),
        StepFault::MissingOperandHandler { selector } => {
            let reason = "selector has no operand-separation handler".to_string();
            (
                PrefixStopReason::UnsupportedHandler {
                    selector: *selector,
                    reason: reason.clone(),
                },
                TraceStopCode::PrefixMissingOperands,
                reason,
            )
        }
    }
}

/// Record the bytes two histories may not agree on where their paths
/// merge: `merged` keeps its image as the representative, and its divergent
/// bytes become both sides' and every byte the two images differ in.
pub(super) fn merge_images(merged: &mut Cursor, other: &Cursor) -> Result<(), String> {
    merge_images_at(merged, other, None)
}

/// [`merge_images`] for images that can differ only at `patched`: the
/// addresses either history patched since they were the same image.
pub(super) fn merge_images_at(
    merged: &mut Cursor,
    other: &Cursor,
    patched: Option<&BTreeSet<usize>>,
) -> Result<(), String> {
    if merged.image.len() != other.image.len() {
        return Err("the merged histories hold bytecode images of different lengths".into());
    }
    merged.split_image.extend(other.split_image.iter().copied());
    if Arc::ptr_eq(&merged.image, &other.image) {
        return Ok(());
    }
    match patched {
        Some(patched) => {
            for &address in patched {
                if merged.image.get(address) != other.image.get(address) {
                    merged.split_image.insert(address);
                }
            }
        }
        None => {
            for (address, (left, right)) in merged.image.iter().zip(other.image.iter()).enumerate()
            {
                if left != right {
                    merged.split_image.insert(address);
                }
            }
        }
    }
    Ok(())
}

/// The first byte in `[start, start + width)` the merged histories may not
/// agree on.
pub(super) fn split_in_range(split: &BTreeSet<usize>, start: usize, width: usize) -> Option<usize> {
    let end = start.checked_add(width)?;
    split.range(start..end).next().copied()
}

/// Stop: a merged path reads a byte its histories patched differently, so
/// its value is only known per history.
pub(super) fn split_read_stop(
    session: &mut TraceSession,
    pc: usize,
    state: usize,
    address: usize,
) -> PrefixStopReason {
    let reason = format!(
        "the branches patched bytecode byte {address} differently, and the merged path reads it"
    );
    session.stop_prefix(TraceStopCode::PrefixBranchJoin, reason.clone(), None);
    PrefixStopReason::UnjoinableBranches { pc, state, reason }
}

#[cfg(test)]
pub(super) fn apply_image_patch(
    image: &mut [u8],
    operand_entry_pc: usize,
    width: usize,
    selector: u32,
) -> Result<(ImagePatchRecord, usize), PrefixStopReason> {
    let (patch, actual_next_pc) = read_image_patch(image, operand_entry_pc, width, selector)?;
    image[patch.address as usize] = patch.value;
    Ok((patch, actual_next_pc))
}

/// The patch the operand at `operand_entry_pc` makes, with the byte it
/// replaces, and where the next instruction starts; the image unchanged.
pub(super) fn read_image_patch(
    image: &[u8],
    operand_entry_pc: usize,
    width: usize,
    selector: u32,
) -> Result<(ImagePatchRecord, usize), PrefixStopReason> {
    // The proof emits `PATCH_OPERAND_WIDTH`. Treat any other width as an
    // explicit unsupported boundary instead of indexing past a short operand or
    // silently ignoring extra bytes.
    if width != PATCH_OPERAND_WIDTH {
        return Err(PrefixStopReason::UnsupportedHandler {
            selector,
            reason: format!(
                "image-patch handler operands are {width} bytes; the proven layout is {PATCH_OPERAND_WIDTH} bytes"
            ),
        });
    }
    let available = image.len().saturating_sub(operand_entry_pc);
    if available < width {
        return Err(PrefixStopReason::TruncatedOperand {
            operand_entry_pc,
            needed: width,
            available,
        });
    }
    let Some(operand_end) = operand_entry_pc.checked_add(width) else {
        return Err(PrefixStopReason::TruncatedOperand {
            operand_entry_pc,
            needed: width,
            available,
        });
    };
    let bytes = &image[operand_entry_pc..operand_end];
    // Proven layout: little-endian 24-bit address in bytes 0..3, then the value.
    let address = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]);
    let value = bytes[PATCH_ADDRESS_BYTES];
    if u64::from(address) >= image.len() as u64 {
        return Err(PrefixStopReason::AddressOutOfBounds {
            address: address.into(),
            image_len: image.len(),
        });
    }
    let address = address as usize;
    let previous = image[address];
    let actual_next_pc = operand_end;
    Ok((
        ImagePatchRecord {
            address: address as u32,
            previous,
            value,
        },
        actual_next_pc,
    ))
}
