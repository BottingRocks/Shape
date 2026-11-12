# OfflineAudioRenderSamples

[Signals index](README.md) · [data-shape example](offline_audio_render_samples.js) · [schema format](schema.md)

## What it collects

Builds a tiny OfflineAudioContext (1 channel, 9 frames, 8003 Hz or 44100 Hz as fallback) with a 3410 Hz sine oscillator plus a 9-sample buffer whose contents are generated from a random entropy seed. It renders the audio and sends the seed together with every rendered sample of channel 0. If the context stays suspended, the unrendered source buffer is sent instead.

## Why

Audio rendering results differ slightly between audio stacks, browsers and hardware; because the seed is sent too, the server can likely recompute the expected input and check the output, making it a challenge-style audio fingerprint and consistency check.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `entropySeed` | number | The seed (a fractional number) used to generate the source buffer contents, so the server can reproduce the input. |
| `sampleCount` | number | Number of samples in rendered channel 0; normally 9. |
| `sample` | number, repeated (once per channel-0 sample, index 0 upward) | One rendered audio sample value (a float, can be 0, -0 or NaN). |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
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
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.OfflineAudioRenderSamples` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`.
