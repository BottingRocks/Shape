//! `VideoCanPlayType`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{layout_nodes::*, render_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `VideoCanPlayType`. Card: [`CARD`].
///
/// Shape: `count, R{N}`. Evidence: `canPlayType` on the slot's path.
pub(crate) fn recognize_video_can_play_type(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    claim_for(view, ctx, "VideoCanPlayType")
}

/// `count, codes`: the answer codes are one repeat after the count.
pub(crate) fn bind_video_can_play_type(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_count_and_items("VideoCanPlayType", layout, "count", "codes")
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `VideoCanPlayType`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "VideoCanPlayType",
    summary: "Creates one <video> element and asks canPlayType for 34 fixed audio/video MIME and codec strings, then sends one small answer code per string, in list order. The list mixes real codec strings with a few nonsense ones (audio/webz, hvc1x, lavc1337, mp4a.67, mp3 audio with a vp9 codec).",
    why: "Codec support differs by browser, OS and build, so the answer vector fingerprints the media stack; the nonsense entries likely catch spoofed canPlayType functions that answer \"maybe\"/\"probably\" to anything.",
    sample_payload: Some(r#"{
  "codes": [
    0,
    2,
    2,
    0,
    0,
    0,
    0,
    0,
    0,
    1,
    0,
    0,
    1,
    2,
    2,
    2,
    2,
    2,
    2,
    0,
    0,
    0,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    1,
    2,
    2,
    0,
    1
  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "count",
            "Number of codes that follow; 34 normally (can be 0 if probing failed early).",
        ),
        CardField::number(
            "codes",
            "One canPlayType answer code per MIME/codec string: 0 = \"\" (no), 1 = \"maybe\", 2 = \"probably\", 3 = the call threw, 4 = any other answer. The strings, in order: 'audio/aac; codecs=\"flac\"', 'audio/flac', 'audio/mpeg; codecs=\"mp3\"', 'audio/mpeg; codecs=\"vp9\"', 'audio/webm; codecs=\"vp8\"', 'audio/webz', 'audio/x-m4a; codecs=\"mp3\"', 'audio/x-m4a; codecs=\"vp8, mp4a.40\"', 'audio/x-m4a; codecs=\"vp9, mp4a.40.2\"', 'video/mp4', 'video/mp4; codecs=\"ac-3\"', 'video/mp4; codecs=\"avc1.42011E\"', 'video/mp4; codecs=\"avc1.42E009\"', 'video/mp4; codecs=\"avc1.42E034\"', 'video/mp4; codecs=\"avc1.42F01E\"', 'video/mp4; codecs=\"avc1.4D001E\"', 'video/mp4; codecs=\"avc3.42001E\"', 'video/mp4; codecs=\"avc3.42E01E, mp4a.40.29\"', 'video/mp4; codecs=\"flac\"', 'video/mp4; codecs=\"hev1\"', 'video/mp4; codecs=\"hvc1x\"', 'video/mp4; codecs=\"lavc1337\"', 'video/mp4; codecs=\"mp4a.40.02\"', 'video/mp4; codecs=\"mp4a.40.5\"', 'video/mp4; codecs=\"mp4a.67\"', 'video/mp4; codecs=\"opus\"', 'video/ogg; codecs=\"flac\"', 'video/ogg; codecs=\"opus\"', 'video/ogg; codecs=\"vp8\"', 'video/webm', 'video/webm; codecs=\"av01.0.04M.08\"', 'video/webm; codecs=\"vp09.02.10.08\"', 'video/x-dv', 'video/x-m4v; codecs=\"avc1.42AC23\"'.",
        )
        .repeat("once per probed string, in list order (34 entries)"),
    ]),
};
