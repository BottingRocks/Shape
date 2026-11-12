//! `events` thread rules.
use crate::naming::shared::*;
use crate::naming::*;

pub(in crate::naming) mod console_error_report;
pub(in crate::naming) mod console_error_statement;
pub(in crate::naming) mod console_error_unresolved_global;
pub(in crate::naming) mod cookie_emission;
pub(in crate::naming) mod custom_event_dispatch;
pub(in crate::naming) mod error_listener;
pub(in crate::naming) mod event_metadata;
pub(in crate::naming) mod exchange_handshake;
pub(in crate::naming) mod exchange_teardown;
pub(in crate::naming) mod message_envelope;
pub(in crate::naming) mod matching_key_callback;
pub(in crate::naming) mod mutation_style;
pub(in crate::naming) mod pointer_travel_accumulation;
pub(in crate::naming) mod pointer_travel_distance;
pub(in crate::naming) mod pointer_travel_try_catch;
pub(in crate::naming) mod ready_state_handler;
pub(in crate::naming) mod remember_checkbox;
pub(in crate::naming) mod selector_event_count;
pub(in crate::naming) mod selector_exclusion_count;
pub(in crate::naming) mod storage_removal_emitter;
pub(in crate::naming) mod storage_removal_emitter_variants;
pub(in crate::naming) mod subscription_adapter;
pub(in crate::naming) mod touch_payload_record;
pub(in crate::naming) mod touch_sample;
pub(in crate::naming) mod visibility_change_publish;
pub(in crate::naming) mod visibility_change_sample;
pub(in crate::naming) mod visibility_change_state;
pub(in crate::naming) mod visibility_method_alias;
pub(in crate::naming) mod visibility_persistence_variants;

pub(in crate::naming) mod submit_notification;

pub(in crate::naming) mod input_paste_marker;

pub(in crate::naming) mod orientation_moments;

pub(in crate::naming) mod wheel_counter_frame;

pub(in crate::naming) mod interaction_timing_recorders;
