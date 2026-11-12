//! `scheduling` thread rules.
use crate::naming::shared::*;
use crate::naming::*;

pub(in crate::naming) mod bounded_timeout;
pub(in crate::naming) mod callback_variants;
pub(in crate::naming) mod elapsed_hours;
pub(in crate::naming) mod elapsed_timer;
pub(in crate::naming) mod frame_sampler;
pub(in crate::naming) mod sine_prng_refill;
pub(in crate::naming) mod task_queue;

pub(in crate::naming) mod interval_extrema;
pub(in crate::naming) mod work_batch_protocols;
