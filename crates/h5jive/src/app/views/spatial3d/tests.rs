use super::*;

#[test]
fn zoomed_volume_requests_only_visible_indices_and_pan_moves_the_request() {
    let shape = [512, 512, 512];
    let slice = VolumeSlice::new(&shape, false, PinnedDefault::Start);
    let rotation = VolumeRotation::identity();
    let overview = visible_volume_slice(
        &slice,
        &shape,
        [400.0, 400.0],
        [200.0, 200.0],
        [220.0; 3],
        rotation,
    );
    assert_eq!(overview.ranges, slice.ranges);
    let zoomed = visible_volume_slice(
        &slice,
        &shape,
        [400.0, 400.0],
        [200.0, 200.0],
        [880.0; 3],
        rotation,
    );
    assert!(zoomed.ranges[0][1] - zoomed.ranges[0][0] < 384);
    assert!(zoomed.ranges[1][1] - zoomed.ranges[1][0] < 384);
    assert_eq!(zoomed.ranges[2], slice.ranges[2]);
    let panned = visible_volume_slice(
        &slice,
        &shape,
        [400.0, 400.0],
        [320.0, 200.0],
        [880.0; 3],
        rotation,
    );
    assert!(panned.ranges[0][0] > zoomed.ranges[0][0]);
}

#[test]
fn rotated_volume_crop_contains_every_projected_visible_voxel() {
    let shape = [64, 64, 64];
    let slice = VolumeSlice::new(&shape, false, PinnedDefault::Start);
    let rotation = VolumeRotation::from_angles(0.7, 0.5);
    let size = [240.0, 220.0];
    let center = [120.0, 110.0];
    let scale = [900.0; 3];
    let crop = visible_volume_slice(&slice, &shape, size, center, scale, rotation);
    for x in 0..64 {
        for y in 0..64 {
            for z in 0..64 {
                let point = [x, y, z].map(|index| 0.5 - (index as f32 + 0.5) / 64.0);
                let screen = project_rotated(point, center, scale, rotation).0;
                if screen[0] >= 0.0
                    && screen[0] < size[0]
                    && screen[1] >= 0.0
                    && screen[1] < size[1]
                {
                    assert!((crop.ranges[0][0]..=crop.ranges[0][1]).contains(&x));
                    assert!((crop.ranges[1][0]..=crop.ranges[1][1]).contains(&y));
                    assert!((crop.ranges[2][0]..=crop.ranges[2][1]).contains(&z));
                }
            }
        }
    }
}

#[test]
fn near_face_projects_larger_than_far_face() {
    let center = [200.0, 150.0];
    let scale = [200.0; 3];
    let face_width = |z| {
        let left = project_rotated([-0.5, 0.0, z], center, scale, VolumeRotation::identity()).0[0];
        let right = project_rotated([0.5, 0.0, z], center, scale, VolumeRotation::identity()).0[0];
        right - left
    };
    assert!(face_width(0.5) > face_width(-0.5));
    assert!(face_width(0.5) / face_width(-0.5) > 1.15);
}
#[test]
fn planar_projections_look_along_the_remaining_axis() {
    let center = [0.0, 0.0];
    for (projection, in_plane) in [
        (VolumeProjection::Xy, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
        (VolumeProjection::Yz, [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
        (VolumeProjection::Zx, [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0]]),
    ] {
        let (yaw, pitch) = projection.angles();
        for point in in_plane {
            assert!(project(point, center, [1.0; 3], yaw, pitch).1.abs() < 1e-6);
        }
    }
}
#[test]
fn planar_axes_start_at_the_front_lower_left_corner() {
    for (projection, in_plane, depth_axis) in [
        (VolumeProjection::Xy, [0, 1], 2),
        (VolumeProjection::Yz, [1, 2], 0),
        (VolumeProjection::Zx, [2, 0], 1),
    ] {
        let (yaw, pitch) = projection.angles();
        let corner = projection.axis_corner();
        let (screen, depth) = project(corner, [0.0, 0.0], [1.0; 3], yaw, pitch);
        let mut right = false;
        let mut up = false;
        for axis in in_plane {
            let mut end = corner;
            end[axis] = -end[axis];
            let projected = project(end, [0.0, 0.0], [1.0; 3], yaw, pitch).0;
            let dx = projected[0] - screen[0];
            let dy = projected[1] - screen[1];
            right |= dx > 0.9 && dy.abs() < 1e-5;
            up |= dy < -0.9 && dx.abs() < 1e-5;
        }
        assert!(right && up, "{projection:?} axes must point right and up");
        let mut back = corner;
        back[depth_axis] = -back[depth_axis];
        assert!(depth > project(back, [0.0, 0.0], [1.0; 3], yaw, pitch).1);
    }
}
#[test]
fn volume_axes_follow_the_front_volume_corner_after_rotation() {
    for rotation in [
        VolumeRotation::identity(),
        VolumeRotation::from_angles(0.65, 0.45),
        VolumeRotation::from_angles(-1.2, 0.8),
        VolumeRotation::from_angles(2.1, -0.6),
    ] {
        let corner = front_axis_corner(rotation, [0.5; 3], [[-0.5, 0.5]; 3]);
        assert!(corner.iter().all(|value| value.abs() == 0.5));
        let depth = project_rotated(corner, [0.0; 2], [1.0; 3], rotation).1;
        for index in 0..8 {
            let other =
                std::array::from_fn(|axis| if index & (1 << axis) == 0 { -0.5 } else { 0.5 });
            assert!(depth + 1e-5 >= project_rotated(other, [0.0; 2], [1.0; 3], rotation).1);
        }
    }
}

#[test]
fn cropped_volume_axes_use_the_same_bounds_as_the_voxels() {
    let shape = [20, 40, 80];
    let mut slice = VolumeSlice::new(&shape, false, PinnedDefault::Start);
    slice.ranges = vec![[0, 9], [10, 29], [40, 79]];
    let bounds = volume_visible_bounds(&slice, &shape);
    assert_eq!(bounds, [[0.0, 0.5], [-0.25, 0.25], [-0.5, 0.0]]);
    let rotation = VolumeRotation::from_angles(0.65, 0.45);
    let corner = front_axis_corner(rotation, [0.5; 3], bounds);
    for axis in 0..3 {
        assert!(bounds[axis].contains(&corner[axis]));
        assert!(corner[axis] == bounds[axis][0] || corner[axis] == bounds[axis][1]);
    }
}
#[test]
fn logarithmic_volume_scale_expands_small_values_and_supports_signs() {
    let linear = VolumeColorScale::new(Some((0.0, 100.0)), false);
    let logarithmic = VolumeColorScale::new(Some((0.0, 100.0)), true);
    assert!(logarithmic.fraction(1.0) > linear.fraction(1.0));
    let fraction = logarithmic.fraction(1.0) as f64;
    assert!((logarithmic.value(fraction).unwrap() - 1.0).abs() < 1e-4);
    let signed = VolumeColorScale::new(Some((-100.0, 100.0)), true);
    assert!((signed.fraction(0.0) - 0.5).abs() < 1e-6);
}

#[test]
fn volume_auto_colors_ignore_extreme_outliers_and_special_values() {
    let mut values = (0..100).map(|value| value as f32).collect::<Vec<_>>();
    values.push(10_000.0);
    values.push(-10_000.0);
    let mut specials = vec![0; values.len()];
    specials[101] = 1;
    let range = volume_auto_color_range(&values, &specials, Some((0.0, 10_000.0))).unwrap();
    assert!(range.0 >= 4.0 && range.0 <= 6.0);
    assert!(range.1 >= 94.0 && range.1 <= 96.0);
    assert!(VolumeColorScale::new(Some(range), false).fraction(50.0) > 0.4);
}
#[test]
fn volume_defaults_and_axis_swaps() {
    let mut slice = VolumeSlice::new(&[5, 6, 7, 8], true, PinnedDefault::Middle);
    assert_eq!(slice.hidden, Some(0));
    assert_eq!(slice.axes, [Some(1), Some(2), Some(3)]);
    assert_eq!(slice.pinned, 2);
    assign_volume_axis(&mut slice, 0, 3);
    assert_eq!(slice.axes, [Some(3), Some(2), Some(1)]);
    assert_eq!(slice.hidden, Some(0));
    assign_volume_axis(&mut slice, 3, 2);
    assert_eq!(slice.hidden, Some(2));
    assert_eq!(slice.axes, [Some(3), Some(0), Some(1)]);
}
#[test]
fn rectangular_volume_preserves_native_pixel_aspect() {
    let shape = [32, 145, 1024];
    let slice = VolumeSlice::new(&shape, false, PinnedDefault::Start);
    let ratios = volume_axis_ratios(&slice, &shape);
    assert_eq!(ratios, [32.0 / 1024.0, 145.0 / 1024.0, 1.0]);
}

#[test]
fn decreasing_yaw_moves_the_front_of_the_volume_right() {
    let center = [100.0, 100.0];
    let front = [0.0, 0.0, 0.5];
    let before = project(front, center, [100.0; 3], 0.0, 0.0).0;
    let after = project(front, center, [100.0; 3], -0.1, 0.0).0;
    assert!(after[0] > before[0]);
}
#[test]
fn trackball_drag_orbits_at_center_and_rolls_at_edge() {
    let center = [50.0, 50.0];
    let orbit = VolumeRotation::identity().drag(center, [60.0, 50.0], center, 40.0);
    let front = project_rotated([0.0, 0.0, 1.0], center, [10.0; 3], orbit);
    assert!(front.0[0] > center[0]);

    let roll = VolumeRotation::identity().drag([90.0, 50.0], [50.0, 90.0], center, 40.0);
    let right = project_rotated([1.0, 0.0, 0.0], center, [10.0; 3], roll);
    assert!((right.0[0] - center[0]).abs() < 1e-5);
    assert!(right.0[1] > center[1]);
    assert!(right.1.abs() < 1e-5);

    let restored = roll.drag([50.0, 90.0], [90.0, 50.0], center, 40.0);
    let point = project_rotated([1.0, 0.0, 0.0], center, [10.0; 3], restored);
    assert!((point.0[0] - 60.0).abs() < 1e-5);
    assert!((point.0[1] - 50.0).abs() < 1e-5);
}

#[test]
fn translated_cropped_volume_rotates_around_its_visible_center() {
    let shape = [20, 40, 80];
    let mut slice = VolumeSlice::new(&shape, false, PinnedDefault::Start);
    slice.ranges = vec![[0, 9], [10, 29], [40, 79]];
    let pivot = volume_visible_center(&slice, &shape);
    let screen_center = [240.0, 180.0];
    let scale = [40.0, 80.0, 160.0];
    for rotation in [
        VolumeRotation::identity(),
        VolumeRotation::from_angles(0.8, 0.4),
        VolumeRotation::from_angles(-1.1, 0.9),
    ] {
        let projection_center = volume_projection_center(screen_center, pivot, scale, rotation);
        let projected = project_rotated(pivot, projection_center, scale, rotation).0;
        assert!((projected[0] - screen_center[0]).abs() < 1e-4);
        assert!((projected[1] - screen_center[1]).abs() < 1e-4);
    }
}
#[test]
fn depth_shading_dims_farther_geometry_without_changing_opacity() {
    let shading = DepthShading::new(VolumeRotation::from_angles(0.65, 0.45), 0.45, [1.0; 3]);
    let limit = shading.limit;
    let color = [0.8, 0.6, 0.4, 0.25];
    let back = shading.shade(color, -limit);
    let front = shading.shade(color, limit);
    assert!(back[..3].iter().zip(front[..3].iter()).all(|(a, b)| a < b));
    assert_eq!(front, color);
    assert_eq!(back[3], color[3]);
}
#[test]
fn opacity_slider_spreads_low_values_over_a_useful_range() {
    assert_eq!(opacity_from_slider(0.0), 0.0);
    assert_eq!(opacity_from_slider(0.25), 0.0625);
    assert_eq!(opacity_from_slider(0.5), 0.25);
    assert_eq!(opacity_from_slider(0.75), 0.5625);
    assert_eq!(opacity_from_slider(1.0), 1.0);
}
#[test]
fn smaller_datasets_can_pin_one_axis() {
    let mut slice = VolumeSlice::new(&[4, 5, 6], false, PinnedDefault::Middle);
    assert_eq!(slice.axes, [Some(0), Some(1), Some(2)]);
    slice.set_hidden(true, PinnedDefault::Middle, &[4, 5, 6]);
    assert_eq!(slice.axes, [Some(0), Some(1), None]);
    assert_eq!(slice.hidden, Some(2));
    assert_eq!(slice.pinned, 3);
    assign_volume_axis(&mut slice, 2, 0);
    assert_eq!(slice.axes, [Some(2), Some(1), None]);
    assert_eq!(slice.hidden, Some(0));
    slice.set_hidden(false, PinnedDefault::Middle, &[4, 5, 6]);
    assert_eq!(slice.axes, [Some(2), Some(1), Some(0)]);
    assert_eq!(slice.hidden, None);
}

#[test]
fn exact_volume_preserves_every_native_cell_and_axis_order() {
    let path = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/space_demo.hdf5"
    ));
    let file = File::open(path).unwrap();
    let shape = [8, 32, 48, 16];
    let slice = VolumeSlice::new(&shape, true, PinnedDefault::Middle);
    let exact =
        load_exact_volume(&file, "/observations/time_spectral_cube_4d", &shape, &slice).unwrap();
    assert_eq!(exact.dimensions, [32, 48, 16]);
    assert_eq!(exact.values.len(), 32 * 48 * 16);
    for [x, y, z] in [[0, 0, 0], [15, 23, 8], [31, 47, 15]] {
        let source = file
            .region(
                "/observations/time_spectral_cube_4d",
                &[4, x, y, z],
                &[1, 1, 1, 1],
            )
            .unwrap();
        let index = x * exact.strides[0] + y * exact.strides[1] + z * exact.strides[2];
        assert_eq!(exact.values[index], source.data[0] as f32);
    }
}

#[test]
fn s5p_large_radiance_uses_a_bounded_volume_preview_when_available() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mock_datasets/S5P");
    let Some(path) = std::fs::read_dir(directory).ok().and_then(|entries| {
        entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("S5P_OFFL_L1B_RA_BD3_"))
            })
    }) else {
        return;
    };
    let file = File::open(&path).unwrap();
    let dataset = "/BAND3_RADIANCE/STANDARD_MODE/OBSERVATIONS/radiance";
    let shape = file.dataset_info(dataset).unwrap().shape;
    assert!(file.axis_names(dataset).unwrap()[0].contains("time"));
    let slice = VolumeSlice::new(&shape, true, PinnedDefault::Middle);
    let volume = load_volume_data(&file, dataset, &shape, &slice, 100_000).unwrap();
    assert!(volume.sampled);
    assert!(volume.values.len() <= 100_000);
    assert_eq!(volume.values.len(), volume.specials.len());
    assert!(volume.color_range.is_some_and(|(low, high)| low < high));
    assert!(volume.dimensions.iter().all(|&length| length > 1));
}

#[test]
#[ignore = "manual validation with a 795 MB external GOME-2 product"]
fn exact_gome2_radiance_reads_all_voxels() {
    let path = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../mock_datasets/GOME2/01/15/20130115084607Z_32387_gome.nc"
    ));
    let file = File::open(path).unwrap();
    let shape = [1, 592, 32, 1024];
    let slice = VolumeSlice::new(&shape, true, PinnedDefault::Middle);
    let exact = load_exact_volume(
        &file,
        "/MODE_NADIR/BAND_3/OBSERVATIONS/radiance",
        &shape,
        &slice,
    )
    .unwrap();
    assert_eq!(exact.values.len(), 592 * 32 * 1024);
    for [x, y, z] in [[0, 0, 0], [296, 16, 512], [591, 31, 1023]] {
        let source = file
            .region(
                "/MODE_NADIR/BAND_3/OBSERVATIONS/radiance",
                &[0, x, y, z],
                &[1, 1, 1, 1],
            )
            .unwrap();
        let index = x * exact.strides[0] + y * exact.strides[1] + z * exact.strides[2];
        assert_eq!(exact.values[index], source.data[0] as f32);
    }
}
