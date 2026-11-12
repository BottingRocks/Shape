# Channel rendering and animation frame protocols

Two full cleaned JavaScript programs preserve the owning browser callbacks and their captured dependencies for separate rendering protocols.

[`channel-render.js`](channel-render.js) supplies the sine/power channel-render parent. [`accepted-audio-children.json`](accepted-audio-children.json) pins accepted child outcomes, including binding spans and evaluator evidence. [`scaled-animation-frame.js`](scaled-animation-frame.js) covers scaled animation-frame behavior.

Consumed by:

- [naming_channel_render.rs](../../../naming_channel_render.rs)
- [naming_scaled_animation_frame.rs](../../../naming_scaled_animation_frame.rs)

Positive and alpha variants establish owned behavior. Parent effect, constructor, exposure, and frame mutations must refuse without borrowing a valid child as proof for its parent.

Preserve full source bytes and the child-outcome JSON together; its spans address the exact channel source. Do not replace the programs with isolated API-call sketches.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../audio-result/README.md) and the [program fixture overview](../README.md).
