use super::*;

pub(super) fn draw_pics_overlay(
    ui: &Ui,
    origin: [f32; 2],
    size: [f32; 2],
    clip_origin: [f32; 2],
    clip_size: [f32; 2],
    view: MapViewport,
    sites: &[PicsSite],
    selected_group: Option<&str>,
    selected: usize,
    palette: &Palette,
) -> bool {
    let mouse = ui.io().mouse_pos();
    let inside = mouse[0] >= clip_origin[0]
        && mouse[0] <= clip_origin[0] + clip_size[0]
        && mouse[1] >= clip_origin[1]
        && mouse[1] <= clip_origin[1] + clip_size[1];
    let mut nearest: Option<(usize, f32)> = None;
    let draw = ui.get_window_draw_list();
    let clip = draw.push_clip_rect(
        clip_origin,
        [clip_origin[0] + clip_size[0], clip_origin[1] + clip_size[1]],
        true,
    );
    for (index, site) in sites.iter().enumerate() {
        if selected_group.is_some_and(|group| site.category != group) {
            continue;
        }
        for (region_index, region) in site.regions.iter().enumerate() {
            let [west, south, east, north] = region.bounds;
            let color = if region_index == 0 {
                palette.accent
            } else {
                palette.fill
            };
            draw.add_rect(
                view.project(origin, size, west, north),
                view.project(origin, size, east, south),
                color,
            )
            .thickness(1.5)
            .build();
        }
        let center = view.project(origin, size, site.center[0], site.center[1]);
        let emphasized = selected == index + 1;
        draw.add_circle(
            center,
            if emphasized { 7.0 } else { 5.0 },
            palette.background,
        )
        .filled(true)
        .build();
        draw.add_circle(center, if emphasized { 5.0 } else { 3.5 }, palette.accent)
            .filled(true)
            .build();
        if inside {
            let distance = (center[0] - mouse[0]).powi(2) + (center[1] - mouse[1]).powi(2);
            if nearest.is_none_or(|(_, previous)| distance < previous) {
                nearest = Some((index, distance));
            }
        }
    }
    drop(clip);
    drop(draw);
    if let Some((index, _)) = nearest.filter(|(_, distance)| *distance <= 100.0) {
        let site = &sites[index];
        let mut description = format!(
            "{} ({})\n{}\nCenter: {:.2}°N, {:.2}°E",
            site.name,
            site.country,
            if site.reference {
                "CEOS reference PICS"
            } else {
                &site.category
            },
            site.center[1],
            site.center[0]
        );
        for region in &site.regions {
            let [west, south, east, north] = region.bounds;
            description.push_str(&format!(
                "\n{} ROI: {:.2} to {:.2}°N, {:.2} to {:.2}°E",
                region.name, south, north, west, east
            ));
        }
        if let Some([width, height]) = site.usable_km {
            description.push_str(&format!("\nUsable area: {width} × {height} km"));
        }
        if let Some(altitude) = site.altitude_m {
            description.push_str(&format!("\nAltitude: {altitude} m"));
        }
        if let Some(wrs) = &site.landsat_wrs_2 {
            description.push_str(&format!("\nLandsat WRS-2: {wrs}"));
        }
        description.push_str(&format!("\nStatus: {}", site.status));
        if !site.notes.is_empty() {
            description.push_str(&format!("\n{}", site.notes));
        }
        description.push_str(&format!("\nSource: {}", site.source_url));
        ui.tooltip_text(description);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_app::imgui::{Condition, Context};

    #[test]
    fn pics_overlay_and_shared_spinner_draw_in_one_frame() {
        let settings = Settings::load().unwrap();
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([800.0, 500.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let ui = context.frame();
        ui.window("Map smoke")
            .size([700.0, 400.0], Condition::Always)
            .build(|| {
                draw_pics_overlay(
                    &ui,
                    [20.0, 30.0],
                    [600.0, 300.0],
                    [20.0, 30.0],
                    [600.0, 300.0],
                    MapViewport::default(),
                    &settings.pics_sites,
                    None,
                    4,
                    &settings.palette,
                );
            });
        draw_loading_indicator(&ui, [790.0, 15.0], "Loading map + stats", &settings.palette);
    }
}
