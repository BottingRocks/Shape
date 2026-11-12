//! `browser` thread rules.
use crate::naming::shared::*;
use crate::naming::*;

pub(in crate::naming) mod cookie_plugin_adapter;
pub(in crate::naming) mod frame_carrier;
pub(in crate::naming) mod frame_start;
pub(in crate::naming) mod injected_global_removal;
pub(in crate::naming) mod navigation_submit;
pub(in crate::naming) mod persistent_token_init;
pub(in crate::naming) mod persistent_token_init_variants;
pub(in crate::naming) mod persistent_id_migration;
pub(in crate::naming) mod proxy_probe;
pub(in crate::naming) mod remove_injected_token_inputs;
pub(in crate::naming) mod submit_instrumentation_teardown;
pub(in crate::naming) mod url_query_merge;
pub(in crate::naming) mod url_query_reader;

pub(in crate::naming) mod uri_component_decode;

pub(in crate::naming) mod persistent_event_state;
