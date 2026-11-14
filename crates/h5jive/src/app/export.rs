use super::*;
use font8x8::{UnicodeFonts, BASIC_FONTS};
use image::{DynamicImage, GenericImage, ImageFormat, Rgba, RgbaImage};
use std::fs::OpenOptions as FileOptions;

pub(super) struct ExportDialog {
    path: String,
    message: Option<String>,
}

pub(super) struct PendingExport {
    path: PathBuf,
    format: ImageFormat,
    description: Vec<String>,
    view_rect: [f32; 4],
    display_size: [f32; 2],
    frames_remaining: u8,
}

impl App {
    pub(super) fn open_export_dialog(&mut self) {
        let stem = self
            .selection
            .as_ref()
            .and_then(|selection| selection.path.rsplit('/').find(|part| !part.is_empty()))
            .unwrap_or("view");
        let stem = stem
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect::<String>();
        let filename = format!("h5jive-{stem}-{}.png", self.view.key());
        self.export_dialog = Some(ExportDialog {
            path: filename,
            message: None,
        });
    }

    pub(super) fn draw_export_dialog(&mut self, ui: &Ui) {
        let Some(dialog) = self.export_dialog.as_mut() else {
            return;
        };
        let mut opened = true;
        let mut save = false;
        let mut close = false;
        ui.window("Save current view")
            .size([520.0, 145.0], Condition::FirstUseEver)
            .opened(&mut opened)
            .build(|| {
                ui.text("Save the current visualization as PNG or JPEG.");
                ui.set_next_item_width(-1.0);
                ui.input_text("##export_path", &mut dialog.path).build();
                if ui.button("Save") {
                    save = true;
                }
                ui.same_line();
                if ui.button("Close") {
                    close = true;
                }
                if let Some(message) = &dialog.message {
                    ui.text_wrapped(message);
                }
            });
        if !opened || close {
            self.export_dialog = None;
            return;
        }
        if save {
            let path = dialog.path.clone();
            match self.prepare_export(&path, ui.io().display_size()) {
                Ok(request) => {
                    self.pending_export = Some(request);
                    self.export_dialog = None;
                }
                Err(error) => self.export_dialog.as_mut().unwrap().message = Some(error),
            }
        }
    }

    fn prepare_export(&self, path: &str, display_size: [f32; 2]) -> Result<PendingExport, String> {
        let selection = self.selection.as_ref();
        let geo_options = match self.view {
            ViewMode::WorldMap => Some(self.map_controls.applied),
            ViewMode::Globe => Some(self.globe_controls.applied),
            _ => None,
        };
        if selection.is_none() && !geo_options.is_some_and(|options| options.show_pics) {
            return Err("Select a dataset before saving a view.".into());
        }
        let path = PathBuf::from(path.trim());
        let format = match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("png") => ImageFormat::Png,
            Some("jpg" | "jpeg") => ImageFormat::Jpeg,
            _ => return Err("Use a .png, .jpg, or .jpeg filename.".into()),
        };
        if path.exists() {
            return Err("That file already exists. Choose a new filename.".into());
        }
        let mut description = vec![format!("{} | H5Jive", self.view.title())];
        if let Some(selection) = selection {
            let file = &self.files[selection.file];
            let source = self
                .file_paths
                .get(selection.file)
                .map_or_else(|| file.name.clone(), |path| path.display().to_string());
            description.push(format!("File: {source}"));
            description.push(format!("Dataset: {}", selection.path));
            if let Some(entry) = find_entry(&file.tree, &selection.path) {
                if let Kind::Dataset { shape, .. } = &entry.kind {
                    description.push(format!("Shape: {shape:?}"));
                }
            }
        }
        if geo_options.is_some_and(|options| options.show_pics) {
            description.push("PICS site source: runtime DIPL catalog".into());
        }
        match self.view {
            ViewMode::Slice2d | ViewMode::Data => {
                let slice = if self.view == ViewMode::Slice2d {
                    self.applied_slice.as_ref()
                } else {
                    self.applied_table.as_ref()
                };
                if let Some(slice) = slice {
                    let rows = slice
                        .row_axis
                        .map(|axis| axis_name(&self.axis_names, axis))
                        .unwrap_or_else(|| "None".into());
                    description.push(format!(
                        "Rows: {rows}; columns: {}",
                        axis_name(&self.axis_names, slice.column_axis)
                    ));
                    let pins = slice
                        .pinned
                        .iter()
                        .enumerate()
                        .filter(|(axis, _)| {
                            Some(*axis) != slice.row_axis && *axis != slice.column_axis
                        })
                        .map(|(axis, index)| {
                            format!("{}={index}", axis_name(&self.axis_names, axis))
                        })
                        .collect::<Vec<_>>();
                    if !pins.is_empty() {
                        description.push(format!("Pinned: {}", pins.join(", ")));
                    }
                }
            }
            ViewMode::Profiles1d => {
                if let Some(slice) = &self.applied_profile {
                    description.push(format!(
                        "Profile axis: {}",
                        axis_name(&self.axis_names, slice.x_axis)
                    ));
                    if let Some(axis) = slice.y_axis {
                        description.push(format!(
                            "Compare across: {}; indexes: {}",
                            axis_name(&self.axis_names, axis),
                            slice.y_indexes
                        ));
                    }
                    let pins = slice
                        .pinned
                        .iter()
                        .enumerate()
                        .filter(|(axis, _)| *axis != slice.x_axis && Some(*axis) != slice.y_axis)
                        .map(|(axis, index)| {
                            format!("{}={index}", axis_name(&self.axis_names, axis))
                        })
                        .collect::<Vec<_>>();
                    if !pins.is_empty() {
                        description.push(format!("Pinned: {}", pins.join(", ")));
                    }
                }
            }
            ViewMode::WorldMap | ViewMode::Globe => {
                let options = geo_options.unwrap();
                description.push(format!(
                    "Time: {}; pinned axis 3: {}; data opacity: {:.0}%",
                    options
                        .time
                        .map_or_else(|| "all".into(), |time| time.to_string()),
                    options.pinned,
                    options.opacity * 100.0
                ));
            }
            ViewMode::Spatial3d => {
                if let Some(slice) = &self.applied_volume {
                    let axes = slice
                        .axes
                        .iter()
                        .flatten()
                        .map(|&axis| axis_name(&self.axis_names, axis))
                        .collect::<Vec<_>>();
                    description.push(format!("Axes: {}", axes.join(", ")));
                    if let Some(hidden) = slice.hidden {
                        description.push(format!(
                            "Pinned: {}={}",
                            axis_name(&self.axis_names, hidden),
                            slice.pinned
                        ));
                    }
                }
            }
        }
        Ok(PendingExport {
            path,
            format,
            description,
            view_rect: self.view_rect,
            display_size,
            frames_remaining: 1,
        })
    }

    pub(super) fn advance_export(&mut self) {
        let Some(request) = self.pending_export.as_mut() else {
            return;
        };
        if request.frames_remaining > 0 {
            request.frames_remaining -= 1;
            return;
        }
        let request = self.pending_export.take().unwrap();
        let message = match save_view_image(&request) {
            Ok(()) => format!("Saved {}", request.path.display()),
            Err(error) => format!("Could not save view: {error}"),
        };
        self.export_dialog = Some(ExportDialog {
            path: request.path.display().to_string(),
            message: Some(message),
        });
    }
}

fn save_view_image(request: &PendingExport) -> Result<(), String> {
    let window = xcap::Window::all()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|window| window.pid().ok() == Some(std::process::id()))
        .ok_or("Application window was not available for capture.")?;
    let screenshot = window.capture_image().map_err(|error| error.to_string())?;
    let output = compose_export_image(request, &screenshot)?;
    let mut file = FileOptions::new()
        .write(true)
        .create_new(true)
        .open(&request.path)
        .map_err(|error| error.to_string())?;
    let image = if request.format == ImageFormat::Jpeg {
        DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(output).to_rgb8())
    } else {
        DynamicImage::ImageRgba8(output)
    };
    if let Err(error) = image.write_to(&mut file, request.format) {
        drop(file);
        let _ = std::fs::remove_file(&request.path);
        return Err(error.to_string());
    }
    Ok(())
}

fn compose_export_image(
    request: &PendingExport,
    screenshot: &RgbaImage,
) -> Result<RgbaImage, String> {
    let logical_width = request.display_size[0].max(1.0);
    let logical_height = request.display_size[1].max(1.0);
    let scale = screenshot.width() as f32 / logical_width;
    let title_height = (screenshot.height() as f32 - logical_height * scale).max(0.0);
    let [x, y, width, height] = request.view_rect;
    let left = (x * scale).round().max(0.0) as u32;
    let top = (title_height + y * scale).round().max(0.0) as u32;
    let width = (width * scale).round().max(1.0) as u32;
    let height = (height * scale).round().max(1.0) as u32;
    if left >= screenshot.width() || top >= screenshot.height() {
        return Err("The current view is outside the captured window.".into());
    }
    let width = width.min(screenshot.width() - left);
    let height = height.min(screenshot.height() - top);
    let view = image::imageops::crop_imm(screenshot, left, top, width, height).to_image();
    let max_chars = ((width.saturating_sub(24)) / 16).max(1) as usize;
    let lines = request
        .description
        .iter()
        .flat_map(|line| wrap_text(line, max_chars))
        .collect::<Vec<_>>();
    let caption_height = 20 + lines.len() as u32 * 20;
    let mut output = RgbaImage::from_pixel(width, height + caption_height, Rgba([20, 25, 23, 255]));
    output
        .copy_from(&view, 0, 0)
        .map_err(|error| error.to_string())?;
    for (line_number, line) in lines.iter().enumerate() {
        draw_label(&mut output, 12, height + 10 + line_number as u32 * 20, line);
    }
    Ok(output)
}

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if current.chars().count() >= max_chars {
            lines.push(std::mem::take(&mut current));
        }
        current.push(ch);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn draw_label(image: &mut RgbaImage, x: u32, y: u32, text: &str) {
    for (index, ch) in text.chars().enumerate() {
        let Some(glyph) = BASIC_FONTS.get(ch).or_else(|| BASIC_FONTS.get('-')) else {
            continue;
        };
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..8 {
                if bits & (1 << column) != 0 {
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let px = x + index as u32 * 16 + column * 2 + dx;
                            let py = y + row as u32 * 2 + dy;
                            if px < image.width() && py < image.height() {
                                image.put_pixel(px, py, Rgba([235, 239, 233, 255]));
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_view_crops_window_and_adds_source_caption() {
        let screenshot = RgbaImage::from_pixel(400, 300, Rgba([180, 20, 30, 255]));
        let request = PendingExport {
            path: PathBuf::from("unused.png"),
            format: ImageFormat::Png,
            description: vec!["File: sample.nc".into(), "Dataset: /data".into()],
            view_rect: [100.0, 20.0, 300.0, 280.0],
            display_size: [400.0, 300.0],
            frames_remaining: 0,
        };
        let result = compose_export_image(&request, &screenshot).unwrap();
        assert_eq!(result.width(), 300);
        assert_eq!(result.height(), 340);
        assert_eq!(*result.get_pixel(0, 0), Rgba([180, 20, 30, 255]));
        assert!(result
            .enumerate_pixels()
            .any(|(_, y, pixel)| y >= 280 && *pixel != Rgba([20, 25, 23, 255])));
    }
}
