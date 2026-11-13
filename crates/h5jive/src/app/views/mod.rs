mod color;
mod data;
mod geospatial;
mod globe;
mod profiles;
mod slice2d;
mod spatial3d;
mod world_map;

pub(super) use color::continuous_color;
pub(super) use geospatial::{
    draw_geo_controls, geo_controls_height, observation_color, pixel_hover_distance,
};
pub(super) use world_map::coastlines;

#[cfg(test)]
pub(super) use profiles::draw_profile_plot;
pub(super) use slice2d::*;
#[cfg(test)]
pub(super) use spatial3d::project_rotated;
pub(super) use spatial3d::DepthShading;
