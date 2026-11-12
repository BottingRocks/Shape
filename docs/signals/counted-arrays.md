# Counted arrays in readable payloads

[Schema format](schema.md) · [Signals index](README.md)

The shared presentation rule follows each compiled `CountedRepeat`'s prior NUMBER
reference to its named leaf and repeat control in the same record scope. A count
must belong to exactly one displayed data array. Packed flags, shared counters,
uncertified repeats, and independent statistics retain their numeric fields.
Names and observed equality do not establish this relationship.

The logical length must match the original IEEE bits exactly and satisfy the
source count bound. Reconstruction replaces the old count with `array.length`
before the physical encoder verifies bytes. All other IEEE bits, masks, XORs,
wire positions, array element types, and contracts remain intact. Older artifacts
without the `countedArrays` presentation flag retain their original payload shape.

Signal roots remain objects. Nested records containing only a certified count and
its array collapse to that array; records with other fields keep those fields.
Empty arrays remain `[]`, explicit null remains null, no-write optional fields
remain omitted, and real empty data records and packed flag objects remain `{}`.

The audited model has these 35 source-proven length associations. This inventory
contains field names only; concrete routing and encoding values are per build.

| Signal | Hidden count | Array | Count-only record collapses |
| --- | --- | --- | --- |
| AnimationFrameTimings | `sampleCount` | `frameTime` | no |
| CanvasReadbackProbe | `recordCount` | `records` | no |
| CanvasReadbackProbe | `records[].directColourCount` | `records[].directColours` | no |
| CanvasReadbackProbe | `records[].roundTripColourCount` | `records[].roundTripColours` | no |
| CanvasScenePixelSamples | `sampleCount` | `samples` | no |
| CanvasScenePixelSamples | `samples[].channelCount` | `samples[].channel` | yes |
| EmojiChannelSampleHalves | `firstHalfCount` | `firstHalfSamples` | no |
| EmojiChannelSampleHalves | `secondHalfCount` | `secondHalfSamples` | no |
| EmojiChannelSamples | `sampleCount` | `samples` | no |
| EmojiGlyphRgbaSamples | `rowCount` | `rows` | no |
| EmojiGlyphRgbaSamples | `rows[].sampleCount` | `rows[].pixels` | yes |
| FontAvailability | `fontCount` | `fontResults` | no |
| FontMetrics | `familyCount` | `familyMetricsHash` | no |
| KeyboardEventSamples | `eventCount` | `events` | no |
| KeystrokeDynamics | `specialKeySlotCount` | `specialKeyCounts` | no |
| MathOwnPropertyNames | `nameCount` | `propertyName` | no |
| MemoryBreakdownSummary | `entryCount` | `entry` | no |
| MemoryBreakdownSummary | `entry[].attributionCount` | `entry[].attribution` | no |
| MemoryBreakdownSummary | `entry[].typeCount` | `entry[].memoryType` | no |
| NavigatorPlugins | `entryCount` | `plugin` | no |
| OfflineAudioRenderSamples | `sampleCount` | `sample` | no |
| PonosXhrRoundTrip | `signalIndexCount` | `signalIndex` | no |
| UiEventSamplesMouse | `buttonEventCount` | `buttonEvents` | no |
| UiEventSamplesMouse | `rawMoveCount` | `rawMoves` | no |
| UiEventSamplesMouse | `throttledMoveCount` | `throttledMoves` | no |
| UiEventSamplesTouch | `recordCount` | `records` | no |
| UiEventSamplesTouch | `records[].pointCount` | `records[].points` | no |
| UserAgentData | `brands.brands` | `brands.brandEntries` | yes |
| UserAgentData | `fullVersionList.fullVersionList` | `fullVersionList.fullVersionEntries` | yes |
| VideoCanPlayType | `count` | `codes` | no |
| VisibilityEventLog | `recordCount` | `records` | no |
| WebGlContext | `dimensions.dimensions` | `dimensions.dimension` | yes |
| WebGlContext | `shaderPrecisions.shaderPrecisions` | `shaderPrecisions.shaderPrecision` | yes |
| WebGlContext | `supportedExtensions.supportedExtensions` | `supportedExtensions.extension` | yes |
| WebRtcIceCandidateAddresses | `recordCount` | `candidates` | no |

Retained count associations:

| Signal | Count | Reason |
| --- | --- | --- |
| GlobalOwnPropertyBits | `probeCount` | Shared probe count controls full packed bytes and a separate remainder; no single displayed array owns it. |
| MatchMedia | `anyHoverCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `anyPointerCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `colorGamutCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `displayModeCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `forcedColorsCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `gridCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `hoverCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `invertedColorsCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `lightLevelCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `orientationCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `overflowBlockCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `overflowInlineCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `pointerCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `prefersColorSchemeCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `prefersContrastCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `prefersReducedMotionCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `prefersReducedTransparencyCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `scanCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `scriptingCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |
| MatchMedia | `updateCount` | Candidate count controls packed/no-write flush loops; the displayed Bits value is an object, including an empty object. |

Independent statistics remain visible, including `NavigatorPlugins.pluginsLength`,
`KeystrokeDynamics.specialKeyTotal`, `AnimationFrameTimings.timeQuantum`,
`CanvasReadbackProbe` new-colour statistics, `VisibilityEventLog.visibilityChangeCount`,
`MemoryBreakdownSummary.breakdownCount` and `totalBytes`, version markers, hashes,
and sentinels. In particular a total before filtering or truncation is not replaced
by the sampled array length. Other numeric fields without a certified repeat
association remain unchanged regardless of their names. `ShuffledDateString.zero`
keeps its existing name.

The two `UiEventSamplesWheel` event counters also remain when that route is
present; no displayed array proves either one's value.

Some routes and nested alternatives may be absent in a capture. Their rules are
source-derived; an empty capture cannot exercise their item values. The full
artifact retains both strict count certificates and accepted source-count facts,
including their distinct bounds and evidence. The public schema is descriptive,
not a standalone executable codec.
