use super::*;
use dear_app::{ExternalTextureHandle, GpuApi, GpuGeneration};

const NATURAL_EARTH_TEXTURE_BMP: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/earth_natural_earth_2_relief_water_4096.bmp"
));
const BLUE_MARBLE_TEXTURE_BMP: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/earth_blue_marble_topo_bathy_4096.bmp"
));

struct RegisteredEarthTexture {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    handle: ExternalTextureHandle,
    id: dear_app::imgui::TextureId,
}

pub(super) struct EarthTexture {
    natural_earth: Option<RegisteredEarthTexture>,
    blue_marble: Option<RegisteredEarthTexture>,
    generation: GpuGeneration,
}

impl App {
    pub(super) fn ensure_earth_texture(&mut self, gpu: &mut GpuApi<'_>) {
        if self
            .earth_texture
            .as_ref()
            .is_some_and(|texture| texture.generation == gpu.generation())
        {
            return;
        }
        self.earth_texture = None;
        self.earth_texture = Some(EarthTexture {
            natural_earth: upload_earth_texture(
                gpu,
                NATURAL_EARTH_TEXTURE_BMP,
                "Natural Earth II shaded relief and water",
            ),
            blue_marble: upload_earth_texture(
                gpu,
                BLUE_MARBLE_TEXTURE_BMP,
                "NASA Blue Marble topography and bathymetry",
            ),
            generation: gpu.generation(),
        });
    }

    pub(super) fn earth_texture_id(&self, background: &str) -> Option<dear_app::imgui::TextureId> {
        let earth = self.earth_texture.as_ref()?;
        let texture = match background {
            "natural_earth" => earth.natural_earth.as_ref(),
            "blue_marble" => earth.blue_marble.as_ref(),
            _ => None,
        }?;
        let _registration_keeps_view_alive = texture.handle;
        Some(texture.id)
    }
}

fn upload_earth_texture(
    gpu: &mut GpuApi<'_>,
    bitmap: &[u8],
    label: &'static str,
) -> Option<RegisteredEarthTexture> {
    let (width, height, rgba) = decode_bmp24(bitmap).ok()?;
    let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue().write_texture(
        texture.as_image_copy(),
        &rgba,
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
    Some(RegisteredEarthTexture {
        _texture: texture,
        _view: view,
        handle,
        id,
    })
}

fn decode_bmp24(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), ()> {
    if bytes.get(0..2) != Some(b"BM") || bytes.len() < 54 {
        return Err(());
    }
    let read_u32 = |offset: usize| -> Result<u32, ()> {
        Ok(u32::from_le_bytes(
            bytes
                .get(offset..offset + 4)
                .ok_or(())?
                .try_into()
                .map_err(|_| ())?,
        ))
    };
    let data_offset = read_u32(10)? as usize;
    let dib_size = read_u32(14)?;
    let width = i32::from_le_bytes(bytes.get(18..22).ok_or(())?.try_into().map_err(|_| ())?);
    let signed_height =
        i32::from_le_bytes(bytes.get(22..26).ok_or(())?.try_into().map_err(|_| ())?);
    let bit_depth = u16::from_le_bytes(bytes.get(28..30).ok_or(())?.try_into().map_err(|_| ())?);
    let compression = read_u32(30)?;
    if dib_size < 40 || width <= 0 || signed_height == 0 || bit_depth != 24 || compression != 0 {
        return Err(());
    }
    let (width, height) = (width as u32, signed_height.unsigned_abs());
    let row_bytes = width.checked_mul(3).ok_or(())?;
    let row_stride = (row_bytes + 3) & !3;
    let total = (row_stride as usize)
        .checked_mul(height as usize)
        .ok_or(())?;
    if data_offset
        .checked_add(total)
        .is_none_or(|end| end > bytes.len())
    {
        return Err(());
    }
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        let source_y = if signed_height < 0 { y } else { height - 1 - y };
        let row = data_offset + source_y as usize * row_stride as usize;
        for x in 0..width as usize {
            let i = row + x * 3;
            rgba.extend_from_slice(&[bytes[i + 2], bytes[i + 1], bytes[i], 255]);
        }
    }
    Ok((width, height, rgba))
}
