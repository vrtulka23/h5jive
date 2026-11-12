use super::*;

impl VolumeRotation {
    pub(crate) fn identity() -> Self {
        Self([1.0, 0.0, 0.0, 0.0])
    }

    fn around(axis: [f32; 3], angle: f32) -> Self {
        let (sin, cos) = (angle * 0.5).sin_cos();
        Self([cos, axis[0] * sin, axis[1] * sin, axis[2] * sin])
    }

    pub(crate) fn from_angles(yaw: f32, pitch: f32) -> Self {
        Self::around([1.0, 0.0, 0.0], pitch).multiply(Self::around([0.0, 1.0, 0.0], -yaw))
    }

    fn multiply(self, other: Self) -> Self {
        let [a, b, c, d] = self.0;
        let [e, f, g, h] = other.0;
        Self([
            a * e - b * f - c * g - d * h,
            a * f + b * e + c * h - d * g,
            a * g - b * h + c * e + d * f,
            a * h + b * g - c * f + d * e,
        ])
    }

    fn normalized(self) -> Self {
        let length = self.0.iter().map(|v| v * v).sum::<f32>().sqrt();
        if length <= f32::EPSILON {
            return Self::identity();
        }
        Self(self.0.map(|v| v / length))
    }

    pub(super) fn transform(self, point: [f32; 3]) -> [f32; 3] {
        let [w, x, y, z] = self.0;
        let q = [x, y, z];
        let cross = |a: [f32; 3], b: [f32; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let t = cross(q, point).map(|v| 2.0 * v);
        let u = cross(q, t);
        std::array::from_fn(|i| point[i] + w * t[i] + u[i])
    }

    pub(crate) fn drag(
        self,
        previous: [f32; 2],
        current: [f32; 2],
        center: [f32; 2],
        radius: f32,
    ) -> Self {
        let sphere = |position: [f32; 2]| {
            let mut x = (position[0] - center[0]) / radius;
            let mut y = (center[1] - position[1]) / radius;
            let distance = x * x + y * y;
            if distance > 1.0 {
                let inverse = distance.sqrt().recip();
                x *= inverse;
                y *= inverse;
                [x, y, 0.0]
            } else {
                [x, y, (1.0 - distance).sqrt()]
            }
        };
        let a = sphere(previous);
        let b = sphere(current);
        let dot = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let delta = if dot < -0.9999 {
            let reference = if a[0].abs() < 0.9 {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            let axis = [
                a[1] * reference[2] - a[2] * reference[1],
                a[2] * reference[0] - a[0] * reference[2],
                a[0] * reference[1] - a[1] * reference[0],
            ];
            Self([0.0, axis[0], axis[1], axis[2]])
        } else {
            Self([1.0 + dot, cross[0], cross[1], cross[2]])
        };
        delta.normalized().multiply(self).normalized()
    }
}

/// Bound the part of the selected volume intersected by rays through the canvas.
/// The full depth interval is retained, including voxels behind the front face.
pub(super) fn visible_volume_slice(
    slice: &VolumeSlice,
    shape: &[usize],
    size: [f32; 2],
    center: [f32; 2],
    scale: [f32; 3],
    rotation: VolumeRotation,
) -> VolumeSlice {
    let bounds = volume_visible_bounds(slice, shape);
    let mut depth = [f32::INFINITY, f32::NEG_INFINITY];
    for mask in 0..8 {
        let point = std::array::from_fn(|axis| bounds[axis][(mask >> axis) & 1] * scale[axis]);
        let z = rotation.transform(point)[2];
        depth[0] = depth[0].min(z);
        depth[1] = depth[1].max(z);
    }
    let inverse = VolumeRotation([
        rotation.0[0],
        -rotation.0[1],
        -rotation.0[2],
        -rotation.0[3],
    ]);
    let mut local = [[f32::INFINITY, f32::NEG_INFINITY]; 3];
    for mask in 0..8 {
        let screen = [
            if mask & 1 == 0 { 0.0 } else { size[0] },
            if mask & 2 == 0 { 0.0 } else { size[1] },
            depth[(mask >> 2) & 1],
        ];
        let camera = [screen[0] - center[0], center[1] - screen[1], screen[2]];
        let point = inverse.transform(camera);
        for axis in 0..3 {
            let value = point[axis] / scale[axis].max(1e-6);
            local[axis][0] = local[axis][0].min(value);
            local[axis][1] = local[axis][1].max(value);
        }
    }
    let mut cropped = slice.clone();
    for slot in 0..3 {
        let Some(axis) = slice.axes[slot] else {
            continue;
        };
        let [first, last] = slice.ranges[axis];
        let [low, high] = local[slot];
        let needed_first = ((0.5 - high) * shape[axis] as f32)
            .floor()
            .max(first as f32) as usize;
        let needed_end = ((0.5 - low) * shape[axis] as f32)
            .ceil()
            .min((last + 1) as f32) as usize;
        if needed_end <= needed_first {
            continue;
        }
        let full = last - first + 1;
        let needed = needed_end - needed_first;
        let quantum = (needed / 8).max(1).next_power_of_two();
        let padded = needed.saturating_add((needed / 8).max(2));
        let width = padded.div_ceil(quantum).saturating_mul(quantum).min(full);
        if width >= full {
            continue;
        }
        let step = (width / 4).max(1);
        let desired = needed_first.saturating_sub((width - needed) / 2);
        let mut start = first + ((desired.saturating_sub(first)) / step) * step;
        if start + width < needed_end {
            start = (start + step).min(last + 1 - width);
        }
        cropped.ranges[axis] = [start, start + width - 1];
    }
    cropped
}

pub(super) fn draw_axis_mark(
    ui: &Ui,
    draw: &dear_app::imgui::DrawListMut<'_>,
    mark: &AxisMark,
    center: [f32; 2],
    scale: [f32; 3],
    rotation: VolumeRotation,
    color: [f32; 4],
) {
    match mark {
        AxisMark::Line {
            from,
            to,
            thickness,
        } => {
            let first = project_rotated(*from, center, scale, rotation).0;
            let last = project_rotated(*to, center, scale, rotation).0;
            draw.add_line(first, last, color)
                .thickness(*thickness)
                .build();
        }
        AxisMark::Text { point, label } => {
            let screen = project_rotated(*point, center, scale, rotation).0;
            let size = ui.calc_text_size(label);
            let position = [
                if screen[0] >= center[0] {
                    screen[0] + 4.0
                } else {
                    screen[0] - size[0] - 4.0
                },
                if screen[1] >= center[1] {
                    screen[1] + 2.0
                } else {
                    screen[1] - size[1] - 2.0
                },
            ];
            draw.add_text(position, color, label);
        }
        AxisMark::AxisName { point, label } => {
            let screen = project_rotated(*point, center, scale, rotation).0;
            let width = ui.calc_text_size(label)[0];
            let x = if screen[0] >= center[0] {
                screen[0] + 5.0
            } else {
                screen[0] - width - 5.0
            };
            draw.add_text([x, screen[1]], color, label);
        }
    }
}

#[cfg(test)]
pub(super) fn project(
    point: [f32; 3],
    center: [f32; 2],
    scale: [f32; 3],
    yaw: f32,
    pitch: f32,
) -> ([f32; 2], f32) {
    project_rotated(
        point,
        center,
        scale,
        VolumeRotation::from_angles(yaw, pitch),
    )
}

pub(super) fn project_rotated(
    point: [f32; 3],
    center: [f32; 2],
    scale: [f32; 3],
    rotation: VolumeRotation,
) -> ([f32; 2], f32) {
    let scaled = std::array::from_fn(|axis| point[axis] * scale[axis]);
    let rotated = rotation.transform(scaled);
    ([center[0] + rotated[0], center[1] - rotated[1]], rotated[2])
}

pub(super) fn front_axis_corner(
    rotation: VolumeRotation,
    preferred: [f32; 3],
    bounds: [[f32; 2]; 3],
) -> [f32; 3] {
    std::array::from_fn(|axis| {
        let mut unit = [0.0; 3];
        unit[axis] = 1.0;
        let facing = rotation.transform(unit)[2];
        if facing.abs() < 1e-5 {
            bounds[axis][usize::from(preferred[axis] >= 0.0)]
        } else {
            bounds[axis][usize::from(facing > 0.0)]
        }
    })
}

/// Offset the projection so the visible solid rotates around its translated center.
pub(super) fn volume_projection_center(
    screen_center: [f32; 2],
    pivot: [f32; 3],
    scale: [f32; 3],
    rotation: VolumeRotation,
) -> [f32; 2] {
    let scaled = std::array::from_fn(|axis| pivot[axis] * scale[axis]);
    let rotated = rotation.transform(scaled);
    [screen_center[0] - rotated[0], screen_center[1] + rotated[1]]
}

#[derive(Clone, Copy)]
pub(crate) struct DepthShading {
    pub(super) limit: f32,
    back_brightness: f32,
}

impl DepthShading {
    pub(crate) fn new(rotation: VolumeRotation, back_brightness: f32, scale: [f32; 3]) -> Self {
        let facing = std::array::from_fn::<_, 3, _>(|axis| {
            let mut unit = [0.0; 3];
            unit[axis] = 1.0;
            rotation.transform(unit)[2]
        });
        Self {
            limit: 0.5
                * facing
                    .iter()
                    .zip(scale)
                    .map(|(v, s)| (v * s).abs())
                    .sum::<f32>(),
            back_brightness,
        }
    }

    pub(super) fn brightness(self, depth: f32) -> f32 {
        let fraction = ((depth / self.limit).clamp(-1.0, 1.0) + 1.0) * 0.5;
        self.back_brightness + (1.0 - self.back_brightness) * fraction
    }

    pub(crate) fn shade(self, mut color: [f32; 4], depth: f32) -> [f32; 4] {
        let brightness = self.brightness(depth);
        for channel in &mut color[..3] {
            *channel *= brightness;
        }
        color
    }
}

pub(super) fn draw_volume_box(
    draw: &dear_app::imgui::DrawListMut<'_>,
    center: [f32; 2],
    scale: [f32; 3],
    rotation: VolumeRotation,
    bounds: [[f32; 2]; 3],
    palette: &Palette,
    back_brightness: f32,
) {
    let depth_shading = DepthShading::new(rotation, back_brightness, scale);
    let corners: [[f32; 3]; 8] = std::array::from_fn(|i| {
        std::array::from_fn(|axis| bounds[axis][usize::from(i & (1 << axis) != 0)])
    });
    for i in 0..8 {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                let a = project_rotated(corners[i], center, scale, rotation);
                let b = project_rotated(corners[i | bit], center, scale, rotation);
                let color = depth_shading.shade(palette.border, (a.1 + b.1) * 0.5);
                draw.add_line(a.0, b.0, color).build();
            }
        }
    }
}

pub(super) fn opacity_from_slider(position: f32) -> f32 {
    position.clamp(0.0, 1.0).powi(2)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_volume_legend(
    draw: &dear_app::imgui::DrawListMut<'_>,
    origin: [f32; 2],
    size: [f32; 2],
    range: Option<(f64, f64)>,
    logarithmic: bool,
    palette: &Palette,
    preset: ColormapPreset,
    plot_ctx: &PlotContext,
    decimals: usize,
) {
    draw.add_text(
        [origin[0], origin[1] + 4.0],
        palette.text,
        if logarithmic {
            "Data values (log)"
        } else {
            "Data values"
        },
    );
    let top = origin[1] + 28.0;
    let bar_height = (size[1] - 60.0).max(30.0);
    let color_scale = VolumeColorScale::new(range, logarithmic);
    if let Some((min, _max)) = range {
        let steps = bar_height.ceil().min(256.0) as usize;
        for step in 0..steps {
            let y = top + bar_height * step as f32 / steps as f32;
            let fraction = 1.0 - (step as f32 + 0.5) / steps as f32;
            let color = continuous_color(plot_ctx, fraction, preset, palette);
            draw.add_rect(
                [origin[0] + 2.0, y],
                [origin[0] + 22.0, y + bar_height / steps as f32 + 0.5],
                color,
            )
            .filled(true)
            .build();
        }
        for (y, fraction) in [
            (top, 1.0),
            (top + bar_height * 0.5, 0.5),
            (top + bar_height, 0.0),
        ] {
            let value = color_scale.value(fraction).unwrap_or(min);
            draw.add_text(
                [origin[0] + 29.0, y - 7.0],
                palette.text,
                format_value(value, decimals),
            );
        }
    } else {
        draw.add_text([origin[0], top], palette.text, "No finite values");
    }
}
