//! `network` thread rules.
use crate::naming::*;

pub(in crate::naming) mod ice_emitter;

pub(in crate::naming) mod url_query_rebuild;

pub(in crate::naming) mod captured_key_callback_removal;

pub(in crate::naming) mod request_open_metadata;
pub(in crate::naming) mod ice_candidate;

pub(in crate::naming) mod script_digest_dispatch;
pub(in crate::naming) mod route_pattern_canonicalizer;
pub(in crate::naming) mod xhr_open_metadata;
