use super::{
    axis_ticks, draw_profile_plot, draw_workspace_mode_icon, extend_finite_range,
    format_attribute_value, format_value, heatmap_axis_ticks, heatmap_tile_bounds,
    heatmap_visible_bounds, heatmap_zoom_limits, human_bytes, load_heatmap_overview,
    load_heatmap_tile, load_map_data, load_profile, overview_sample_index, parse_input_spec,
    parse_profile_indexes, resolve_startup_selection, scan_heatmap_range, AttributeValue,
    ColormapPreset, File, GeoViewControls, MapViewport, PinnedDefault, ProfileBin, ProfileData,
    ProfileLine, ProfileSlice, ScanFilter, Selection, Settings, SpecialValue, TableSlice,
    ThemeColors, ValueVisibility, WorkspaceMode,
};
use h5jive_data::ScanDirectionClass;
use scinumtools3::Project;
use std::{path::Path, sync::OnceLock};

fn copy_settings_for_schema_test(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == ".cache" {
            continue;
        }
        let source = entry.path();
        let destination = to.join(entry.file_name());
        if source.is_dir() {
            copy_settings_for_schema_test(&source, &destination);
        } else {
            std::fs::copy(source, destination).unwrap();
        }
    }
}

fn test_settings() -> &'static Settings {
    static SETTINGS: OnceLock<Settings> = OnceLock::new();
    SETTINGS.get_or_init(|| Settings::load().unwrap())
}

#[test]
fn startup_dataset_suffix_selects_a_real_dataset() {
    let demo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/space_demo.hdf5");
    let (file_path, dataset) = parse_input_spec(&format!(
        "{}:/satellite/gome2/so2_slant_column",
        demo.display()
    ))
    .unwrap();
    let file = File::open(&file_path).unwrap();
    let selected = resolve_startup_selection(&[file], &[(file_path, dataset)])
        .unwrap()
        .unwrap();
    assert_eq!(selected.file, 0);
    assert_eq!(selected.path, "/satellite/gome2/so2_slant_column");
}

#[test]
fn startup_dataset_suffix_rejects_missing_and_group_paths() {
    let file_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/space_demo.hdf5");
    let file = File::open(&file_path).unwrap();
    let inputs = [(file_path.clone(), Some("/missing".to_owned()))];
    assert!(resolve_startup_selection(&[file], &inputs)
        .unwrap_err()
        .contains("not found"));
    let file = File::open(&file_path).unwrap();
    let inputs = [(file_path, Some("/satellite/gome2".to_owned()))];
    assert!(resolve_startup_selection(&[file], &inputs)
        .unwrap_err()
        .contains("not a dataset"));
}

#[test]
fn startup_dataset_suffix_keeps_windows_drive_and_plain_paths() {
    let (file, dataset) = parse_input_spec("C:\\data\\orbit.nc:/group/radiance").unwrap();
    assert_eq!(file.to_string_lossy(), "C:\\data\\orbit.nc");
    assert_eq!(dataset.as_deref(), Some("/group/radiance"));
    assert_eq!(parse_input_spec("C:/data/orbit.nc").unwrap().1, None);
    assert_eq!(
        parse_input_spec("C:/data/orbit.nc:/group/radiance")
            .unwrap()
            .1
            .as_deref(),
        Some("/group/radiance")
    );
    assert_eq!(parse_input_spec("/tmp/orbit.nc").unwrap().1, None);
    assert!(parse_input_spec("orbit.nc:/").is_err());
}

#[test]
fn dataset_formats_load_from_dipl() {
    let rules = &test_settings().dataset_rules;
    assert_eq!(rules.len(), 5);
    assert!(rules.iter().any(|rule| {
        rule.file_glob == "ER2_RPRO_GOM_L1B*.nc"
            && rule.platform == "ERS-2"
            && rule.observation_group == "OBSERVATIONS"
            && rule.coordinate_group == "GEODATA"
    }));
    assert!(rules
        .iter()
        .any(|rule| rule.platform == "MetOp M0?" && rule.product_type == "GO2_L1B___"));
    let gome2 = rules.iter().find(|rule| rule.sensor == "GOME-2").unwrap();
    let scan = gome2.scan_direction.as_ref().unwrap();
    assert_eq!(scan.variable, "scan_direction");
    assert_eq!(scan.forward_values, [1]);
    assert_eq!(scan.backward_values, [2]);
    assert_eq!(scan.default_filter, ScanFilter::Forward);
    let scia = rules
        .iter()
        .find(|rule| rule.sensor == "SCIAMACHY")
        .unwrap();
    let scan = scia.scan_direction.as_ref().unwrap();
    assert_eq!(scan.group, "observation");
    assert_eq!(scan.variable, "backscan_flag");
    assert_eq!(scan.forward_values, [0]);
    assert_eq!(scan.backward_values, [1]);
    assert_eq!(scan.default_filter, ScanFilter::Forward);
}

#[test]
fn dataset_format_schema_rejects_invalid_scan_options_and_depth() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../settings");
    let temporary = std::env::temp_dir().join(format!(
        "h5jive-schema-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    copy_settings_for_schema_test(&source, &temporary);
    assert!(Project::load(temporary.join("DIPfile")).is_ok());
    let format_file = temporary.join("dataset/formats/gome2_l1b.dip");
    let original = std::fs::read_to_string(&format_file).unwrap();
    for (valid, invalid) in [
        ("group = \"coordinate\"", "group = \"sideways\""),
        (
            "default_filter = \"forward\"",
            "default_filter = \"sideways\"",
        ),
        (
            "forward_values = \"1\"",
            "forward_values = \"not-a-number\"",
        ),
        ("forward_values = \"1\"", "forward_values = \"\""),
    ] {
        assert!(original.contains(valid));
        std::fs::write(&format_file, original.replacen(valid, invalid, 1)).unwrap();
        assert!(
            Project::load(temporary.join("DIPfile")).is_err(),
            "DIPL accepted {invalid}"
        );
    }
    let schema_file = temporary.join("dataset/formats/fdr_atmos_l1b_uvn.dip");
    let original_format = std::fs::read_to_string(&schema_file).unwrap();
    assert!(original_format.contains("overpass_group_depth = 1"));
    std::fs::write(&format_file, original).unwrap();
    std::fs::write(
        &schema_file,
        original_format.replacen("overpass_group_depth = 1", "overpass_group_depth = 9", 1),
    )
    .unwrap();
    assert!(
        Project::load(temporary.join("DIPfile")).is_err(),
        "DIPL accepted overpass_group_depth = 9"
    );
    std::fs::remove_dir_all(temporary).unwrap();
}

#[test]
fn mock_product_formats_discover_geolocation_when_available() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mock_datasets");
    if !root.exists() {
        return;
    }
    let mut pending = vec![root.clone()];
    let mut checked = 0;
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let file_path = entry.unwrap().path();
            if file_path.is_dir() {
                pending.push(file_path);
                continue;
            }
            if file_path
                .extension()
                .is_none_or(|extension| extension != "nc")
            {
                continue;
            }
            let relative = file_path.strip_prefix(&root).unwrap();
            let family = relative
                .components()
                .next()
                .unwrap()
                .as_os_str()
                .to_str()
                .unwrap();
            let path = match family {
                "GOME" => "/MODE_NADIR/BAND_3/OBSERVATIONS/radiance",
                "SCIAMACHY" => "/MODE_NADIR/BAND_09/OBSERVATIONS/radiance",
                "GOME2" => "/MODE_NADIR/BAND_1A/OBSERVATIONS/radiance",
                "FDR" => "/GOME/NIR/OBSERVATIONS/radiance_fdr",
                _ => continue,
            };
            let file = File::open(&file_path).unwrap();
            let geolocation = file
                .geolocation_with_rules(path, &test_settings().dataset_rules)
                .unwrap()
                .unwrap_or_else(|| panic!("no geolocation for {}: {path}", file_path.display()));
            assert!(geolocation.latitude.ends_with("/GEODATA/latitude"));
            assert!(geolocation.longitude.ends_with("/GEODATA/longitude"));
            assert!(geolocation.latitude_bounds.is_some());
            assert!(geolocation.longitude_bounds.is_some());
            if matches!(family, "GOME2" | "SCIAMACHY") {
                let scan = geolocation.scan_direction.as_ref().unwrap();
                let expected_group = if family == "GOME2" {
                    "/GEODATA/"
                } else {
                    "/OBSERVATIONS/"
                };
                assert!(scan.path.contains(expected_group));
                assert_eq!(scan.forward_values.len(), 1);
                assert_eq!(scan.backward_values.len(), 1);
            }
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn mock_scan_flags_reach_map_pixels_when_available() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mock_datasets");
    let products = [
        (
            root.join("GOME2/01/12/20130112080756Z_32344_gome.nc"),
            "/MODE_NADIR/BAND_1A/OBSERVATIONS/radiance",
        ),
        (
            root.join("SCIAMACHY/EN1_RPRO_SCI_____1C_20090606T064119_20090606T081734_037993_01_041000_20230808T124622.nc"),
            "/MODE_NADIR/BAND_09/OBSERVATIONS/radiance",
        ),
    ];
    for (product, path) in products {
        if !product.exists() {
            continue;
        }
        let file = File::open(&product).unwrap();
        let geo = file
            .geolocation_with_rules(path, &test_settings().dataset_rules)
            .unwrap()
            .unwrap();
        let shape = file.dataset_info(path).unwrap().shape;
        let map = load_map_data(&file, path, &shape, &geo, Some(0), 0, 30_000).unwrap();
        let forward = map
            .points
            .iter()
            .filter(|point| point.scan_direction == Some(ScanDirectionClass::Forward))
            .count();
        let backward = map
            .points
            .iter()
            .filter(|point| point.scan_direction == Some(ScanDirectionClass::Backward))
            .count();
        assert!(
            forward > 0,
            "{} has no forward map pixels",
            product.display()
        );
        assert!(
            backward > 0,
            "{} has no backward map pixels",
            product.display()
        );
    }
}

#[test]
fn geospatial_views_start_at_the_middle_of_the_fourth_axis() {
    let selection = Selection {
        file: 0,
        path: "/radiance".into(),
    };
    let shape = [1, 219, 3, 1024];
    let mut map = GeoViewControls::new(true);
    let mut globe = GeoViewControls::new(true);
    assert_eq!(test_settings().map_pinned_default, PinnedDefault::Middle);
    assert_eq!(test_settings().globe_pinned_default, PinnedDefault::Middle);
    map.sync_source(
        Some((&selection, &shape)),
        test_settings().map_pinned_default,
        ScanFilter::Forward,
    );
    globe.sync_source(
        Some((&selection, &shape)),
        test_settings().globe_pinned_default,
        ScanFilter::Forward,
    );
    assert_eq!(map.applied.pinned, 512);
    assert_eq!(globe.applied.pinned, 512);
    assert_eq!(map.applied.scan_filter, ScanFilter::Forward);
    assert_eq!(globe.applied.scan_filter, ScanFilter::Forward);
    map.draft.pinned = 300;
    assert_eq!(map.applied.pinned, 512);
}

#[test]
fn pics_catalog_loads_reference_and_additional_regions() {
    let sites = &test_settings().pics_sites;
    assert_eq!(sites.len(), 22);
    assert_eq!(sites.iter().filter(|site| site.reference).count(), 6);
    assert!(sites
        .iter()
        .any(|site| site.name == "Libya 4" && site.center == [23.39, 28.55]));
    for site in sites {
        assert!((1..=2).contains(&site.regions.len()));
        for region in &site.regions {
            assert!(region.bounds[0] < region.bounds[2]);
            assert!(region.bounds[1] < region.bounds[3]);
        }
        if site.reference {
            let ceos = site.regions[0].bounds;
            let cnes = site.regions[1].bounds;
            assert!(ceos[0] < site.center[0] && site.center[0] < ceos[2]);
            assert!(ceos[1] < site.center[1] && site.center[1] < ceos[3]);
            assert!(cnes[0] >= ceos[0] && cnes[2] <= ceos[2]);
            assert!(cnes[1] >= ceos[1] && cnes[3] <= ceos[3]);
        }
        assert!(site.source_url.starts_with("https://"));
    }
}

#[test]
fn map_zoom_preserves_pointer_location_and_pan_moves_view() {
    let origin = [10.0, 20.0];
    let size = [720.0, 360.0];
    let pointer = [400.0, 150.0];
    let mut view = MapViewport::default();
    let geo = view.unproject(origin, size, pointer);
    view.zoom_at(origin, size, pointer, 4.0, test_settings().map_max_zoom);
    let projected = view.project(origin, size, geo[0], geo[1]);
    assert!((projected[0] - pointer[0]).abs() < 0.001);
    assert!((projected[1] - pointer[1]).abs() < 0.001);
    view.pan(size, [40.0, -20.0]);
    let moved = view.project(origin, size, geo[0], geo[1]);
    assert!((moved[0] - pointer[0] - 40.0).abs() < 0.001);
    assert!((moved[1] - pointer[1] + 20.0).abs() < 0.001);
    let anchor = view.unproject(origin, size, pointer);
    view.zoom_at(
        origin,
        size,
        pointer,
        10_000.0,
        test_settings().map_max_zoom,
    );
    assert_eq!(view.zoom, test_settings().map_max_zoom);
    assert!(view.zoom > 128.0);
    let projected = view.project(origin, size, anchor[0], anchor[1]);
    assert!((projected[0] - pointer[0]).abs() < 0.01);
    assert!((projected[1] - pointer[1]).abs() < 0.01);
}

#[test]
fn theme_colors_pop_in_reverse_scope_order() {
    use dear_app::imgui::{Context, StyleColor};
    let mut context = Context::create();
    context.set_ini_filename(None::<String>).unwrap();
    context.io_mut().set_display_size([640.0, 480.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let ui = context.frame();
    let colors = ThemeColors(vec![
        ui.push_style_color(StyleColor::Text, [1.0, 1.0, 1.0, 1.0]),
        ui.push_style_color(StyleColor::WindowBg, [0.0, 0.0, 0.0, 1.0]),
    ]);
    drop(colors);
    let data = ProfileData {
        x_len: 2,
        min: -1.0,
        max: 1.0,
        lines: vec![ProfileLine {
            index: 0,
            bins: vec![
                ProfileBin {
                    first: -1.0,
                    last: -1.0,
                    min: -1.0,
                    max: -1.0,
                    count: 1,
                },
                ProfileBin {
                    first: 1.0,
                    last: 1.0,
                    min: 1.0,
                    max: 1.0,
                    count: 1,
                },
            ],
        }],
    };
    let slice = ProfileSlice::new(&[2], PinnedDefault::Start);
    let mut visibility = ValueVisibility::default();
    ui.window("Profile smoke")
        .size([500.0, 350.0], dear_app::imgui::Condition::Always)
        .build(|| {
            draw_profile_plot(
                &ui,
                &data,
                &slice,
                &["Index".into()],
                &test_settings().palette,
                &std::collections::HashMap::from([(0, "0 (12.5)".into()), (1, "1 (13.5)".into())]),
                &["Value".into()],
                test_settings().display_decimals,
                1.0,
                &mut visibility,
            )
        });
}

#[test]
fn workspace_icon_reuses_the_active_draw_list() {
    use dear_app::imgui::{Condition, Context};
    let mut context = Context::create();
    context.set_ini_filename(None::<String>).unwrap();
    context.io_mut().set_display_size([640.0, 480.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let ui = context.frame();
    ui.window("Workspace icon smoke")
        .size([300.0, 80.0], Condition::Always)
        .build(|| {
            let draw = ui.get_window_draw_list();
            draw.add_rect([10.0, 10.0], [40.0, 40.0], [1.0; 4])
                .filled(true)
                .build();
            draw_workspace_mode_icon(&draw, WorkspaceMode::DatasetBrowser, [15.0, 15.0], [1.0; 4]);
        });
}

#[test]
fn heatmap_navigation_zooms_around_the_center_and_stays_in_bounds() {
    let full = [0.0, 100.0, 0.0, 50.0];
    let zoomed = heatmap_zoom_limits(full, full, 2.0);
    assert_eq!(zoomed, [25.0, 75.0, 12.5, 37.5]);
    assert_eq!(heatmap_zoom_limits(zoomed, full, 0.5), full);
    let edge = heatmap_zoom_limits([90.0, 100.0, 40.0, 50.0], full, 2.0);
    assert_eq!(edge, [92.5, 97.5, 42.5, 47.5]);
}

#[test]
fn table_region_tracks_axis_order_and_pins() {
    let shape = [8, 32, 48, 16];
    let slice = TableSlice {
        row_axis: Some(2),
        column_axis: 0,
        pinned: vec![0, 7, 0, 3],
    };
    let (start, count) = slice.region(&shape, 5, 8, 2, 4);
    assert_eq!(start, [2, 7, 5, 3]);
    assert_eq!(count, [2, 1, 3, 1]);
    assert_eq!(slice.region_offset(&count, 5, 2, &start), 0);
    assert_eq!(slice.region_offset(&count, 7, 3, &start), 5);
    let one_dimensional = TableSlice::new(&[8], PinnedDefault::Start);
    assert_eq!(one_dimensional.rows(&[8]), 8);
    assert_eq!(one_dimensional.columns(&[8]), 1);
    assert_eq!(one_dimensional.region(&[8], 2, 5, 0, 1), (vec![2], vec![3]));
}

#[test]
fn table_and_heatmap_default_to_axes_after_leading_time() {
    let shape = [8, 32, 48];
    let table = TableSlice::with_leading_time(&shape, true, PinnedDefault::Start);
    assert_eq!((table.row_axis, table.column_axis), (Some(1), 2));
    assert_eq!(table.pinned[0], 0);
    assert_eq!(
        table.region(&shape, 4, 6, 7, 10),
        (vec![0, 4, 7], vec![1, 2, 3])
    );
    let later_time = TableSlice::with_leading_time(&shape, true, PinnedDefault::Middle);
    assert_eq!(later_time.pinned[0], 4);
    assert_eq!(later_time.region(&shape, 4, 6, 7, 10).0, [4, 4, 7]);
    let ordinary = TableSlice::with_leading_time(&shape, false, PinnedDefault::Start);
    assert_eq!((ordinary.row_axis, ordinary.column_axis), (Some(0), 1));
    let two_dimensional = TableSlice::with_leading_time(&[8, 32], true, PinnedDefault::Start);
    assert_eq!(
        (two_dimensional.row_axis, two_dimensional.column_axis),
        (None, 1)
    );
    assert_eq!(
        two_dimensional.region(&[8, 32], 4, 6, 0, 1),
        ([0, 4].to_vec(), [1, 2].to_vec())
    );
}

#[test]
fn display_precision_uses_fixed_notation_only_in_the_middle_range() {
    assert_eq!(test_settings().display_decimals, 4);
    assert_eq!(format_value(0.0, 4), "0");
    assert_eq!(format_value(-0.0, 4), "0");
    assert_eq!(format_value(0.001, 4), "0.001");
    assert_eq!(format_value(1000.0, 4), "1000");
    assert_eq!(format_value(12.34567, 4), "12.3457");
    assert_eq!(format_value(1.23004, 4), "1.23");
    assert_eq!(format_value(0.000123456, 4), "1.2346e-4");
    assert_eq!(format_value(1000.1, 4), "1.0001e3");
    assert_eq!(format_value(-0.000001, 4), "-1e-6");
    assert_eq!(format_value(f64::INFINITY, 4), "+Inf");
    assert_eq!(format_value(12.34567, 2), "12.35");
    let attribute = AttributeValue {
        name: "_FillValue".into(),
        value: "9969209968386869000000000000000000000".into(),
        float_values: Some(vec![9.969209968386869e36]),
    };
    assert_eq!(format_attribute_value(&attribute, 4), "9.9692e36");
    let vector = AttributeValue {
        name: "scale".into(),
        value: "raw".into(),
        float_values: Some(vec![0.001, 1000.1]),
    };
    assert_eq!(format_attribute_value(&vector, 4), "[0.001, 1.0001e3]");
    assert_eq!(human_bytes(1536, 4), "1.5 KiB");
}

#[test]
fn pinned_defaults_choose_start_middle_or_end() {
    let shape = [8, 5, 1, 0];
    assert_eq!(
        TableSlice::new(&shape, PinnedDefault::Start).pinned,
        [0, 0, 0, 0]
    );
    assert_eq!(
        TableSlice::new(&shape, PinnedDefault::Middle).pinned,
        [4, 2, 0, 0]
    );
    assert_eq!(
        TableSlice::new(&shape, PinnedDefault::End).pinned,
        [7, 4, 0, 0]
    );
}

#[test]
fn profile_indexes_and_pinned_dimensions_select_distinct_lines() {
    assert_eq!(parse_profile_indexes("7, 2, 7", 8).unwrap(), [2, 7]);
    assert!(parse_profile_indexes("8", 8).is_err());
    assert!(parse_profile_indexes("2,", 8).is_err());

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/space_demo.hdf5");
    let file = File::open(&path).unwrap();
    let shape = [8, 32, 48, 16];
    let slice = ProfileSlice {
        x_axis: 2,
        y_axis: Some(3),
        pinned: vec![2, 7, 0, 0],
        y_indexes: "1, 4".into(),
    };
    let data = load_profile(
        &file,
        "/observations/time_spectral_cube_4d",
        &shape,
        &slice,
        &ValueVisibility::default(),
    )
    .unwrap();
    assert_eq!(data.x_len, 48);
    assert_eq!(
        data.lines.iter().map(|line| line.index).collect::<Vec<_>>(),
        [1, 4]
    );
    for line in &data.lines {
        let expected = file
            .region(
                "/observations/time_spectral_cube_4d",
                &[2, 7, 0, line.index],
                &[1, 1, 48, 1],
            )
            .unwrap();
        let valid = expected
            .data
            .iter()
            .filter(|&&value| value.is_finite() && expected.fill_value != Some(value))
            .count();
        assert_eq!(line.bins.iter().map(|bin| bin.count).sum::<usize>(), valid);
    }
}

#[test]
fn dipl_theme_collection_provides_distinct_profiles() {
    let settings = test_settings();
    assert_eq!(settings.volume_default_opacity, 1.0);
    assert_eq!(settings.volume_max_zoom, 32.0);
    assert_eq!(settings.volume_back_brightness, 0.45);
    assert_eq!(settings.heatmap_colormap_preset, ColormapPreset::Viridis);
    assert_eq!(settings.graphics_backend, None);
    assert_eq!(settings.forwarded_x11_backend, dear_app::wgpu::Backends::GL);
    let profile = |name| {
        settings
            .profiles
            .iter()
            .find(|profile| profile.name == name)
            .unwrap()
    };
    for name in ["dark", "light", "high_contrast", "soft_slate", "h5jive"] {
        profile(name);
    }
    let dark = &profile("dark").palette;
    let light = &profile("light").palette;
    assert!(dark.background[0] < light.background[0]);
    assert!(dark.text[0] > dark.background[0]);
    assert!(light.text[0] < light.background[0]);
    assert_eq!(dark.heatmap_low, light.heatmap_low);
    let h5jive = &profile("h5jive").palette;
    assert!(h5jive.heatmap_low[2] > h5jive.heatmap_low[0]);
    assert!(h5jive.heatmap_high[0] > h5jive.heatmap_high[2]);
    assert!(profile("soft_slate").rounding > profile("dark").rounding);
    let mut selected = dark.clone();
    ColormapPreset::Grayscale.apply(&mut selected);
    assert_ne!(selected.heatmap_low, dark.heatmap_low);
    assert_eq!(selected.nan, dark.nan);
    assert_eq!(selected.fill, dark.fill);
    assert_eq!(selected.positive_inf, dark.positive_inf);
    assert_eq!(selected.negative_inf, dark.negative_inf);
}

#[test]
fn heatmap_ticks_follow_the_visible_cells() {
    let visible = heatmap_visible_bounds(100, 200, [40.0, 45.0, 30.0, 34.0]);
    assert_eq!(
        heatmap_axis_ticks(visible[2], visible[3], 6),
        [40, 41, 42, 43, 44]
    );
    assert_eq!(
        heatmap_axis_ticks(visible[0], visible[1], 6),
        [66, 67, 68, 69]
    );
    assert!(heatmap_axis_ticks(0, 0, 6).is_empty());
}

#[test]
fn heatmap_preserves_native_cells_and_special_values() {
    let palette = &test_settings().palette;
    for (value, fill, special) in [
        (-9999.0, Some(-9999.0), SpecialValue::Fill),
        (f64::NAN, None, SpecialValue::Nan),
        (f64::INFINITY, None, SpecialValue::PositiveInf),
        (f64::NEG_INFINITY, None, SpecialValue::NegativeInf),
    ] {
        assert_eq!(SpecialValue::from_value(value, fill), Some(special));
        assert_ne!(special.color(palette), palette.heatmap_mid);
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/space_demo.hdf5");
    let file = File::open(&path).unwrap();
    let shape = [32, 48];
    let slice = TableSlice::new(&shape, PinnedDefault::Start);
    let image = load_heatmap_tile(
        &file,
        "/observations/swath_2d",
        &shape,
        &slice,
        [4, 8, 7, 12],
        -1.0,
    )
    .unwrap();
    let region = file
        .region("/observations/swath_2d", &[4, 7], &[4, 5])
        .unwrap();
    assert_eq!(image.values.len(), 20);
    for (got, expected) in image.values.iter().zip(region.data.iter()) {
        assert!(got == expected || got.is_nan() && expected.is_nan());
    }
    assert!(image
        .values
        .iter()
        .any(|&value| { SpecialValue::from_value(value, image.fill_value).is_some() }));
    let mut four_dimensional = TableSlice::new(&[8, 32, 48, 16], PinnedDefault::Start);
    four_dimensional.row_axis = Some(1);
    four_dimensional.column_axis = 2;
    four_dimensional.pinned = vec![2, 0, 0, 3];
    let cube = load_heatmap_tile(
        &file,
        "/observations/time_spectral_cube_4d",
        &[8, 32, 48, 16],
        &four_dimensional,
        [0, 2, 0, 3],
        -1.0,
    )
    .unwrap();
    let expected = file
        .region(
            "/observations/time_spectral_cube_4d",
            &[2, 0, 0, 3],
            &[1, 1, 1, 1],
        )
        .unwrap();
    assert_eq!(cube.values[0], expected.data[0]);
    assert_eq!(cube.values.len(), 6);
    assert_eq!(
        heatmap_visible_bounds(32, 48, [7.0, 12.0, 24.0, 28.0]),
        [4, 8, 7, 12]
    );
    let tile = heatmap_tile_bounds(32, 48, [4, 8, 7, 12], 65536);
    assert!(tile[0] <= 4 && tile[1] >= 8 && tile[2] <= 7 && tile[3] >= 12);
    assert_eq!(axis_ticks(32, 5), [0, 7, 15, 23, 31]);
}

#[test]
fn special_colors_are_separate_from_each_other_and_viridis_stops() {
    let distance = |a: [f32; 4], b: [f32; 4]| {
        a[..3]
            .iter()
            .zip(&b[..3])
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    };
    for profile in &test_settings().profiles {
        let mut palette = profile.palette.clone();
        ColormapPreset::Viridis.apply(&mut palette);
        assert!(
            palette.negative_inf[0] < palette.negative_inf[1].max(palette.negative_inf[2]),
            "{}: -Inf is red",
            profile.name
        );
        let fill_channels = &palette.fill[..3];
        assert!(
            fill_channels
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max)
                - fill_channels.iter().copied().fold(f32::INFINITY, f32::min)
                > 0.25,
            "{}: fill is grayish",
            profile.name
        );
        let special = [
            palette.nan,
            palette.positive_inf,
            palette.negative_inf,
            palette.fill,
        ];
        for (index, color) in special.iter().enumerate() {
            for other in &special[index + 1..] {
                assert!(
                    distance(*color, *other) > 0.3,
                    "{}: special colors overlap",
                    profile.name
                );
            }
            for stop in [
                palette.heatmap_low,
                palette.heatmap_mid,
                palette.heatmap_high,
            ] {
                assert!(
                    distance(*color, stop) > 0.3,
                    "{}: special color overlaps Viridis",
                    profile.name
                );
            }
        }
    }
}

#[test]
fn full_heatmap_overview_samples_source_cells_without_averaging() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/space_demo.hdf5");
    let file = File::open(&path).unwrap();
    let shape = [32, 48];
    let slice = TableSlice::new(&shape, PinnedDefault::Start);
    let image =
        load_heatmap_overview(&file, "/observations/swath_2d", &shape, &slice, [8, 12]).unwrap();
    assert_eq!(image.bounds, [0, 32, 0, 48]);
    assert_eq!(image.grid, [8, 12]);
    let source = file
        .region("/observations/swath_2d", &[0, 0], &shape)
        .unwrap();
    for row_bin in 0..8 {
        for column_bin in 0..12 {
            let row = overview_sample_index(row_bin, 32, 8);
            let column = overview_sample_index(column_bin, 48, 12);
            let actual = image.values[row_bin * 12 + column_bin];
            let expected = source.data[row * 48 + column];
            assert!(actual == expected || actual.is_nan() && expected.is_nan());
        }
    }
}

#[test]
fn heatmap_range_scans_selected_plane_and_ignores_special_values() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/space_demo.hdf5");
    let file = File::open(&path).unwrap();
    let shape = [32, 48];
    let slice = TableSlice::new(&shape, PinnedDefault::Start);
    let region = file
        .region("/observations/swath_2d", &[0, 0], &shape)
        .unwrap();
    let mut expected = None;
    extend_finite_range(&mut expected, &region.data, region.fill_value);
    let actual = scan_heatmap_range(&file, "/observations/swath_2d", &shape, &slice).unwrap();
    assert_eq!(actual, expected);
    assert!(actual.is_some_and(|(min, max)| min < max));

    let shape = [8, 32, 48, 16];
    let mut slice = TableSlice::new(&shape, PinnedDefault::Start);
    slice.row_axis = Some(1);
    slice.column_axis = 2;
    slice.pinned = vec![2, 0, 0, 3];
    let region = file
        .region(
            "/observations/time_spectral_cube_4d",
            &[2, 0, 0, 3],
            &[1, 32, 48, 1],
        )
        .unwrap();
    let mut expected = None;
    extend_finite_range(&mut expected, &region.data, region.fill_value);
    assert_eq!(
        scan_heatmap_range(&file, "/observations/time_spectral_cube_4d", &shape, &slice,).unwrap(),
        expected
    );
}
