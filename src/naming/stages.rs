//! Frozen stage order for the thread rule pipeline: every older stage
//! must decline before the next group is consulted.
use super::shared::*;
use super::*;

pub(super) fn evaluate_additions(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    // PASS6 precedence is frozen: run every original module first and return
    // their claims unchanged when any exist.
    let mut pass6 = Vec::new();
    pass6.extend(events::subscription_adapter::evaluate(
        model, site, f, proposals,
    ));
    pass6.extend(runtime::owned_runtime::evaluate(model, site, f, proposals));
    pass6.extend(encoding::aes_gcm_packing::evaluate(
        model, site, f, proposals,
    ));
    pass6.extend(observables::observable_protocol::evaluate(
        model, site, f, proposals,
    ));
    pass6.extend(events::cookie_emission::evaluate(model, site, f, proposals));
    pass6.extend(events::message_envelope::evaluate(
        model, site, f, proposals,
    ));
    pass6.extend(encoding::state_dataflow::evaluate(
        model, site, f, proposals,
    ));
    pass6.extend(runtime::traversal::evaluate(model, site, f, proposals));
    pass6.extend(runtime::immutable_wrappers::evaluate(
        model, site, f, proposals,
    ));
    // Only the exact same-function generic-state versus immutable-sentinel pair
    // is collapsed; all other simultaneous claims stay in the normal resolver.
    let pass6 = resolve_sentinel_specificity(pass6);
    if !pass6.is_empty() {
        return pass6;
    }
    // PASS7 has priority over the rolling frontier. Only when all ten PASS7
    // anchors decline are the six newly accepted claims collected together
    // for the normal ambiguity resolver.
    let mut pass7 = Vec::new();
    pass7.extend(browser::uri_component_decode::evaluate(model, site, f, proposals));
    pass7.extend(runtime::media_feature_bisection::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::touch_payload_record::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(encoding::seeded_hash_random_bytes::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(encoding::seeded_hash_digest::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(observables::signal_report_builder::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(observables::seeded_signal_frame::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::touch_sample::evaluate(model, site, f, proposals));
    pass7.extend(graphics::animation_frame_sample::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::selector_event_count::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::pointer_travel_distance::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(scheduling::task_queue::evaluate(model, site, f, proposals));
    pass7.extend(events::console_error_statement::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(scheduling::sine_prng_refill::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(graphics::bounded_pixel_vector_history::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(browser::submit_instrumentation_teardown::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(encoding::seeded_hash_encoding::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::visibility_change_sample::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(graphics::distinct_vector_sample::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::visibility_change_state::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::console_error_report::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(runtime::captured_field_sync::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::visibility_change_publish::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(observables::hash_string_or_buffer::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(graphics::frame_sample_request::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::pointer_travel_try_catch::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(encoding::entropy_tagged_numeric_array::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(encoding::chacha_like_stream_cipher::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(runtime::class_member_installer::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(scheduling::elapsed_timer::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(runtime::extension_registry::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::custom_event_dispatch::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::ready_state_handler::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(runtime::serializer_error::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::console_error_unresolved_global::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::selector_exclusion_count::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(events::pointer_travel_accumulation::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(graphics::scaled_animation_frame_sample::evaluate(
        model, site, f, proposals,
    ));
    pass7.extend(graphics::unique_vector_history::evaluate(
        model, site, f, proposals,
    ));
    if !pass7.is_empty() {
        return pass7;
    }

    let mut frontier = Vec::new();
    frontier.extend(runtime::native_function::evaluate(
        model, site, f, proposals,
    ));
    frontier.extend(graphics::frontier_unique_sample::evaluate(
        model, site, f, proposals,
    ));
    frontier.extend(observables::error_enqueue::evaluate(model, site, f, proposals));
    frontier.extend(observables::relay_subscription::evaluate(
        model, site, f, proposals,
    ));
    frontier.extend(runtime::map_keys_to_object::evaluate(
        model, site, f, proposals,
    ));
    frontier.extend(observables::relay_subscription_consumer::evaluate(
        model, site, f, proposals,
    ));
    if !frontier.is_empty() {
        return frontier;
    }

    // A003: collect the three reviewed frontier claims together after every
    // older frontier has declined, preserving normal ambiguity resolution.
    let mut stage_a = Vec::new();
    stage_a.extend(graphics::distinct_vector_copy::evaluate(
        model, site, f, proposals,
    ));
    stage_a.extend(observables::chunked_entries_reassembly::evaluate(
        model, site, f, proposals,
    ));
    stage_a.extend(scheduling::bounded_timeout::evaluate(
        model, site, f, proposals,
    ));
    stage_a.extend(runtime::worker_message_dispatch::evaluate(
        model, site, f, proposals,
    ));
    if !stage_a.is_empty() {
        return stage_a;
    }

    // B001: evaluate this trio only after A has declined.
    let mut stage_b = Vec::new();
    stage_b.extend(events::visibility_method_alias::evaluate(
        model, site, f, proposals,
    ));
    stage_b.extend(browser::remove_injected_token_inputs::evaluate(
        model, site, f, proposals,
    ));
    stage_b.extend(observables::snapshot::evaluate(model, site, f, proposals));
    if !stage_b.is_empty() {
        return stage_b;
    }

    // C001: evaluate this trio only after B has declined.
    let mut stage_c = Vec::new();
    stage_c.extend(encoding::code_units::evaluate(model, site, f, proposals));
    stage_c.extend(scheduling::elapsed_hours::evaluate(
        model, site, f, proposals,
    ));
    stage_c.extend(browser::injected_global_removal::evaluate(
        model, site, f, proposals,
    ));
    if !stage_c.is_empty() {
        return stage_c;
    }

    // POST110: collect both ready families only after every older stage has
    // declined, preserving the normal ambiguity resolver for their claims.
    let mut stage_d = Vec::new();
    stage_d.extend(browser::frame_start::evaluate(model, site, f, proposals));
    stage_d.extend(runtime::worker_init::evaluate(model, site, f, proposals));
    if !stage_d.is_empty() {
        return stage_d;
    }

    // POST110-FIVE: these three reviewed families run only after all older
    // stages and the pair above decline, preserving normal ambiguity handling.
    let mut stage_e = Vec::new();
    stage_e.extend(runtime::collector_start::evaluate(
        model, site, f, proposals,
    ));
    stage_e.extend(encoding::hash_variants::evaluate(model, site, f, proposals));
    stage_e.extend(observables::observable_factory::evaluate(
        model, site, f, proposals,
    ));
    if !stage_e.is_empty() {
        return stage_e;
    }

    // RESUME-001: collect the three independently reviewed post-host families
    // only after every earlier stage, including POST110-FIVE, has declined.
    // Keep all claims together so the normal ambiguity resolver can reject
    // conflicting names without allowing packet order to decide the result.
    let mut stage_f = Vec::new();
    stage_f.extend(runtime::function_hooks::evaluate(model, site, f, proposals));
    stage_f.extend(observables::serializer::evaluate(model, site, f, proposals));
    stage_f.extend(observables::payload_finalizer::evaluate(
        model, site, f, proposals,
    ));
    if !stage_f.is_empty() {
        return stage_f;
    }

    // RESUME-001 next-three families are collected together after all first
    // three post-host claims decline, preserving normal ambiguity handling.
    let mut stage_g = Vec::new();
    stage_g.extend(encoding::numeric_writer::evaluate(
        model, site, f, proposals,
    ));
    stage_g.extend(graphics::canvas_image_probe::evaluate(
        model, site, f, proposals,
    ));
    if !stage_g.is_empty() {
        return stage_g;
    }

    // ACCESSOR-004: evaluate only after every earlier rule, including the
    // accepted numeric and canvas families, has declined. Keep its claims
    // separate so existing stage precedence and ambiguity behavior remain
    // unchanged.
    let mut stage_h = Vec::new();
    stage_h.extend(runtime::accessor_hooks::evaluate(model, site, f, proposals));
    if !stage_h.is_empty() {
        return stage_h;
    }

    // POST2792-FRAME-CARRIER: evaluate the hidden nested-frame bridge carrier
    // only after the accepted accessor stage declines.
    let mut stage_i = Vec::new();
    stage_i.extend(browser::frame_carrier::evaluate(model, site, f, proposals));
    if !stage_i.is_empty() {
        return stage_i;
    }

    // POST2792-A: evaluate the accepted persistent initializer only after
    // every earlier rule, including ACCESSOR-004, declines. Keep its claims
    // in a fallback stage so normal ambiguity resolution remains unchanged.
    let mut stage_i = Vec::new();
    stage_i.extend(browser::persistent_token_init::evaluate(
        model, site, f, proposals,
    ));
    if !stage_i.is_empty() {
        return stage_i;
    }

    // Owned persistent-ID migration grammar; fallback after accepted initializers.
    let migration = browser::persistent_id_migration::evaluate(model, site, f, proposals);
    if !migration.is_empty() {
        return migration;
    }

    let variant = browser::persistent_token_init_variants::evaluate(model, site, f, proposals);
    if !variant.is_empty() { return variant; }

    // EXCHANGE-001: evaluate the accepted custom-event exchange handshake
    // only after every earlier fallback stage has declined.
    let mut stage_j = Vec::new();
    stage_j.extend(events::exchange_handshake::evaluate(
        model, site, f, proposals,
    ));
    stage_j.extend(events::submit_notification::evaluate(model, site, f, proposals));
    stage_j.extend(browser::navigation_submit::evaluate(
        model, site, f, proposals,
    ));
    if !stage_j.is_empty() {
        return stage_j;
    }

    // EVENT-METADATA: last fallback after every accepted stage declines.
    let mut stage_k = Vec::new();
    stage_k.extend(events::event_metadata::evaluate(model, site, f, proposals));
    if !stage_k.is_empty() {
        return stage_k;
    }

    // MUTATION-STYLE-012: final fallback for the bounded owned callback proof.
    let mut stage_l = Vec::new();
    stage_l.extend(events::mutation_style::evaluate(model, site, f, proposals));
    if !stage_l.is_empty() {
        return stage_l;
    }

    // EXCHANGE-TEARDOWN-001: last fallback for the custom-event teardown proof.
    let mut stage_m = Vec::new();
    stage_m.extend(events::exchange_teardown::evaluate(
        model, site, f, proposals,
    ));
    if !stage_m.is_empty() {
        return stage_m;
    }

    // ENTROPY-002: final fallback for the bounded entropy-seed helper proof.
    let mut stage_n = Vec::new();
    stage_n.extend(encoding::entropy_seed::evaluate(model, site, f, proposals));
    if !stage_n.is_empty() {
        return stage_n;
    }

    // ICE-CANDIDATE-001: the WebRTC-candidate field variant of the reviewed
    // seeded emitter is the last fallback. It runs only when every earlier
    // source-backed evaluator, including the entropy-seed helper above, declined.
    let mut stage_ice = Vec::new();
    stage_ice.extend(network::ice_emitter::evaluate(model, site, f, proposals));
    if !stage_ice.is_empty() {
        return stage_ice;
    }

    // COOKIE-PLUGIN-ADAPTER: after the actual final pre-existing ICE fallback,
    // claim the custom cookie collector's stage-registrar plugin and its
    // captured ctor registration adapter. Additive only; every earlier stage
    // remains unchanged.
    let mut stage_o = Vec::new();
    stage_o.extend(browser::cookie_plugin_adapter::evaluate(
        model, site, f, proposals,
    ));
    if !stage_o.is_empty() {
        return stage_o;
    }

    // URL-QUERY-MERGE-002: evaluate the reviewed three-formal URL query
    // merger only after every existing fallback stage has declined.
    let mut stage_url = Vec::new();
    stage_url.extend(browser::url_query_merge::evaluate(
        model, site, f, proposals,
    ));
    if !stage_url.is_empty() {
        return stage_url;
    }

    // WORKER-ENVIRONMENT: run the accepted worker-posted seeded environment
    // bundle only after every existing shared fallback has declined. The
    // evaluator itself reuses accepted helper claims and does not preempt any
    // earlier proposal.
    let mut stage_worker = Vec::new();
    stage_worker.extend(runtime::worker_environment::evaluate(
        model, site, f, proposals,
    ));
    if !stage_worker.is_empty() {
        return stage_worker;
    }

    // CANVAS-TEXT-PIXEL-001: the seeded canvas-text pixel emitter is the last
    // fallback. It runs only after every earlier accepted stage (including the
    // worker-environment sweep above) declines, so no existing claim, stage
    // precedence or ambiguity behaviour changes.
    let mut stage_canvas = Vec::new();
    stage_canvas.extend(graphics::canvas_text_pixel::evaluate(
        model, site, f, proposals,
    ));
    if !stage_canvas.is_empty() {
        return stage_canvas;
    }

    // SELECT-META-PROBE-001: the one-formal `_selectMeta` accessor setter is the
    // true final fallback. It runs only after every earlier source-backed
    // evaluator, including the canvas-text pixel stage above, declined.
    let mut stage_meta = Vec::new();
    stage_meta.extend(runtime::select_meta_setter::evaluate(
        model, site, f, proposals,
    ));
    if !stage_meta.is_empty() {
        return stage_meta;
    }

    // STORAGE-REMOVAL-EMITTER-001: the real `addEventListener("storage", ...)`
    // callback frame emitter is the true final fallback. It runs only after
    // every earlier source-backed evaluator, including the select-meta setter
    // above, declined the function.
    let mut stage_storage = Vec::new();
    stage_storage.extend(events::storage_removal_emitter::evaluate(
        model, site, f, proposals,
    ));
    if !stage_storage.is_empty() {
        return stage_storage;
    }

    // CURRIED-SEEDED-EMITTER-002: last fallback for the curried interned
    // string-writer plus elapsed-clock seeded emitter variant. This root hunk
    // appends after the root's last stage (ENTROPY-002); an integration
    // worktree with later stages must append this block after its actual last
    // fallback instead and must keep a stage variable name that does not
    // collide with an existing stage.
    let mut stage_p = Vec::new();
    stage_p.extend(encoding::curried_seeded_emitter::evaluate(
        model, site, f, proposals,
    ));
    if !stage_p.is_empty() {
        return stage_p;
    }

    // FONT-METRIC-CANVAS-SAMPLER-001: true final fallback for the standalone
    // measureText font-presence sampler. It cannot borrow
    // aggregate collector labels or the p10 font configuration binding.
    let stage_font = graphics::font_metric_sampler::evaluate(model, site, f, proposals);
    if !stage_font.is_empty() {
        return stage_font;
    }
    let stage_lexical = runtime::lexical_variants::evaluate(model, site, f, proposals);
    if !stage_lexical.is_empty() {
        return stage_lexical;
    }
    let stage_visibility = events::visibility_persistence_variants::evaluate(model, site, f, proposals);
    if !stage_visibility.is_empty() {
        return stage_visibility;
    }
    let stage_history = observables::history_persistence::evaluate(model, site, f, proposals);
    if !stage_history.is_empty() {
        return stage_history;
    }
    let stage_matching = events::matching_key_callback::evaluate(model, site, f, proposals);
    if !stage_matching.is_empty() {
        return stage_matching;
    }
    let stage_string_table = observables::string_table_selection::evaluate(model, site, f, proposals);
    if !stage_string_table.is_empty() {
        return stage_string_table;
    }
    let stage_remember = events::remember_checkbox::evaluate(model, site, f, proposals);
    if !stage_remember.is_empty() {
        return stage_remember;
    }
    let stage_url_query = network::url_query_rebuild::evaluate(model, site, f, proposals);
    if !stage_url_query.is_empty() {
        return stage_url_query;
    }
    let prior_network = network::captured_key_callback_removal::evaluate(model, site, f, proposals);
    if !prior_network.is_empty() { return prior_network; }
    let prior_observable = observables::observable_map_operator::evaluate(model, site, f, proposals);
    if !prior_observable.is_empty() { return prior_observable; }
    let subscription_operators = observables::subscription_operators::evaluate(model, site, f, proposals);
    if !subscription_operators.is_empty() { return subscription_operators; }
    let prior_bounded = encoding::bounded_list_time_emitter::evaluate(model, site, f, proposals);
    if !prior_bounded.is_empty() { return prior_bounded; }
    let prior_audio = encoding::audio_result_emitter::evaluate(model, site, f, proposals);
    if !prior_audio.is_empty() { return prior_audio; }
    let prior_paste = events::input_paste_marker::evaluate(model, site, f, proposals);
    if !prior_paste.is_empty() { return prior_paste; }
    let prior_canvas = graphics::canvas_text_sampling_setup::evaluate(model, site, f, proposals);
    if !prior_canvas.is_empty() { return prior_canvas; }
    let prior_reverse = encoding::reverse_array_emitter::evaluate(model, site, f, proposals);
    if !prior_reverse.is_empty() { return prior_reverse; }
    let work_batch = scheduling::work_batch_protocols::evaluate(model, site, f);
    if !work_batch.is_empty() { return work_batch; }
    let numeric_writers = encoding::numeric_byte_writers::evaluate(model, site, f, proposals);
    if !numeric_writers.is_empty() { return numeric_writers; }
    let adapters = runtime::adapter_protocols::evaluate(model, site, f);
    if !adapters.is_empty() { return adapters; }
    let observer_forwarding = observables::captured_observer_forwarding::evaluate(model, site, f, proposals);
    if !observer_forwarding.is_empty() { return observer_forwarding; }
    let memory_attribution = encoding::memory_attribution_format::evaluate(model, site, f, proposals);
    if !memory_attribution.is_empty() { return memory_attribution; }
    let source_teardown = observables::captured_source_teardown::evaluate(model, site, f);
    if !source_teardown.is_empty() { return source_teardown; }
    let joined = observables::joined_observables::evaluate(model, site, f, proposals);
    if !joined.is_empty() { return joined; }
    let route_pattern = network::route_pattern_canonicalizer::evaluate(model, site, f, proposals);
    if !route_pattern.is_empty() { return route_pattern; }
    let tagged_float = encoding::software_float_byte_writer::evaluate_tagged_arithmetic(model, site, f);
    if !tagged_float.is_empty() { return tagged_float; }
    let software_float = encoding::software_float_byte_writer::evaluate(model, site, f);
    if !software_float.is_empty() { return software_float; }
    let xhr_open = network::xhr_open_metadata::evaluate(model, site, f);
    if !xhr_open.is_empty() { return xhr_open; }
    let handle_termination = observables::captured_handle_termination::evaluate(model, site, f);
    if !handle_termination.is_empty() { return handle_termination; }
    let prior_unregister = operations::captured_key_list_unregister::evaluate(model, site, f);
    if !prior_unregister.is_empty() { return prior_unregister; }
    let prior_range = observables::captured_range_predicate::evaluate(model, site, f);
    if !prior_range.is_empty() { return prior_range; }
    let prior_request = network::request_open_metadata::evaluate(model, site, f, proposals);
    if !prior_request.is_empty() { return prior_request; }
    let prior_lifecycle = observables::observer_batch_lifecycle::evaluate(model, site, f);
    if !prior_lifecycle.is_empty() { return prior_lifecycle; }
    let prior_traits = runtime::function_shape_traits::evaluate(model, site, f);
    if !prior_traits.is_empty() { return prior_traits; }
    let storage = browser::persistent_event_state::evaluate(model, site, f);
    if !storage.is_empty() { return storage; }
    let callback_list = operations::callback_list_dispatch::evaluate(model, site, f);
    if !callback_list.is_empty() { return callback_list; }
    let candidate = network::ice_candidate::evaluate(model, site, f);
    if !candidate.is_empty() { return candidate; }
    let mut pair08 = Vec::new();
    pair08.extend(events::orientation_moments::evaluate(model, site, f, proposals));
    pair08.extend(events::wheel_counter_frame::evaluate(model, site, f, proposals));
    pair08.extend(scheduling::interval_extrema::evaluate(model, site, f, proposals));
    pair08.extend(network::script_digest_dispatch::evaluate(model, site, f, proposals));
    if !pair08.is_empty() { return pair08; }
    let whole_body = runtime::whole_body_rules::evaluate(model, site, f, proposals);
    if !whole_body.is_empty() { return whole_body; }
    let thrown = operations::throw_type_error::evaluate(model, site, f);
    if !thrown.is_empty() { return thrown; }
    let scene = graphics::canvas_scene_anchor_frame::evaluate(model, site, f, proposals);
    if !scene.is_empty() { return scene; }
    let channel = graphics::sine_power_channel_render::evaluate(model, site, f, proposals);
    if !channel.is_empty() { return channel; }
    encoding::code_units::evaluate_code_units_to_string(model, site, f)
}

/// Post-shared entry point for the worker-posted seeded environment bundle.
/// The caller supplies only sites that no earlier stage has claimed.
pub(super) fn evaluate_worker_environment(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    runtime::worker_environment::evaluate(model, site, f, proposals)
}
