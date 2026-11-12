//! Exact, screen-space rendering of native volume cells.
use super::*;
use dear_app::{ExternalTextureHandle, GpuApi, GpuGeneration};

#[derive(Clone, PartialEq)]
pub(super) struct VolumeRasterKey {
    pub(super) volume: VolumeKey,
    pub(super) layer_keys: Vec<VolumeKey>,
    pub(super) size: [u32; 2],
    pub(super) center: [f32; 2],
    pub(super) scale: [f32; 3],
    pub(super) rotation: [f32; 4],
    pub(super) opacity: f32,
    pub(super) back_brightness: f32,
    pub(super) range: Option<(f64, f64)>,
    pub(super) logarithmic: bool,
    pub(super) visibility: ValueVisibility,
    pub(super) preset: ColormapPreset,
    pub(super) color_stops: [[f32; 4]; 3],
    pub(super) special_colors: [[f32; 4]; 4],
}

pub(super) struct RegisteredVolumeRaster {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _handle: ExternalTextureHandle,
    pub(super) id: dear_app::imgui::TextureId,
    generation: GpuGeneration,
}

pub(super) struct VolumeRasterRequest {
    pub(super) key: VolumeRasterKey,
    pub(super) layers: Vec<Arc<ExactVolumeData>>,
    pub(super) colors: [[f32; 4]; 256],
}

impl App {
    pub(super) fn poll_volume_raster(&mut self, gpu: &mut GpuApi<'_>) {
        if self
            .volume_raster
            .as_ref()
            .is_some_and(|(_, raster)| raster.generation != gpu.generation())
        {
            self.volume_raster = None;
        }
        if self
            .volume_overview_raster
            .as_ref()
            .is_some_and(|(_, raster)| raster.generation != gpu.generation())
        {
            self.volume_overview_raster = None;
        }
        let ready = self
            .volume_raster_job
            .as_ref()
            .and_then(|(_, receiver)| receiver.try_recv().ok());
        if let Some(rgba) = ready {
            if let Some((key, _)) = self.volume_raster_job.take() {
                if let Some(raster) = upload_volume_raster(gpu, &key, &rgba) {
                    if let Some(old) = self.volume_raster.take() {
                        if self.applied_volume.as_ref() == Some(&old.0.volume.slice) {
                            self.volume_overview_raster = Some(old);
                        }
                    }
                    self.volume_raster = Some((key, raster));
                }
            }
        } else if self
            .volume_raster_job
            .as_ref()
            .is_some_and(|(_, receiver)| {
                matches!(receiver.try_recv(), Err(mpsc::TryRecvError::Disconnected))
            })
        {
            self.volume_raster_job = None;
        }
    }

    pub(super) fn request_volume_raster(&mut self, request: VolumeRasterRequest) {
        if self
            .volume_raster
            .as_ref()
            .is_some_and(|(key, _)| *key == request.key)
            || self.volume_raster_job.is_some()
        {
            return;
        }
        let key = request.key.clone();
        let generation = Arc::clone(&self.view_load_generation);
        let expected = generation.load(Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let image = render_exact_volume_with_cancel(&request, &|| {
                generation.load(Ordering::Relaxed) != expected
            });
            let _ = sender.send(image);
        });
        self.volume_raster_job = Some((key, receiver));
    }

    pub(super) fn volume_raster_preview(
        &self,
        key: &VolumeRasterKey,
        desired: &VolumeKey,
    ) -> Option<(dear_app::imgui::TextureId, [f32; 2], [f32; 2])> {
        let compatible = |cached: &VolumeRasterKey| {
            let compatible = cached.volume.file == key.volume.file
                && cached.volume.path == key.volume.path
                && cached.volume.slice.axes == key.volume.slice.axes
                && cached.volume.slice.hidden == key.volume.slice.hidden
                && cached.volume.slice.pinned == key.volume.slice.pinned;
            compatible
        };
        let covers = |cached: &VolumeRasterKey| {
            cached.layer_keys.iter().any(|layer| {
                layer
                    .slice
                    .ranges
                    .iter()
                    .zip(&desired.slice.ranges)
                    .all(|(old, new)| old[0] <= new[0] && old[1] >= new[1])
            })
        };
        let current = self
            .volume_raster
            .as_ref()
            .filter(|(cached, _)| compatible(cached));
        let overview = self
            .volume_overview_raster
            .as_ref()
            .filter(|(cached, _)| compatible(cached));
        let (cached, raster) = current
            .filter(|(cached, _)| covers(cached))
            .or(overview)
            .or(current)?;
        let (min, max) = volume_preview_rect(cached, key);
        Some((raster.id, min, max))
    }
}

/// Move and scale the last image with the camera while a fresh ray cast runs.
/// A rotation cannot be represented by a flat image transform, so retain it
/// in place until the new orientation is rendered.
fn volume_preview_rect(old: &VolumeRasterKey, new: &VolumeRasterKey) -> ([f32; 2], [f32; 2]) {
    if old.rotation != new.rotation || old.scale[0] <= 0.0 {
        return ([0.0; 2], new.size.map(|length| length as f32));
    }
    let factor = new.scale[0] / old.scale[0];
    if !factor.is_finite() || factor <= 0.0 {
        return ([0.0; 2], new.size.map(|length| length as f32));
    }
    let min = std::array::from_fn(|axis| new.center[axis] - factor * old.center[axis]);
    let max = std::array::from_fn(|axis| min[axis] + factor * old.size[axis] as f32);
    (min, max)
}

fn upload_volume_raster(
    gpu: &mut GpuApi<'_>,
    key: &VolumeRasterKey,
    rgba: &[u8],
) -> Option<RegisteredVolumeRaster> {
    let [width, height] = key.size;
    if rgba.len() != width as usize * height as usize * 4 {
        return None;
    }
    let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("Exact 3D volume ray cast"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // ImGui's legend sends its colors as vertex values. Keep the raster
        // texture in the same color space so both take the same output path.
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue().write_texture(
        texture.as_image_copy(),
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        texture.size(),
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let handle = gpu.register_external_texture(&view).ok()?;
    let id = gpu.resolve_external_texture(handle).ok()?;
    Some(RegisteredVolumeRaster {
        _texture: texture,
        _view: view,
        _handle: handle,
        id,
        generation: gpu.generation(),
    })
}

fn rotate(q: [f32; 4], point: [f32; 3]) -> [f32; 3] {
    let [w, x, y, z] = q;
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let t = cross([x, y, z], point).map(|value| value * 2.0);
    let u = cross([x, y, z], t);
    std::array::from_fn(|axis| point[axis] + w * t[axis] + u[axis])
}

#[derive(Clone, Copy)]
pub(super) struct VolumeColorScale {
    transformed_range: Option<(f64, f64)>,
    log_reference: Option<f64>,
}

impl VolumeColorScale {
    pub(super) fn new(range: Option<(f64, f64)>, logarithmic: bool) -> Self {
        let Some((min, max)) =
            range.filter(|(min, max)| min.is_finite() && max.is_finite() && max > min)
        else {
            return Self {
                transformed_range: None,
                log_reference: None,
            };
        };
        let log_reference =
            logarithmic.then(|| (min.abs().max(max.abs()) * 0.01).max(f64::MIN_POSITIVE));
        let transform =
            |value: f64| log_reference.map_or(value, |reference| (value / reference).asinh());
        Self {
            transformed_range: Some((transform(min), transform(max))),
            log_reference,
        }
    }

    pub(super) fn fraction(self, value: f64) -> f32 {
        let Some((min, max)) = self.transformed_range else {
            return 0.5;
        };
        let transformed = self
            .log_reference
            .map_or(value, |reference| (value / reference).asinh());
        ((transformed - min) / (max - min)).clamp(0.0, 1.0) as f32
    }

    pub(super) fn value(self, fraction: f64) -> Option<f64> {
        let (min, max) = self.transformed_range?;
        let transformed = min + (max - min) * fraction;
        Some(
            self.log_reference
                .map_or(transformed, |reference| transformed.sinh() * reference),
        )
    }
}

#[cfg(test)]
fn render_exact_volume(request: &VolumeRasterRequest) -> Vec<u8> {
    render_exact_volume_with_cancel(request, &|| false)
}

fn render_exact_volume_with_cancel(
    request: &VolumeRasterRequest,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Vec<u8> {
    let [width, height] = request.key.size;
    let mut image = vec![0; width as usize * height as usize * 4];
    let workers = thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(8);
    let rows_per_worker = (height as usize + workers - 1) / workers;
    let chunk_bytes = rows_per_worker * width as usize * 4;
    thread::scope(|scope| {
        for (worker, chunk) in image.chunks_mut(chunk_bytes).enumerate() {
            scope.spawn(move || {
                for (row, pixels) in chunk.chunks_mut(width as usize * 4).enumerate() {
                    if cancelled() {
                        break;
                    }
                    let y = worker * rows_per_worker + row;
                    for (x, pixel) in pixels.chunks_mut(4).enumerate() {
                        pixel.copy_from_slice(&raycast_pixel(
                            request,
                            x as f32 + 0.5,
                            y as f32 + 0.5,
                        ));
                    }
                }
            });
        }
    });
    image
}

fn volume_ray_interval(
    data: &ExactVolumeData,
    origin: [f32; 3],
    direction: [f32; 3],
) -> Option<(f32, f32)> {
    let mut entry = f32::NEG_INFINITY;
    let mut exit = f32::INFINITY;
    for axis in 0..3 {
        let [low, high] = data.bounds[axis];
        if direction[axis].abs() < 1e-9 {
            if origin[axis] < low || origin[axis] >= high {
                return None;
            }
            continue;
        }
        let a = (low - origin[axis]) / direction[axis];
        let b = (high - origin[axis]) / direction[axis];
        entry = entry.max(a.min(b));
        exit = exit.min(a.max(b));
    }
    (exit > entry.max(0.0)).then_some((entry.max(0.0), exit))
}

fn raycast_pixel(request: &VolumeRasterRequest, x: f32, y: f32) -> [u8; 4] {
    let key = &request.key;
    let inverse = [
        key.rotation[0],
        -key.rotation[1],
        -key.rotation[2],
        -key.rotation[3],
    ];
    let far = key.scale.iter().copied().sum::<f32>() + 1.0;
    let camera_origin = [x - key.center[0], key.center[1] - y, far];
    let origin = rotate(inverse, camera_origin);
    let direction = rotate(inverse, [0.0, 0.0, -1.0]);
    let origin = std::array::from_fn::<_, 3, _>(|axis| origin[axis] / key.scale[axis].max(1e-6));
    let direction =
        std::array::from_fn::<_, 3, _>(|axis| direction[axis] / key.scale[axis].max(1e-6));
    // The view retains at most four refined crops, an overview, and one
    // in-flight result. Keep intervals on the stack for every screen pixel.
    let mut interval_storage = [None; 8];
    for (interval, data) in interval_storage.iter_mut().zip(&request.layers) {
        *interval = volume_ray_interval(data, origin, direction);
    }
    let intervals = &interval_storage[..request.layers.len()];
    let Some(entry) = intervals
        .iter()
        .flatten()
        .map(|(entry, _)| *entry)
        .min_by(f32::total_cmp)
    else {
        return [0; 4];
    };
    let exit = intervals
        .iter()
        .flatten()
        .map(|(_, exit)| *exit)
        .fold(0.0, f32::max);
    let mut distance = entry + 1e-5;
    let mut accumulated = [0.0_f32; 4];
    let max_steps = request
        .layers
        .iter()
        .map(|data| data.dimensions.iter().sum::<usize>() + 3)
        .sum::<usize>()
        + 3;
    let color_scale = VolumeColorScale::new(key.range, key.logarithmic);
    let depth_shading =
        DepthShading::new(VolumeRotation(key.rotation), key.back_brightness, key.scale);
    for _ in 0..max_steps {
        if distance >= exit || accumulated[3] >= 0.995 {
            break;
        }
        let active = intervals.iter().position(|interval| {
            interval.is_some_and(|(start, end)| distance >= start && distance < end)
        });
        let Some(layer) = active else {
            let next = intervals
                .iter()
                .flatten()
                .filter_map(|(start, _)| (*start > distance).then_some(*start))
                .min_by(f32::total_cmp);
            let Some(next) = next else { break };
            distance = next + 1e-5;
            continue;
        };
        let data = &request.layers[layer];
        let point =
            std::array::from_fn::<_, 3, _>(|axis| origin[axis] + direction[axis] * distance);
        let index = std::array::from_fn::<_, 3, _>(|axis| {
            let [low, high] = data.bounds[axis];
            (((high - point[axis]) / (high - low) * data.dimensions[axis] as f32).floor() as isize)
                .clamp(0, data.dimensions[axis] as isize - 1) as usize
        });
        let offset = (0..3)
            .map(|axis| index[axis] * data.strides[axis])
            .sum::<usize>();
        let value = data.values[offset] as f64;
        let special = match data.specials[offset] {
            1 => Some(SpecialValue::Fill),
            2 => Some(SpecialValue::Nan),
            3 => Some(SpecialValue::PositiveInf),
            4 => Some(SpecialValue::NegativeInf),
            _ => None,
        };
        if key.visibility.allows_class(value, special) {
            let color = if let Some(special) = special {
                key.special_colors[match special {
                    SpecialValue::Fill => 0,
                    SpecialValue::Nan => 1,
                    SpecialValue::PositiveInf => 2,
                    SpecialValue::NegativeInf => 3,
                }]
            } else {
                request.colors[(color_scale.fraction(value) * 255.0).round() as usize]
            };
            // Camera-space z decreases by one unit for each unit travelled
            // along the ray, so no per-voxel rotation is needed here.
            let color = depth_shading.shade(color, far - distance);
            let contribution = (1.0 - accumulated[3]) * color[3] * key.opacity;
            for channel in 0..3 {
                accumulated[channel] += color[channel] * contribution;
            }
            accumulated[3] += contribution;
        }
        let mut next = f32::INFINITY;
        for axis in 0..3 {
            let step = direction[axis];
            if step.abs() < 1e-9 {
                continue;
            }
            let [low, high] = data.bounds[axis];
            let boundary_index = index[axis] + usize::from(step < 0.0);
            let boundary =
                high - (high - low) * boundary_index as f32 / data.dimensions[axis] as f32;
            let crossing = (boundary - origin[axis]) / step;
            if crossing > distance {
                next = next.min(crossing);
            }
        }
        next = next.min(intervals[layer].unwrap().1);
        for interval in intervals.iter().take(layer) {
            if let Some((start, _)) = interval {
                if *start > distance {
                    next = next.min(*start);
                }
            }
        }
        if !next.is_finite() {
            break;
        }
        distance = next + 1e-5;
    }
    let alpha = accumulated[3];
    if alpha > 0.0 {
        for channel in &mut accumulated[..3] {
            *channel /= alpha;
        }
    }
    accumulated.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refined_voxels_replace_coarse_values_only_inside_their_region() {
        let fine = Arc::new(ExactVolumeData {
            values: vec![1.0; 4],
            specials: vec![0; 4],
            dimensions: [1, 2, 2],
            strides: [4, 2, 1],
            bounds: [[0.0, 0.5], [-0.5, 0.5], [-0.5, 0.5]],
            color_range: Some((0.0, 1.0)),
            sampled: true,
        });
        let coarse = Arc::new(ExactVolumeData {
            values: vec![0.0; 8],
            specials: vec![0; 8],
            dimensions: [2, 2, 2],
            strides: [4, 2, 1],
            bounds: [[-0.5, 0.5]; 3],
            color_range: Some((0.0, 1.0)),
            sampled: true,
        });
        let mut request = VolumeRasterRequest {
            key: VolumeRasterKey {
                volume: VolumeKey {
                    file: 0,
                    path: "/volume".into(),
                    slice: VolumeSlice::new(&[2, 2, 2], false, PinnedDefault::Start),
                },
                layer_keys: Vec::new(),
                size: [2, 2],
                center: [1.0, 1.0],
                scale: [4.0; 3],
                rotation: VolumeRotation::identity().0,
                opacity: 1.0,
                back_brightness: 1.0,
                range: Some((0.0, 1.0)),
                logarithmic: false,
                visibility: ValueVisibility::default(),
                preset: ColormapPreset::Theme,
                color_stops: [[0.0; 4]; 3],
                special_colors: [[0.0; 4]; 4],
            },
            layers: vec![fine, coarse],
            colors: std::array::from_fn(|index| {
                if index == 255 {
                    [1.0, 0.0, 0.0, 1.0]
                } else {
                    [0.0, 0.0, 1.0, 1.0]
                }
            }),
        };
        assert_eq!(raycast_pixel(&request, 1.5, 0.5), [255, 0, 0, 255]);
        assert_eq!(raycast_pixel(&request, 0.5, 0.5), [0, 0, 255, 255]);
        Arc::get_mut(&mut request.layers[0]).unwrap().bounds[2] = [0.0, 0.5];
        request.key.opacity = 0.5;
        let mixed = raycast_pixel(&request, 1.5, 0.5);
        assert!(
            mixed[0] > 0 && mixed[2] > 0,
            "coarse voxels behind the fine crop must remain visible"
        );
    }

    #[test]
    fn prior_volume_image_follows_pan_and_zoom_while_refining() {
        let old = VolumeRasterKey {
            volume: VolumeKey {
                file: 0,
                path: "/volume".into(),
                slice: VolumeSlice::new(&[8, 8, 8], false, PinnedDefault::Start),
            },
            layer_keys: Vec::new(),
            size: [400, 300],
            center: [200.0, 150.0],
            scale: [100.0; 3],
            rotation: VolumeRotation::identity().0,
            opacity: 1.0,
            back_brightness: 1.0,
            range: None,
            logarithmic: false,
            visibility: ValueVisibility::default(),
            preset: ColormapPreset::Theme,
            color_stops: [[0.0; 4]; 3],
            special_colors: [[0.0; 4]; 4],
        };
        let mut new = old.clone();
        new.center = [230.0, 140.0];
        assert_eq!(
            volume_preview_rect(&old, &new),
            ([30.0, -10.0], [430.0, 290.0])
        );
        new.scale = [200.0; 3];
        assert_eq!(
            volume_preview_rect(&old, &new),
            ([-170.0, -160.0], [630.0, 440.0])
        );
        new.rotation = VolumeRotation::from_angles(0.3, 0.4).0;
        assert_eq!(
            volume_preview_rect(&old, &new),
            ([0.0, 0.0], [400.0, 300.0])
        );
    }

    #[test]
    fn ray_traverses_native_front_and_back_voxels() {
        let data = Arc::new(ExactVolumeData {
            values: vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0],
            specials: vec![0; 8],
            dimensions: [2, 2, 2],
            strides: [4, 2, 1],
            bounds: [[-0.5, 0.5]; 3],
            color_range: Some((0.0, 1.0)),
            sampled: false,
        });
        let mut request = VolumeRasterRequest {
            key: VolumeRasterKey {
                volume: VolumeKey {
                    file: 0,
                    path: String::new(),
                    slice: VolumeSlice::new(&[2, 2, 2], false, PinnedDefault::Start),
                },
                layer_keys: Vec::new(),
                size: [2, 2],
                center: [1.0, 1.0],
                scale: [4.0; 3],
                rotation: VolumeRotation::identity().0,
                opacity: 1.0,
                back_brightness: 1.0,
                range: data.color_range,
                logarithmic: false,
                visibility: ValueVisibility::default(),
                preset: ColormapPreset::Theme,
                color_stops: [[0.0; 4]; 3],
                special_colors: [[0.0; 4]; 4],
            },
            layers: vec![data],
            colors: std::array::from_fn(|index| {
                if index == 255 {
                    [1.0, 0.0, 0.0, 1.0]
                } else {
                    [0.0, 0.0, 1.0, 1.0]
                }
            }),
        };
        assert_eq!(raycast_pixel(&request, 0.5, 0.5), [255, 0, 0, 255]);
        request.key.visibility.positive = false;
        assert_eq!(raycast_pixel(&request, 0.5, 0.5), [0, 0, 255, 255]);
        Arc::get_mut(&mut request.layers[0]).unwrap().specials[4] = 4;
        request.key.special_colors[3] = [1.0, 1.0, 0.0, 1.0];
        request.key.visibility.positive = true;
        assert_eq!(raycast_pixel(&request, 0.5, 0.5), [255, 255, 0, 255]);

        // The front and rear special voxels contribute at their own depths.
        request.key.opacity = 0.5;
        assert_eq!(raycast_pixel(&request, 0.5, 0.5), [170, 170, 85, 191]);
        Arc::get_mut(&mut request.layers[0]).unwrap().specials[5] = 2;
        request.key.special_colors[1] = [1.0, 0.0, 1.0, 1.0];
        assert_eq!(raycast_pixel(&request, 0.5, 0.5), [255, 170, 85, 191]);
        Arc::get_mut(&mut request.layers[0]).unwrap().specials[4] = 0;
        assert_eq!(raycast_pixel(&request, 0.5, 0.5), [255, 0, 85, 191]);
        request.key.visibility.nan = false;
        assert_eq!(raycast_pixel(&request, 0.5, 0.5), [255, 0, 0, 128]);

        // Distant ordinary and special voxels dim by the same amount, with
        // alpha unchanged. The near voxel keeps its full color.
        request.key.opacity = 1.0;
        request.key.back_brightness = 0.7;
        request.colors.fill([1.0, 1.0, 1.0, 1.0]);
        Arc::get_mut(&mut request.layers[0]).unwrap().specials[5] = 0;
        assert_eq!(raycast_pixel(&request, 0.5, 0.5), [255, 255, 255, 255]);
        request.key.visibility.positive = false;
        assert_eq!(raycast_pixel(&request, 0.5, 0.5), [217, 217, 217, 255]);
        Arc::get_mut(&mut request.layers[0]).unwrap().specials[5] = 2;
        request.key.special_colors[1] = [1.0, 1.0, 1.0, 1.0];
        request.key.visibility.nan = true;
        assert_eq!(raycast_pixel(&request, 0.5, 0.5), [217, 217, 217, 255]);
    }

    #[test]
    #[ignore = "manual image check with the external GOME-2 product"]
    fn render_gome2_radiance_preview() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../mock_datasets/GOME2/01/15/20130115084607Z_32387_gome.nc"
        ));
        let file = File::open(path).unwrap();
        let shape = [1, 592, 32, 1024];
        let slice = VolumeSlice::new(&shape, true, PinnedDefault::Middle);
        let data = Arc::new(
            load_exact_volume(
                &file,
                "/MODE_NADIR/BAND_3/OBSERVATIONS/radiance",
                &shape,
                &slice,
            )
            .unwrap(),
        );
        let ordinary = data
            .specials
            .iter()
            .filter(|&&special| special == 0)
            .count();
        println!(
            "GOME-2 radiance: {ordinary} ordinary / {} total, range {:?}",
            data.values.len(),
            data.color_range
        );
        let special_counts = (0..=4)
            .map(|kind| data.specials.iter().filter(|&&value| value == kind).count())
            .collect::<Vec<_>>();
        println!("special counts [ordinary, fill, nan, +inf, -inf]: {special_counts:?}");
        let mut ordinary_values: Vec<f32> = data
            .values
            .iter()
            .zip(&data.specials)
            .filter_map(|(&value, &special)| (special == 0).then_some(value))
            .collect();
        for percentile in [0.01, 0.05, 0.1, 0.5, 0.9, 0.95, 0.99] {
            let index = (ordinary_values.len() as f32 * percentile) as usize;
            let (_, value, _) = ordinary_values.select_nth_unstable_by(index, f32::total_cmp);
            println!("percentile {percentile}: {value}");
        }
        let rotation = VolumeRotation::from_angles(0.65, 0.45);
        let base = 360.0 * 0.53;
        let scale = volume_axis_ratios(&slice, &shape).map(|ratio| base * ratio);
        let settings = Settings::load().unwrap();
        let context = dear_app::imgui::Context::create();
        let plot_ctx = PlotContext::create(&context);
        let palette = &settings.palette;
        let key = VolumeRasterKey {
            volume: VolumeKey {
                file: 0,
                path: String::new(),
                slice,
            },
            layer_keys: Vec::new(),
            size: [480, 360],
            center: [240.0, 180.0],
            scale,
            rotation: rotation.0,
            opacity: 1.0,
            back_brightness: settings.volume_back_brightness,
            range: data.color_range,
            logarithmic: false,
            visibility: ValueVisibility {
                fill: false,
                ..ValueVisibility::default()
            },
            preset: settings.heatmap_colormap_preset,
            color_stops: [
                palette.heatmap_low,
                palette.heatmap_mid,
                palette.heatmap_high,
            ],
            special_colors: [
                palette.fill,
                palette.nan,
                palette.positive_inf,
                palette.negative_inf,
            ],
        };
        let request = VolumeRasterRequest {
            key,
            layers: vec![data],
            colors: std::array::from_fn(|index| {
                continuous_color(
                    &plot_ctx,
                    index as f32 / 255.0,
                    settings.heatmap_colormap_preset,
                    palette,
                )
            }),
        };
        let rgba = render_exact_volume(&request);
        assert!(rgba.chunks_exact(4).any(|pixel| pixel[3] != 0));
        let distinct: std::collections::HashSet<[u8; 4]> = rgba
            .chunks_exact(4)
            .filter(|pixel| pixel[3] != 0)
            .map(|pixel| pixel.try_into().unwrap())
            .collect();
        println!("visible colors: {}", distinct.len());
        let mut frequencies = std::collections::HashMap::new();
        for pixel in rgba.chunks_exact(4).filter(|pixel| pixel[3] != 0) {
            *frequencies
                .entry(<[u8; 4]>::try_from(pixel).unwrap())
                .or_insert(0usize) += 1;
        }
        let mut common: Vec<_> = frequencies.into_iter().collect();
        common.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        println!("most common colors: {:?}", &common[..common.len().min(8)]);
        assert!(distinct.len() > 16);
        image::save_buffer(
            "/private/tmp/h5jive-gome2-exact-volume.png",
            &rgba,
            480,
            360,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}
