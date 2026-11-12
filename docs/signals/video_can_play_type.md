# VideoCanPlayType

[Signals index](README.md) · [data-shape example](video_can_play_type.js) · [schema format](schema.md)

## What it collects

Creates one &lt;video&gt; element and asks canPlayType for 34 fixed audio/video MIME and codec strings, then sends one small answer code per string, in list order. The list mixes real codec strings with a few nonsense ones (audio/webz, hvc1x, lavc1337, mp4a.67, mp3 audio with a vp9 codec).

## Why

Codec support differs by browser, OS and build, so the answer vector fingerprints the media stack; the nonsense entries likely catch spoofed canPlayType functions that answer "maybe"/"probably" to anything.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `count` | number | Number of codes that follow; 34 normally (can be 0 if probing failed early). |
| `codes` | number, repeated (once per probed string, in list order (34 entries)) | One canPlayType answer code per MIME/codec string: 0 = "" (no), 1 = "maybe", 2 = "probably", 3 = the call threw, 4 = any other answer. The strings, in order: 'audio/aac; codecs="flac"', 'audio/flac', 'audio/mpeg; codecs="mp3"', 'audio/mpeg; codecs="vp9"', 'audio/webm; codecs="vp8"', 'audio/webz', 'audio/x-m4a; codecs="mp3"', 'audio/x-m4a; codecs="vp8, mp4a.40"', 'audio/x-m4a; codecs="vp9, mp4a.40.2"', 'video/mp4', 'video/mp4; codecs="ac-3"', 'video/mp4; codecs="avc1.42011E"', 'video/mp4; codecs="avc1.42E009"', 'video/mp4; codecs="avc1.42E034"', 'video/mp4; codecs="avc1.42F01E"', 'video/mp4; codecs="avc1.4D001E"', 'video/mp4; codecs="avc3.42001E"', 'video/mp4; codecs="avc3.42E01E, mp4a.40.29"', 'video/mp4; codecs="flac"', 'video/mp4; codecs="hev1"', 'video/mp4; codecs="hvc1x"', 'video/mp4; codecs="lavc1337"', 'video/mp4; codecs="mp4a.40.02"', 'video/mp4; codecs="mp4a.40.5"', 'video/mp4; codecs="mp4a.67"', 'video/mp4; codecs="opus"', 'video/ogg; codecs="flac"', 'video/ogg; codecs="opus"', 'video/ogg; codecs="vp8"', 'video/webm', 'video/webm; codecs="av01.0.04M.08"', 'video/webm; codecs="vp09.02.10.08"', 'video/x-dv', 'video/x-m4v; codecs="avc1.42AC23"'. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
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
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.VideoCanPlayType` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`.
