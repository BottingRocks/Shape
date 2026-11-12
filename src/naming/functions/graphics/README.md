# Graphics and frame-sampling rules

These rules sit under [function naming](../README.md) and are declared by the
sibling [graphics.rs](../graphics.rs). They recognize canvas operations,
animation-frame callbacks and vector-history dataflow.

| File | Responsibility |
| --- | --- |
| [canvas_image_probe.rs](canvas_image_probe.rs) | Bounded canvas image-probe fallback |
| [canvas_text_sampling_setup.rs](canvas_text_sampling_setup.rs), [canvas_text_pixel.rs](canvas_text_pixel.rs) | Text sampling setup and pixel handling |
| [font_metric_sampler.rs](font_metric_sampler.rs) | Font metric sampling |
| [bounded_pixel_vector_history.rs](bounded_pixel_vector_history.rs) | Bounded unique pixel-vector history |
| [scaled_animation_frame_sample.rs](scaled_animation_frame_sample.rs) | Scaled animation-frame sampling |

The bounded-history rule checks the second formal's field updates, the vector
comparison callback and copying through the unshadowed `Array.prototype.slice`
path. A history push or a pixel-looking member is not sufficient evidence for
that complete protocol. Related distinct-vector rules have their own leaves;
keep their effects and return paths separate when diagnosing competing claims.

Evaluator precedence lives in [stages.rs](../../stages.rs), while shared
vector predicates live in [vector_state.rs](../../support/vector_state.rs).
See [scaled frame tests](../../../../tests/naming_scaled_animation_frame.rs),
[vector split tests](../../../../tests/naming_vector_split_null.rs) and
[selector/graphics tests](../../../../tests/naming_selector_and_graphics_roles.rs).
