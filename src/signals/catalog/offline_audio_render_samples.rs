//! `OfflineAudioRenderSamples`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{layout_nodes::*, render_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `OfflineAudioRenderSamples`. Card: [`CARD`].
///
/// Shape: `seed, count, R{N}` in either order of the seed. Evidence:
/// `getChannelData` on the value path.
pub(crate) fn recognize_offline_audio_render_samples(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    claim_for(view, ctx, "OfflineAudioRenderSamples")
}

/// The sample count is the scalar directly before the sample repeat; the
/// other scalar is the entropy seed.
pub(crate) fn bind_offline_audio_render_samples(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    const SIGNAL: &str = "OfflineAudioRenderSamples";
    let mut binding = FieldBinding::new();
    for (message, layout_message) in layout.messages.iter().enumerate() {
        let fields = &layout_message.fields;
        let pairs = count_array_pairs(fields).ok_or_else(|| refuse(SIGNAL, "repeat without a count"))?;
        let [(count, samples)] = pairs.as_slice() else {
            return Err(refuse(SIGNAL, "expected one sample repeat"));
        };
        let seeds: Vec<usize> = (0..fields.len()).filter(|i| i != count && i != samples).collect();
        let [seed] = seeds.as_slice() else {
            return Err(refuse(SIGNAL, format!("message {message}: expected seed, count and samples")));
        };
        if !is_scalar(&fields[*seed].node) {
            return Err(refuse(SIGNAL, "seed is not a scalar"));
        }
        binding.insert(message, fields[*seed].key.clone(), "entropySeed");
        binding.insert(message, fields[*count].key.clone(), "sampleCount");
        binding.insert(message, fields[*samples].key.clone(), "sample");
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `OfflineAudioRenderSamples`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "OfflineAudioRenderSamples",
    summary: "Builds a tiny OfflineAudioContext (1 channel, 9 frames, 8003 Hz or 44100 Hz as fallback) with a 3410 Hz sine oscillator plus a 9-sample buffer whose contents are generated from a random entropy seed. It renders the audio and sends the seed together with every rendered sample of channel 0. If the context stays suspended, the unrendered source buffer is sent instead.",
    why: "Audio rendering results differ slightly between audio stacks, browsers and hardware; because the seed is sent too, the server can likely recompute the expected input and check the output, making it a challenge-style audio fingerprint and consistency check.",
    sample_payload: Some(r#"{
  "entropySeed": 0.8041851753480164,
  "sample": [
    1.3649976253509521,
    {
      "numberBits": 9221120237041090560,
      "special": "nan"
    },
    0.31301018595695496,
    -0.45291197299957275,
    0.33357828855514526,
    0.7065178751945496,
    0.3548361659049988,
    {
      "numberBits": 9221120237041090560,
      "special": "nan"
    },
    {
      "numberBits": 9221120237041090560,
      "special": "nan"
    }
  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "entropySeed",
            "The seed (a fractional number) used to generate the source buffer contents, so the server can reproduce the input.",
        ),
        CardField::number("sampleCount", "Number of samples in rendered channel 0; normally 9."),
        CardField::number("sample", "One rendered audio sample value (a float, can be 0, -0 or NaN).")
            .repeat("once per channel-0 sample, index 0 upward"),
    ]),
};
