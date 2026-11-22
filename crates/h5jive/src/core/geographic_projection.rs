//! Shared geographic projections for map and globe layers.

#[derive(Clone, Copy, Debug)]
pub(crate) struct GlobePoint {
    pub(crate) point: [f32; 2],
    pub(crate) depth: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GlobeProjection {
    pub(crate) center: [f32; 2],
    pub(crate) radius: f32,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}

impl GlobeProjection {
    pub(crate) fn project(self, longitude: f32, latitude: f32) -> GlobePoint {
        let lon = longitude.to_radians();
        let lat = latitude.to_radians();
        let (sl, cl) = lon.sin_cos();
        let (sp, cp) = lat.sin_cos();
        let (sy, cy) = self.yaw.sin_cos();
        let (sx, cx) = self.pitch.sin_cos();
        let x = cy * cp * sl + sy * cp * cl;
        let z = -sy * cp * sl + cy * cp * cl;
        let y = cx * sp - sx * z;
        GlobePoint {
            point: [
                self.center[0] + x * self.radius,
                self.center[1] - y * self.radius,
            ],
            depth: sx * sp + cx * z,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CoverageCell {
    pub(crate) longitude: f32,
    pub(crate) latitude: f32,
    pub(crate) count: u16,
}

/// Coverage rasters use geographic cell centers in an equirectangular grid.
pub(crate) fn coverage_cells<'a>(
    width: usize,
    height: usize,
    counts: &'a [u16],
) -> impl Iterator<Item = CoverageCell> + 'a {
    counts
        .iter()
        .enumerate()
        .filter_map(move |(index, &count)| {
            if count == 0 || width == 0 || index >= width.saturating_mul(height) {
                return None;
            }
            Some(CoverageCell {
                longitude: -180.0 + 360.0 * ((index % width) as f32 + 0.5) / width as f32,
                latitude: 90.0 - 180.0 * ((index / width) as f32 + 0.5) / height as f32,
                count,
            })
        })
}
