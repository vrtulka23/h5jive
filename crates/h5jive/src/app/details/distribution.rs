use super::super::*;

impl App {
    pub(crate) fn draw_distribution_tab(&mut self, ui: &Ui) {
        let Some(selection) = self.selection.clone() else {
            ui.text_wrapped("Select a numeric dataset to see its distribution.");
            return;
        };
        let Some(entry) = find_entry(&self.files[selection.file].tree, &selection.path) else {
            return;
        };
        if !matches!(entry.kind, Kind::Dataset { numeric: true, .. }) {
            ui.text_wrapped("Select a numeric dataset to see its distribution.");
            return;
        }
        self.poll_statistics(&selection);
        if self.statistics_cache.is_none() {
            ui.text("Calculating statistics...");
            return;
        }
        let statistics = match self.statistics_cache.as_ref().unwrap() {
            Ok(statistics) => statistics,
            Err(err) => {
                ui.text_wrapped(format!("Distribution unavailable: {err}"));
                return;
            }
        };
        if statistics.valid == 0 {
            ui.text("No finite, non-fill values to plot.");
            return;
        }
        let statistics = statistics.clone();
        self.poll_distribution(&selection, &statistics);
        if self.distribution_cache.is_none() {
            ui.text("Calculating distribution...");
            return;
        }
        match self.distribution_cache.as_ref().unwrap() {
            Ok(distribution) => draw_distribution(ui, &statistics, distribution, &self.settings),
            Err(err) => ui.text_wrapped(format!("Distribution unavailable: {err}")),
        }
    }
}

fn plot_count(value: u128) -> String {
    if value >= 1_000_000 {
        format!("{:.1e}", value as f64)
    } else {
        value.to_string()
    }
}
fn draw_distribution(
    ui: &Ui,
    stats: &DatasetStatistics,
    distribution: &DatasetDistribution,
    settings: &Settings,
) {
    let available = ui.content_region_avail();
    if available[0] < settings.distribution_min_width
        || available[1] < settings.distribution_min_height
    {
        ui.text_wrapped("Enlarge this panel to see the histogram and box plot.");
        return;
    }
    let size = [available[0] - 2.0, available[1] - 2.0];
    ui.dummy(size);
    let origin = ui.item_rect_min();
    let max_bin_count = distribution.bins.iter().copied().max().unwrap_or(1).max(1);
    let left = origin[0] + ui.calc_text_size(plot_count(max_bin_count))[0] + 12.0;
    let right = origin[0] + size[0] - 8.0;
    let min = stats.min.unwrap();
    let max = stats.max.unwrap();
    let box_min = distribution.lower_whisker;
    let box_max = distribution.upper_whisker;
    let box_left = left + 5.0;
    let box_right = right - 5.0;
    let box_position = |value: f64| {
        let fraction = if box_min == box_max {
            0.5
        } else if (box_max - box_min).is_finite() {
            (value - box_min) / (box_max - box_min)
        } else {
            (value * 0.5 - box_min * 0.5) / (box_max * 0.5 - box_min * 0.5)
        };
        box_left + (box_right - box_left) * fraction.clamp(0.0, 1.0) as f32
    };
    let draw = ui.get_window_draw_list();
    let text = settings.palette.text;
    let accent = settings.palette.accent;
    let mut box_color = accent;
    box_color[3] = settings.palette.box_alpha;
    draw.add_text([left, origin[1] + 2.0], text, "Histogram (valid values)");
    let histogram_top = origin[1] + 24.0;
    let histogram_bottom = origin[1] + size[1] * 0.43;
    let max_count = max_bin_count as f64;
    let bar_width = (right - left) / distribution.bins.len() as f32;
    for (index, &count) in distribution.bins.iter().enumerate() {
        let height = (histogram_bottom - histogram_top) * (count as f64 / max_count) as f32;
        let x = left + index as f32 * bar_width;
        draw.add_rect(
            [x + 0.5, histogram_bottom - height],
            [x + bar_width - 0.5, histogram_bottom],
            accent,
        )
        .filled(true)
        .build();
    }
    draw.add_line([left, histogram_top], [left, histogram_bottom], text)
        .build();
    draw.add_line([left, histogram_bottom], [right, histogram_bottom], text)
        .build();
    let mut previous_count = None;
    let y_intervals = settings.distribution_y_tick_intervals as u128;
    for step in 0..=y_intervals {
        let count = (max_bin_count / y_intervals) * step
            + (max_bin_count % y_intervals) * step / y_intervals;
        if previous_count == Some(count) {
            continue;
        }
        previous_count = Some(count);
        let y = histogram_bottom
            - (histogram_bottom - histogram_top) * (count as f64 / max_count) as f32;
        draw.add_line([left - 4.0, y], [left, y], text).build();
        let label = plot_count(count);
        let label_size = ui.calc_text_size(&label);
        draw.add_text(
            [left - label_size[0] - 7.0, y - label_size[1] * 0.5],
            text,
            label,
        );
    }
    let intervals =
        (((right - left) / settings.distribution_x_tick_spacing).floor() as usize).clamp(1, 5);
    for step in 0..=intervals {
        if min == max && step > 0 {
            break;
        }
        let fraction = step as f64 / intervals as f64;
        let value = min * (1.0 - fraction) + max * fraction;
        let x = if min == max {
            (left + right) * 0.5
        } else {
            left + (right - left) * fraction as f32
        };
        draw.add_line([x, histogram_bottom], [x, histogram_bottom + 5.0], text)
            .build();
        let label = format_value(value, settings.display_decimals);
        let label_width = ui.calc_text_size(&label)[0];
        draw.add_text(
            [
                (x - label_width * 0.5).clamp(origin[0], origin[0] + size[0] - label_width),
                histogram_bottom + 7.0,
            ],
            text,
            label,
        );
    }

    let box_label_y = origin[1] + size[1] * 0.57;
    draw.add_text([left, box_label_y], text, "Box plot (whisker range)");
    let box_y = origin[1] + size[1] * 0.70;
    let lower = box_position(distribution.lower_whisker);
    let upper = box_position(distribution.upper_whisker);
    let q1 = box_position(stats.first_quartile.unwrap());
    let q3 = box_position(stats.third_quartile.unwrap());
    let median = box_position(stats.median.unwrap());
    draw.add_line([lower, box_y], [upper, box_y], text).build();
    for x in [lower, upper] {
        draw.add_line([x, box_y - 8.0], [x, box_y + 8.0], text)
            .build();
    }
    draw.add_rect(
        [q1, box_y - 13.0],
        [q3.max(q1 + 2.0), box_y + 13.0],
        box_color,
    )
    .filled(true)
    .build();
    draw.add_line([median, box_y - 13.0], [median, box_y + 13.0], text)
        .thickness(2.0)
        .build();
    let quartiles = [
        ("Q1", stats.first_quartile.unwrap()),
        ("Median", stats.median.unwrap()),
        ("Q3", stats.third_quartile.unwrap()),
    ];
    for (index, (name, value)) in quartiles.into_iter().enumerate() {
        let label = format!("{name} {}", format_value(value, settings.display_decimals));
        let label_width = ui.calc_text_size(&label)[0];
        let x = match index {
            0 => left,
            1 => (left + right - label_width) * 0.5,
            _ => right - label_width,
        };
        draw.add_text([x, box_y + 19.0], text, label);
    }
    draw.add_text(
        [left, origin[1] + size[1] - 18.0],
        text,
        format!("# Outliers: {}", stats.outliers),
    );
    if stats.approximate_quartiles {
        let label = "Quartiles sampled";
        draw.add_text(
            [
                right - ui.calc_text_size(label)[0],
                origin[1] + size[1] - 18.0,
            ],
            text,
            label,
        );
    }
}
