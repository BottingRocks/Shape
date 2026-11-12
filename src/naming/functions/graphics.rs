//! `graphics` thread rules.
use crate::naming::shared::*;
use crate::naming::*;

pub(in crate::naming) mod animation_frame_sample;
pub(in crate::naming) mod bounded_pixel_vector_history;
pub(in crate::naming) mod canvas_image_probe;
pub(in crate::naming) mod canvas_text_pixel;
pub(in crate::naming) mod canvas_text_sampling_setup;
pub(in crate::naming) mod distinct_vector_copy;
pub(in crate::naming) mod distinct_vector_sample;
pub(in crate::naming) mod font_metric_sampler;
pub(in crate::naming) mod frame_sample_request;
pub(in crate::naming) mod frontier_unique_sample;
pub(in crate::naming) mod scaled_animation_frame_sample;
pub(in crate::naming) mod unique_vector_history;

pub(in crate::naming) mod canvas_scene_anchor_frame;

pub(in crate::naming) mod sine_power_channel_render;
