//! The signal catalog: one file per canonical signal (`<signal_snake>.rs`,
//! holding that signal's `recognize_*` and `bind_*`, the helpers only it
//! uses, and its approved field card `CARD`), the registry [`SIGNALS`] of all
//! 93 signals and the card types ([`card`]). Helpers shared by two
//! or more signals live in [`crate::signals::analysis`], one module per
//! technique (shape text, leaf checks, slot closures, property census, …).

use crate::signals::analysis::{Binder, Recognizer};

pub mod card;
pub mod view;

pub use card::Card;

mod anchor_idna_unicode_version;
mod anchor_properties;
mod animation_frame_timings;
mod brave_duckduckgo_navigator_flags;
mod browser_api_own_property_flags;
mod browser_api_proxy_flags;
mod builtin_probe_checksum;
mod canvas_readback_probe;
mod canvas_scene_pixel_samples;
mod canvas_to_data_url;
mod clean_console_stack_access_flags;
mod computed_style_properties;
mod console_member_nativeness_hash_pair;
mod console_property_names_hash;
mod console_stack_access_flags;
mod constant_empty_string;
mod cors_cookie_value;
mod cross_origin_isolation_flags;
mod current_script_async_policy_state;
mod device_memory;
mod device_orientation;
mod document_referrer_history;
mod dom_element_statistics;
mod emoji_channel_sample_halves;
mod emoji_channel_samples;
mod emoji_glyph_rgba_samples;
mod feature_policy;
mod fingerprint_api_integrity;
mod font_availability;
mod font_metrics;
mod function_to_string_comparison;
mod global_native_function_count;
mod global_own_property_bits;
mod global_properties_count;
mod global_property_checks;
mod grecaptcha_accessor_hook_flags;
mod historic_date_to_string;
mod host_environment;
mod iframe_realm_native_function_count;
mod iframe_realm_property_name_hashes;
mod incognito_storage_probe;
mod interval_clock_drift_stats;
mod intl_locale_time_zone;
mod json_parse_fingerprint;
mod keyboard_event_samples;
mod keystroke_dynamics;
mod lifecycle_entropy_samples;
mod lifecycle_phase_timings;
mod local_storage_persistent_id;
mod location_property_names;
mod location_url_components;
mod match_media;
mod math_own_property_names;
mod memory_breakdown_summary;
mod mouse_travel_and_click_count;
mod namespace_property_name_hashes;
mod native_api_integrity_flags;
mod navigation_timing_offsets;
mod navigator_plugins;
mod navigator_properties;
mod nested_frame_anchor_property_names_hash;
mod non_native_builtin_count;
mod null_deref_type_error_fields;
mod offline_audio_render_samples;
mod outer_args_global_process;
mod parse_int_hex_constant;
mod paste_count_unfocused_input_flag;
mod persistent_storage_state;
mod ponos_xhr_round_trip;
mod product_page_snapshot;
mod proxy_tamper_flags;
mod remember_me_checkbox_state;
mod scroll_offsets;
mod select_meta_injection_flags;
mod selenium_ide_artifact_flags;
mod sensor_ready_event;
mod shuffled_date_string;
mod storage_access_api_presence;
mod timestamps_epoch;
mod ui_event_is_trusted;
mod ui_event_samples_mouse;
mod ui_event_samples_touch;
mod ui_event_samples_wheel;
mod user_agent_data;
mod video_can_play_type;
mod viewport_dimensions;
mod visibility_event_log;
mod vm_source_integrity;
mod webauthn_interface_presence;
mod webgl_context;
mod webgl_render_hash;
mod webrtc_ice_candidate_addresses;
mod worker_realm_profile;

/// One registered signal.
pub(crate) struct SignalEntry {
    pub canonical: &'static str,
    pub recognize: Recognizer,
    pub bind: Binder,
    pub card: &'static Card,
}

/// All 93 canonical signals: the 90 of the
/// baseline corpus plus 3 found in the wider corpus.
pub(crate) const SIGNALS: &[SignalEntry] = &[
    SignalEntry {
        canonical: "CanvasReadbackProbe",
        recognize: canvas_readback_probe::recognize_canvas_readback_probe,
        bind: canvas_readback_probe::bind_canvas_readback_probe,
        card: &canvas_readback_probe::CARD,
    },
    SignalEntry {
        canonical: "CanvasScenePixelSamples",
        recognize: canvas_scene_pixel_samples::recognize_canvas_scene_pixel_samples,
        bind: canvas_scene_pixel_samples::bind_canvas_scene_pixel_samples,
        card: &canvas_scene_pixel_samples::CARD,
    },
    SignalEntry {
        canonical: "CanvasToDataUrl",
        recognize: canvas_to_data_url::recognize_canvas_to_data_url,
        bind: canvas_to_data_url::bind_canvas_to_data_url,
        card: &canvas_to_data_url::CARD,
    },
    SignalEntry {
        canonical: "EmojiChannelSamples",
        recognize: emoji_channel_samples::recognize_emoji_channel_samples,
        bind: emoji_channel_samples::bind_emoji_channel_samples,
        card: &emoji_channel_samples::CARD,
    },
    SignalEntry {
        canonical: "EmojiChannelSampleHalves",
        recognize: emoji_channel_sample_halves::recognize_emoji_channel_sample_halves,
        bind: emoji_channel_sample_halves::bind_emoji_channel_sample_halves,
        card: &emoji_channel_sample_halves::CARD,
    },
    SignalEntry {
        canonical: "EmojiGlyphRgbaSamples",
        recognize: emoji_glyph_rgba_samples::recognize_emoji_glyph_rgba_samples,
        bind: emoji_glyph_rgba_samples::bind_emoji_glyph_rgba_samples,
        card: &emoji_glyph_rgba_samples::CARD,
    },
    SignalEntry {
        canonical: "WebGlContext",
        recognize: webgl_context::recognize_webgl_context,
        bind: webgl_context::bind_webgl_context,
        card: &webgl_context::CARD,
    },
    SignalEntry {
        canonical: "WebGlRenderHash",
        recognize: webgl_render_hash::recognize_webgl_render_hash,
        bind: webgl_render_hash::bind_webgl_render_hash,
        card: &webgl_render_hash::CARD,
    },
    SignalEntry {
        canonical: "OfflineAudioRenderSamples",
        recognize: offline_audio_render_samples::recognize_offline_audio_render_samples,
        bind: offline_audio_render_samples::bind_offline_audio_render_samples,
        card: &offline_audio_render_samples::CARD,
    },
    SignalEntry {
        canonical: "FontAvailability",
        recognize: font_availability::recognize_font_availability,
        bind: font_availability::bind_font_availability,
        card: &font_availability::CARD,
    },
    SignalEntry {
        canonical: "FontMetrics",
        recognize: font_metrics::recognize_font_metrics,
        bind: font_metrics::bind_font_metrics,
        card: &font_metrics::CARD,
    },
    SignalEntry {
        canonical: "VideoCanPlayType",
        recognize: video_can_play_type::recognize_video_can_play_type,
        bind: video_can_play_type::bind_video_can_play_type,
        card: &video_can_play_type::CARD,
    },
    SignalEntry {
        canonical: "BrowserApiProxyFlags",
        recognize: browser_api_proxy_flags::recognize_browser_api_proxy_flags,
        bind: browser_api_proxy_flags::bind_browser_api_proxy_flags,
        card: &browser_api_proxy_flags::CARD,
    },
    SignalEntry {
        canonical: "NativeApiIntegrityFlags",
        recognize: native_api_integrity_flags::recognize_native_api_integrity_flags,
        bind: native_api_integrity_flags::bind_native_api_integrity_flags,
        card: &native_api_integrity_flags::CARD,
    },
    SignalEntry {
        canonical: "FunctionToStringComparison",
        recognize: function_to_string_comparison::recognize_function_to_string_comparison,
        bind: function_to_string_comparison::bind_function_to_string_comparison,
        card: &function_to_string_comparison::CARD,
    },
    SignalEntry {
        canonical: "FingerprintApiIntegrity",
        recognize: fingerprint_api_integrity::recognize_fingerprint_api_integrity,
        bind: fingerprint_api_integrity::bind_fingerprint_api_integrity,
        card: &fingerprint_api_integrity::CARD,
    },
    SignalEntry {
        canonical: "ProxyTamperFlags",
        recognize: proxy_tamper_flags::recognize_proxy_tamper_flags,
        bind: proxy_tamper_flags::bind_proxy_tamper_flags,
        card: &proxy_tamper_flags::CARD,
    },
    SignalEntry {
        canonical: "GrecaptchaAccessorHookFlags",
        recognize: grecaptcha_accessor_hook_flags::recognize_grecaptcha_accessor_hook_flags,
        bind: grecaptcha_accessor_hook_flags::bind_grecaptcha_accessor_hook_flags,
        card: &grecaptcha_accessor_hook_flags::CARD,
    },
    SignalEntry {
        canonical: "NonNativeBuiltinCount",
        recognize: non_native_builtin_count::recognize_non_native_builtin_count,
        bind: non_native_builtin_count::bind_non_native_builtin_count,
        card: &non_native_builtin_count::CARD,
    },
    SignalEntry {
        canonical: "GlobalNativeFunctionCount",
        recognize: global_native_function_count::recognize_global_native_function_count,
        bind: global_native_function_count::bind_global_native_function_count,
        card: &global_native_function_count::CARD,
    },
    SignalEntry {
        canonical: "IframeRealmNativeFunctionCount",
        recognize: iframe_realm_native_function_count::recognize_iframe_realm_native_function_count,
        bind: iframe_realm_native_function_count::bind_iframe_realm_native_function_count,
        card: &iframe_realm_native_function_count::CARD,
    },
    SignalEntry {
        canonical: "BuiltinProbeChecksum",
        recognize: builtin_probe_checksum::recognize_builtin_probe_checksum,
        bind: builtin_probe_checksum::bind_builtin_probe_checksum,
        card: &builtin_probe_checksum::CARD,
    },
    SignalEntry {
        canonical: "VmSourceIntegrity",
        recognize: vm_source_integrity::recognize_vm_source_integrity,
        bind: vm_source_integrity::bind_vm_source_integrity,
        card: &vm_source_integrity::CARD,
    },
    SignalEntry {
        canonical: "ParseIntHexConstant",
        recognize: parse_int_hex_constant::recognize_parse_int_hex_constant,
        bind: parse_int_hex_constant::bind_parse_int_hex_constant,
        card: &parse_int_hex_constant::CARD,
    },
    SignalEntry {
        canonical: "NullDerefTypeErrorFields",
        recognize: null_deref_type_error_fields::recognize_null_deref_type_error_fields,
        bind: null_deref_type_error_fields::bind_null_deref_type_error_fields,
        card: &null_deref_type_error_fields::CARD,
    },
    SignalEntry {
        canonical: "ConsoleStackAccessFlags",
        recognize: console_stack_access_flags::recognize_console_stack_access_flags,
        bind: console_stack_access_flags::bind_console_stack_access_flags,
        card: &console_stack_access_flags::CARD,
    },
    SignalEntry {
        canonical: "CleanConsoleStackAccessFlags",
        recognize: clean_console_stack_access_flags::recognize_clean_console_stack_access_flags,
        bind: clean_console_stack_access_flags::bind_clean_console_stack_access_flags,
        card: &clean_console_stack_access_flags::CARD,
    },
    SignalEntry {
        canonical: "BrowserApiOwnPropertyFlags",
        recognize: browser_api_own_property_flags::recognize_browser_api_own_property_flags,
        bind: browser_api_own_property_flags::bind_browser_api_own_property_flags,
        card: &browser_api_own_property_flags::CARD,
    },
    SignalEntry {
        canonical: "AnchorProperties",
        recognize: anchor_properties::recognize_anchor_properties,
        bind: anchor_properties::bind_anchor_properties,
        card: &anchor_properties::CARD,
    },
    SignalEntry {
        canonical: "ComputedStyleProperties",
        recognize: computed_style_properties::recognize_computed_style_properties,
        bind: computed_style_properties::bind_computed_style_properties,
        card: &computed_style_properties::CARD,
    },
    SignalEntry {
        canonical: "LocationPropertyNames",
        recognize: location_property_names::recognize_location_property_names,
        bind: location_property_names::bind_location_property_names,
        card: &location_property_names::CARD,
    },
    SignalEntry {
        canonical: "MathOwnPropertyNames",
        recognize: math_own_property_names::recognize_math_own_property_names,
        bind: math_own_property_names::bind_math_own_property_names,
        card: &math_own_property_names::CARD,
    },
    SignalEntry {
        canonical: "ConsolePropertyNamesHash",
        recognize: console_property_names_hash::recognize_console_property_names_hash,
        bind: console_property_names_hash::bind_console_property_names_hash,
        card: &console_property_names_hash::CARD,
    },
    SignalEntry {
        canonical: "ConsoleMemberNativenessHashPair",
        recognize: console_member_nativeness_hash_pair::recognize_console_member_nativeness_hash_pair,
        bind: console_member_nativeness_hash_pair::bind_console_member_nativeness_hash_pair,
        card: &console_member_nativeness_hash_pair::CARD,
    },
    SignalEntry {
        canonical: "NamespacePropertyNameHashes",
        recognize: namespace_property_name_hashes::recognize_namespace_property_name_hashes,
        bind: namespace_property_name_hashes::bind_namespace_property_name_hashes,
        card: &namespace_property_name_hashes::CARD,
    },
    SignalEntry {
        canonical: "IframeRealmPropertyNameHashes",
        recognize: iframe_realm_property_name_hashes::recognize_iframe_realm_property_name_hashes,
        bind: iframe_realm_property_name_hashes::bind_iframe_realm_property_name_hashes,
        card: &iframe_realm_property_name_hashes::CARD,
    },
    SignalEntry {
        canonical: "NestedFrameAnchorPropertyNamesHash",
        recognize: nested_frame_anchor_property_names_hash::recognize_nested_frame_anchor_property_names_hash,
        bind: nested_frame_anchor_property_names_hash::bind_nested_frame_anchor_property_names_hash,
        card: &nested_frame_anchor_property_names_hash::CARD,
    },
    SignalEntry {
        canonical: "GlobalPropertiesCount",
        recognize: global_properties_count::recognize_global_properties_count,
        bind: global_properties_count::bind_global_properties_count,
        card: &global_properties_count::CARD,
    },
    SignalEntry {
        canonical: "GlobalOwnPropertyBits",
        recognize: global_own_property_bits::recognize_global_own_property_bits,
        bind: global_own_property_bits::bind_global_own_property_bits,
        card: &global_own_property_bits::CARD,
    },
    SignalEntry {
        canonical: "GlobalPropertyChecks",
        recognize: global_property_checks::recognize_global_property_checks,
        bind: global_property_checks::bind_global_property_checks,
        card: &global_property_checks::CARD,
    },
    SignalEntry {
        canonical: "AnchorIdnaUnicodeVersion",
        recognize: anchor_idna_unicode_version::recognize_anchor_idna_unicode_version,
        bind: anchor_idna_unicode_version::bind_anchor_idna_unicode_version,
        card: &anchor_idna_unicode_version::CARD,
    },
    SignalEntry {
        canonical: "StorageAccessApiPresence",
        recognize: storage_access_api_presence::recognize_storage_access_api_presence,
        bind: storage_access_api_presence::bind_storage_access_api_presence,
        card: &storage_access_api_presence::CARD,
    },
    SignalEntry {
        canonical: "WebAuthnInterfacePresence",
        recognize: webauthn_interface_presence::recognize_webauthn_interface_presence,
        bind: webauthn_interface_presence::bind_webauthn_interface_presence,
        card: &webauthn_interface_presence::CARD,
    },
    SignalEntry {
        canonical: "BraveDuckDuckGoNavigatorFlags",
        recognize: brave_duckduckgo_navigator_flags::recognize_brave_duckduckgo_navigator_flags,
        bind: brave_duckduckgo_navigator_flags::bind_brave_duckduckgo_navigator_flags,
        card: &brave_duckduckgo_navigator_flags::CARD,
    },
    SignalEntry {
        canonical: "CrossOriginIsolationFlags",
        recognize: cross_origin_isolation_flags::recognize_cross_origin_isolation_flags,
        bind: cross_origin_isolation_flags::bind_cross_origin_isolation_flags,
        card: &cross_origin_isolation_flags::CARD,
    },
    SignalEntry {
        canonical: "SeleniumIdeArtifactFlags",
        recognize: selenium_ide_artifact_flags::recognize_selenium_ide_artifact_flags,
        bind: selenium_ide_artifact_flags::bind_selenium_ide_artifact_flags,
        card: &selenium_ide_artifact_flags::CARD,
    },
    SignalEntry {
        canonical: "SelectMetaInjectionFlags",
        recognize: select_meta_injection_flags::recognize_select_meta_injection_flags,
        bind: select_meta_injection_flags::bind_select_meta_injection_flags,
        card: &select_meta_injection_flags::CARD,
    },
    SignalEntry {
        canonical: "DeviceMemory",
        recognize: device_memory::recognize_device_memory,
        bind: device_memory::bind_device_memory,
        card: &device_memory::CARD,
    },
    SignalEntry {
        canonical: "NavigatorProperties",
        recognize: navigator_properties::recognize_navigator_properties,
        bind: navigator_properties::bind_navigator_properties,
        card: &navigator_properties::CARD,
    },
    SignalEntry {
        canonical: "NavigatorPlugins",
        recognize: navigator_plugins::recognize_navigator_plugins,
        bind: navigator_plugins::bind_navigator_plugins,
        card: &navigator_plugins::CARD,
    },
    SignalEntry {
        canonical: "UserAgentData",
        recognize: user_agent_data::recognize_user_agent_data,
        bind: user_agent_data::bind_user_agent_data,
        card: &user_agent_data::CARD,
    },
    SignalEntry {
        canonical: "IntlLocaleTimeZone",
        recognize: intl_locale_time_zone::recognize_intl_locale_time_zone,
        bind: intl_locale_time_zone::bind_intl_locale_time_zone,
        card: &intl_locale_time_zone::CARD,
    },
    SignalEntry {
        canonical: "HistoricDateToString",
        recognize: historic_date_to_string::recognize_historic_date_to_string,
        bind: historic_date_to_string::bind_historic_date_to_string,
        card: &historic_date_to_string::CARD,
    },
    SignalEntry {
        canonical: "ShuffledDateString",
        recognize: shuffled_date_string::recognize_shuffled_date_string,
        bind: shuffled_date_string::bind_shuffled_date_string,
        card: &shuffled_date_string::CARD,
    },
    SignalEntry {
        canonical: "HostEnvironment",
        recognize: host_environment::recognize_host_environment,
        bind: host_environment::bind_host_environment,
        card: &host_environment::CARD,
    },
    SignalEntry {
        canonical: "WorkerRealmProfile",
        recognize: worker_realm_profile::recognize_worker_realm_profile,
        bind: worker_realm_profile::bind_worker_realm_profile,
        card: &worker_realm_profile::CARD,
    },
    SignalEntry {
        canonical: "WebRtcIceCandidateAddresses",
        recognize: webrtc_ice_candidate_addresses::recognize_webrtc_ice_candidate_addresses,
        bind: webrtc_ice_candidate_addresses::bind_webrtc_ice_candidate_addresses,
        card: &webrtc_ice_candidate_addresses::CARD,
    },
    SignalEntry {
        canonical: "MatchMedia",
        recognize: match_media::recognize_match_media,
        bind: match_media::bind_match_media,
        card: &match_media::CARD,
    },
    SignalEntry {
        canonical: "FeaturePolicy",
        recognize: feature_policy::recognize_feature_policy,
        bind: feature_policy::bind_feature_policy,
        card: &feature_policy::CARD,
    },
    SignalEntry {
        canonical: "MemoryBreakdownSummary",
        recognize: memory_breakdown_summary::recognize_memory_breakdown_summary,
        bind: memory_breakdown_summary::bind_memory_breakdown_summary,
        card: &memory_breakdown_summary::CARD,
    },
    SignalEntry {
        canonical: "IncognitoStorageProbe",
        recognize: incognito_storage_probe::recognize_incognito_storage_probe,
        bind: incognito_storage_probe::bind_incognito_storage_probe,
        card: &incognito_storage_probe::CARD,
    },
    SignalEntry {
        canonical: "ViewportDimensions",
        recognize: viewport_dimensions::recognize_viewport_dimensions,
        bind: viewport_dimensions::bind_viewport_dimensions,
        card: &viewport_dimensions::CARD,
    },
    SignalEntry {
        canonical: "LocationUrlComponents",
        recognize: location_url_components::recognize_location_url_components,
        bind: location_url_components::bind_location_url_components,
        card: &location_url_components::CARD,
    },
    SignalEntry {
        canonical: "DocumentReferrerHistory",
        recognize: document_referrer_history::recognize_document_referrer_history,
        bind: document_referrer_history::bind_document_referrer_history,
        card: &document_referrer_history::CARD,
    },
    SignalEntry {
        canonical: "DomElementStatistics",
        recognize: dom_element_statistics::recognize_dom_element_statistics,
        bind: dom_element_statistics::bind_dom_element_statistics,
        card: &dom_element_statistics::CARD,
    },
    SignalEntry {
        canonical: "OuterArgsGlobalProcess",
        recognize: outer_args_global_process::recognize_outer_args_global_process,
        bind: outer_args_global_process::bind_outer_args_global_process,
        card: &outer_args_global_process::CARD,
    },
    SignalEntry {
        canonical: "JsonParseFingerprint",
        recognize: json_parse_fingerprint::recognize_json_parse_fingerprint,
        bind: json_parse_fingerprint::bind_json_parse_fingerprint,
        card: &json_parse_fingerprint::CARD,
    },
    SignalEntry {
        canonical: "ConstantEmptyString",
        recognize: constant_empty_string::recognize_constant_empty_string,
        bind: constant_empty_string::bind_constant_empty_string,
        card: &constant_empty_string::CARD,
    },
    SignalEntry {
        canonical: "CorsCookieValue",
        recognize: cors_cookie_value::recognize_cors_cookie_value,
        bind: cors_cookie_value::bind_cors_cookie_value,
        card: &cors_cookie_value::CARD,
    },
    SignalEntry {
        canonical: "UiEventIsTrusted",
        recognize: ui_event_is_trusted::recognize_ui_event_is_trusted,
        bind: ui_event_is_trusted::bind_ui_event_is_trusted,
        card: &ui_event_is_trusted::CARD,
    },
    SignalEntry {
        canonical: "RememberMeCheckboxState",
        recognize: remember_me_checkbox_state::recognize_remember_me_checkbox_state,
        bind: remember_me_checkbox_state::bind_remember_me_checkbox_state,
        card: &remember_me_checkbox_state::CARD,
    },
    SignalEntry {
        canonical: "ProductPageSnapshot",
        recognize: product_page_snapshot::recognize_product_page_snapshot,
        bind: product_page_snapshot::bind_product_page_snapshot,
        card: &product_page_snapshot::CARD,
    },
    SignalEntry {
        canonical: "AnimationFrameTimings",
        recognize: animation_frame_timings::recognize_animation_frame_timings,
        bind: animation_frame_timings::bind_animation_frame_timings,
        card: &animation_frame_timings::CARD,
    },
    SignalEntry {
        canonical: "DeviceOrientation",
        recognize: device_orientation::recognize_device_orientation,
        bind: device_orientation::bind_device_orientation,
        card: &device_orientation::CARD,
    },
    SignalEntry {
        canonical: "IntervalClockDriftStats",
        recognize: interval_clock_drift_stats::recognize_interval_clock_drift_stats,
        bind: interval_clock_drift_stats::bind_interval_clock_drift_stats,
        card: &interval_clock_drift_stats::CARD,
    },
    SignalEntry {
        canonical: "KeyboardEventSamples",
        recognize: keyboard_event_samples::recognize_keyboard_event_samples,
        bind: keyboard_event_samples::bind_keyboard_event_samples,
        card: &keyboard_event_samples::CARD,
    },
    SignalEntry {
        canonical: "KeystrokeDynamics",
        recognize: keystroke_dynamics::recognize_keystroke_dynamics,
        bind: keystroke_dynamics::bind_keystroke_dynamics,
        card: &keystroke_dynamics::CARD,
    },
    SignalEntry {
        canonical: "LifecycleEntropySamples",
        recognize: lifecycle_entropy_samples::recognize_lifecycle_entropy_samples,
        bind: lifecycle_entropy_samples::bind_lifecycle_entropy_samples,
        card: &lifecycle_entropy_samples::CARD,
    },
    SignalEntry {
        canonical: "LifecyclePhaseTimings",
        recognize: lifecycle_phase_timings::recognize_lifecycle_phase_timings,
        bind: lifecycle_phase_timings::bind_lifecycle_phase_timings,
        card: &lifecycle_phase_timings::CARD,
    },
    SignalEntry {
        canonical: "MouseTravelAndClickCount",
        recognize: mouse_travel_and_click_count::recognize_mouse_travel_and_click_count,
        bind: mouse_travel_and_click_count::bind_mouse_travel_and_click_count,
        card: &mouse_travel_and_click_count::CARD,
    },
    SignalEntry {
        canonical: "PasteCountUnfocusedInputFlag",
        recognize: paste_count_unfocused_input_flag::recognize_paste_count_unfocused_input_flag,
        bind: paste_count_unfocused_input_flag::bind_paste_count_unfocused_input_flag,
        card: &paste_count_unfocused_input_flag::CARD,
    },
    SignalEntry {
        canonical: "UiEventSamplesMouse",
        recognize: ui_event_samples_mouse::recognize_ui_event_samples_mouse,
        bind: ui_event_samples_mouse::bind_ui_event_samples_mouse,
        card: &ui_event_samples_mouse::CARD,
    },
    SignalEntry {
        canonical: "UiEventSamplesTouch",
        recognize: ui_event_samples_touch::recognize_ui_event_samples_touch,
        bind: ui_event_samples_touch::bind_ui_event_samples_touch,
        card: &ui_event_samples_touch::CARD,
    },
    SignalEntry {
        canonical: "UiEventSamplesWheel",
        recognize: ui_event_samples_wheel::recognize_ui_event_samples_wheel,
        bind: ui_event_samples_wheel::bind_ui_event_samples_wheel,
        card: &ui_event_samples_wheel::CARD,
    },
    SignalEntry {
        canonical: "VisibilityEventLog",
        recognize: visibility_event_log::recognize_visibility_event_log,
        bind: visibility_event_log::bind_visibility_event_log,
        card: &visibility_event_log::CARD,
    },
    SignalEntry {
        canonical: "ScrollOffsets",
        recognize: scroll_offsets::recognize_scroll_offsets,
        bind: scroll_offsets::bind_scroll_offsets,
        card: &scroll_offsets::CARD,
    },
    SignalEntry {
        canonical: "TimestampsEpoch",
        recognize: timestamps_epoch::recognize_timestamps_epoch,
        bind: timestamps_epoch::bind_timestamps_epoch,
        card: &timestamps_epoch::CARD,
    },
    SignalEntry {
        canonical: "NavigationTimingOffsets",
        recognize: navigation_timing_offsets::recognize_navigation_timing_offsets,
        bind: navigation_timing_offsets::bind_navigation_timing_offsets,
        card: &navigation_timing_offsets::CARD,
    },
    SignalEntry {
        canonical: "LocalStoragePersistentId",
        recognize: local_storage_persistent_id::recognize_local_storage_persistent_id,
        bind: local_storage_persistent_id::bind_local_storage_persistent_id,
        card: &local_storage_persistent_id::CARD,
    },
    SignalEntry {
        canonical: "PersistentStorageState",
        recognize: persistent_storage_state::recognize_persistent_storage_state,
        bind: persistent_storage_state::bind_persistent_storage_state,
        card: &persistent_storage_state::CARD,
    },
    SignalEntry {
        canonical: "CurrentScriptAsyncPolicyState",
        recognize: current_script_async_policy_state::recognize_current_script_async_policy_state,
        bind: current_script_async_policy_state::bind_current_script_async_policy_state,
        card: &current_script_async_policy_state::CARD,
    },
    SignalEntry {
        canonical: "PonosXhrRoundTrip",
        recognize: ponos_xhr_round_trip::recognize_ponos_xhr_round_trip,
        bind: ponos_xhr_round_trip::bind_ponos_xhr_round_trip,
        card: &ponos_xhr_round_trip::CARD,
    },
    SignalEntry {
        canonical: "SensorReadyEvent",
        recognize: sensor_ready_event::recognize_sensor_ready_event,
        bind: sensor_ready_event::bind_sensor_ready_event,
        card: &sensor_ready_event::CARD,
    },
];

/// The page-visible `effect` of a no-data signal (a declared-not-emitted
/// slot its recognizer claimed); `None` for every other signal.
pub(crate) fn no_data_effect(
    canonical: &str,
    view: &crate::signals::analysis::SlotView,
    ctx: &crate::signals::analysis::FinderCtx,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    match canonical {
        "SensorReadyEvent" => sensor_ready_event::effect_sensor_ready_event(view, ctx),
        _ => None,
    }
}

/// The card of every registered signal, in registry order.
pub fn cards() -> impl Iterator<Item = &'static Card> {
    SIGNALS.iter().map(|entry| entry.card)
}

/// The card of a canonical signal.
pub fn card(signal: &str) -> Option<&'static Card> {
    SIGNALS
        .iter()
        .find(|entry| entry.canonical == signal)
        .map(|entry| entry.card)
}

/// Close-pair tie tests of recognizer predicates on synthetic leaves (no
/// corpus): each single-string date/constant shape and each location or
/// referrer leaf set is claimed by exactly one signal.
#[cfg(test)]
mod close_pair_tests {
    use crate::signals::analysis::{LeafOp, LeafView};
    use crate::signals::extract::contracts::SourceClass;
    use SourceClass::{HostObservation, Literal, LoopIteration, SharedState, SourceTable};

    use super::constant_empty_string::is_constant_string;
    use super::document_referrer_history::is_document_referrer;
    use super::historic_date_to_string::is_historic_date;
    use super::location_url_components::is_location_url;
    use super::shuffled_date_string::is_shuffled_date;

    fn leaf(op: LeafOp, classes: &[SourceClass], transforms: &[&str], apis: &[&str]) -> LeafView {
        LeafView {
            op,
            repeat_depth: 0,
            choice_depth: 0,
            classes: classes.to_vec(),
            transforms: transforms.iter().map(|t| t.to_string()).collect(),
            apis: apis.iter().map(|a| a.to_string()).collect(),
            generic_key: None,
            write: Default::default(),
        }
    }

    fn historic() -> Vec<LeafView> {
        vec![leaf(
            LeafOp::String,
            &[Literal, HostObservation],
            &[".toString()", "new Date", "-"],
            &["new Date"],
        )]
    }

    fn shuffled_date() -> Vec<LeafView> {
        vec![
            leaf(LeafOp::Number, &[Literal], &[], &[]),
            leaf(
                LeafOp::Number,
                &[Literal, SourceTable, SharedState, LoopIteration],
                &[".shift()", ".length"],
                &[],
            ),
            leaf(
                LeafOp::String,
                &[Literal, HostObservation],
                &[".join()", "Array.from", ".toString()", "new Date", "-"],
                &["new Date"],
            ),
        ]
    }

    fn constant() -> Vec<LeafView> {
        vec![leaf(LeafOp::String, &[Literal], &[], &[])]
    }

    fn refs(leaves: &[LeafView]) -> Vec<&LeafView> {
        leaves.iter().collect()
    }

    /// Tie: HistoricDateToString vs ShuffledDateString vs ConstantEmptyString
    /// (each single-STRING date/constant shape claims exactly one name).
    #[test]
    fn date_and_constant_string_ties() {
        let one = ["RAW4 STRING"];
        let three = ["RAW4 NUMBER NUMBER STRING"];
        let h = historic();
        let s = shuffled_date();
        let c = constant();
        assert!(is_historic_date(&one, &refs(&h)));
        assert!(!is_shuffled_date(&one, &refs(&h)));
        assert!(!is_constant_string(&one, &refs(&h)));

        assert!(is_shuffled_date(&three, &refs(&s)));
        assert!(!is_historic_date(&three, &refs(&s)));
        assert!(!is_constant_string(&three, &refs(&s)));
        // The shuffled string alone (no zero/seed fields) is neither.
        let lone = vec![s[2].clone()];
        assert!(!is_historic_date(&one, &refs(&lone)));
        assert!(!is_shuffled_date(&one, &refs(&lone)));
        assert!(!is_constant_string(&one, &refs(&lone)));

        assert!(is_constant_string(&one, &refs(&c)));
        assert!(!is_historic_date(&one, &refs(&c)));
        assert!(!is_shuffled_date(&one, &refs(&c)));
        // Another single STRING with a host source is not the constant.
        let host = vec![leaf(
            LeafOp::String,
            &[HostObservation],
            &[],
            &["navigator.userAgent"],
        )];
        assert!(!is_constant_string(&one, &refs(&host)));
        assert!(!is_historic_date(&one, &refs(&host)));
        // A literal string reached through a transform is not the constant.
        let transformed = vec![leaf(LeafOp::String, &[Literal], &[".slice()"], &[])];
        assert!(!is_constant_string(&one, &refs(&transformed)));
    }

    /// Tie: LocationUrlComponents vs DocumentReferrerHistory, with and
    /// without the optional href / referrer text.
    #[test]
    fn location_and_referrer_ties() {
        let s = |api: &str| leaf(LeafOp::String, &[HostObservation], &[], &[api]);
        let location = vec![
            s("$A.location.hostname"),
            s("$A.location.href.substring()"),
            s("$A.location.protocol"),
            s("$A.location.port"),
        ];
        let location_no_href = vec![
            s("$A.location.hostname"),
            s("$A.location.protocol"),
            s("$A.location.port"),
        ];
        let referrer = vec![
            leaf(
                LeafOp::Byte,
                &[Literal, HostObservation],
                &["select", "==="],
                &["$A.document.referrer"],
            ),
            leaf(
                LeafOp::Number,
                &[HostObservation],
                &[],
                &["$A.history.length"],
            ),
            leaf(
                LeafOp::String,
                &[Literal, HostObservation],
                &[],
                &["$A.document.referrer.substring()"],
            ),
        ];
        let referrer_no_text = referrer[..2].to_vec();
        // The hostname's provenance lost through a shared register.
        let location_lost_hostname = vec![
            s("$A.location.href.substring()"),
            s("$A.location.protocol"),
            s("$A.location.port"),
        ];
        for leaves in [&location, &location_no_href, &location_lost_hostname] {
            assert!(is_location_url(&refs(leaves)));
            assert!(!is_document_referrer(&refs(leaves)));
        }
        // A repeated property-name string carrying every location path (a
        // census of another object) is not the components.
        let mut census = leaf(
            LeafOp::String,
            &[HostObservation],
            &[],
            &[
                "$A.location.hostname",
                "$A.location.protocol",
                "$A.location.port",
                "$A.location.href.substring()",
            ],
        );
        census.repeat_depth = 1;
        assert!(!is_location_url(&refs(&[census])));
        // Protocol plus one other component is not enough.
        assert!(!is_location_url(&refs(&[
            s("$A.location.protocol"),
            s("$A.location.port")
        ])));
        for leaves in [&referrer, &referrer_no_text] {
            assert!(is_document_referrer(&refs(leaves)));
            assert!(!is_location_url(&refs(leaves)));
        }
    }
}
