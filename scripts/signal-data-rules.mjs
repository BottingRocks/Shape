export const forbidden = new Set(['numberBits', 'wireBytes', 'grammarPath', 'staticXor', 'byteMapping',
  'tagByte', 'nullMarker', 'nullFlag', 'rawBytes', 'rawBuffer', 'root_span', 'program_sha256',
  'wow64Placeholder', 'mobilePlaceholder', 'entriesPlaceholder', 'totalBytesPlaceholder',
  'breakdownCountPlaceholder', 'containerMarker']);

// These exact paths are controls, not measured counts. Evidence: the adjacent
// cards describe list framing, literal tags or fixed-list lengths; counted.rs
// removes proven array counts. Never infer this from a Count/Length/Marker suffix:
// entryBytes, breakdownCount, pluginsLength, directNewCount, buildMarker, etc. stay.
export const controls = {
  anchor_idna_unicode_version: ['testCount'],
  animation_frame_timings: ['sampleCount'],
  canvas_readback_probe: ['recordCount', 'records[].directColourCount', 'records[].roundTripColourCount',
    'records[].directColours[].pixelLength', 'records[].roundTripColours[].pixelLength'],
  canvas_scene_pixel_samples: ['sampleCount', 'samples[].channelCount', 'terminator'],
  current_script_async_policy_state: ['scriptAsync', 'status1', 'status2', 'policyAbsentTag', 'scriptFlags', 'asyncState'],
  emoji_channel_sample_halves: ['firstHalfCount', 'secondHalfCount'],
  emoji_channel_samples: ['sampleCount'],
  emoji_glyph_rgba_samples: ['rowCount', 'rows[].sampleCount'],
  font_availability: ['fontCount'],
  font_metrics: ['familyCount'],
  global_own_property_bits: ['probeCount'],
  keyboard_event_samples: ['eventCount'],
  keystroke_dynamics: ['specialKeySlotCount'],
  lifecycle_entropy_samples: ['sampleCount'],
  math_own_property_names: ['nameCount'],
  memory_breakdown_summary: ['entryCount', 'entry[].attributionCount', 'entry[].typeCount'],
  navigator_plugins: ['entryCount'],
  offline_audio_render_samples: ['sampleCount'],
  ponos_xhr_round_trip: ['signalIndexCount'],
  shuffled_date_string: ['zero'],
  ui_event_samples_mouse: ['rawMoveCount', 'throttledMoveCount', 'buttonEventCount'],
  ui_event_samples_touch: ['recordCount', 'records[].pointCount'],
  video_can_play_type: ['count'],
  visibility_event_log: ['recordCount', 'stateNullTag'],
  webgl_context: ['paramsMissing', 'getParameterMissing', 'debugInfoMissing', 'antialiasMissing'],
  webgl_render_hash: ['kind'],
  webrtc_ice_candidate_addresses: ['recordCount'],
};
