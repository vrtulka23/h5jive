use super::*;
use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

fn count_overpass_hits(
    results: &HashMap<String, Result<Vec<OverpassGroupResult>, String>>,
) -> usize {
    results
        .values()
        .filter(|result| {
            result
                .as_ref()
                .is_ok_and(|groups| groups.iter().any(|group| group.hit))
        })
        .count()
}

fn overpass_group_names(
    results: &HashMap<String, Result<Vec<OverpassGroupResult>, String>>,
) -> Vec<String> {
    results
        .values()
        .filter_map(|result| result.as_ref().ok())
        .flat_map(|groups| groups.iter().map(|group| group.name.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

struct SourceIndex {
    products: Vec<Product>,
    error: Option<String>,
    scanning: bool,
    visited: usize,
    receiver: Option<mpsc::Receiver<ScanMessage>>,
    cancel: Option<Arc<AtomicBool>>,
    available: Option<bool>,
    availability_job: Option<mpsc::Receiver<bool>>,
    last_availability_check: Option<std::time::Instant>,
    metadata_job: Option<mpsc::Receiver<(String, Result<[String; 4], String>)>>,
    metadata_status: Option<String>,
    overpass_area_key: Option<String>,
    overpass_running_area_key: Option<String>,
    overpass_results: HashMap<String, Result<Vec<OverpassGroupResult>, String>>,
    overpass_groups: Vec<String>,
    overpass_cache_receiver: Option<
        mpsc::Receiver<(
            String,
            HashMap<String, Result<Vec<OverpassGroupResult>, String>>,
        )>,
    >,
    overpass_receiver: Option<mpsc::Receiver<OverpassMessage>>,
    overpass_cancel: Option<Arc<AtomicBool>>,
    overpass_done: usize,
    overpass_total: usize,
    overpass_hits: usize,
    overpass_cancelled: bool,
    overpass_error: Option<String>,
}

pub(super) struct DatasetBrowser {
    sources: Vec<SourceIndex>,
    selected_source: usize,
    selected_product: Option<String>,
    search: String,
    date_search: String,
    orbit_search: String,
    tag_search: String,
    tag_choice: usize,
    tags: Vec<String>,
    filtered: Vec<usize>,
    sort_column: usize,
    sort_descending: bool,
    dirty: bool,
    product_footer_height: f32,
    selected_area: usize,
    only_overpasses: bool,
    selected_group: usize,
}

impl DatasetBrowser {
    pub fn new(sources: &[DatasetSource]) -> Self {
        let mut tags: Vec<String> = sources
            .iter()
            .flat_map(|source| source.tags.iter().cloned())
            .collect();
        tags.sort();
        tags.dedup();
        tags.insert(0, "All tags".into());
        let indexes = sources
            .iter()
            .map(|source| {
                let cached = read_cache(source);
                SourceIndex {
                    products: cached.unwrap_or_default(),
                    error: None,
                    scanning: false,
                    visited: 0,
                    receiver: None,
                    cancel: None,
                    available: None,
                    availability_job: None,
                    last_availability_check: None,
                    metadata_job: None,
                    metadata_status: None,
                    overpass_area_key: None,
                    overpass_running_area_key: None,
                    overpass_results: HashMap::new(),
                    overpass_groups: Vec::new(),
                    overpass_cache_receiver: None,
                    overpass_receiver: None,
                    overpass_cancel: None,
                    overpass_done: 0,
                    overpass_total: 0,
                    overpass_hits: 0,
                    overpass_cancelled: false,
                    overpass_error: None,
                }
            })
            .collect();
        Self {
            sources: indexes,
            selected_source: 0,
            selected_product: None,
            search: String::new(),
            date_search: String::new(),
            orbit_search: String::new(),
            tag_search: String::new(),
            tag_choice: 0,
            tags,
            filtered: Vec::new(),
            sort_column: 0,
            sort_descending: false,
            dirty: true,
            product_footer_height: 105.0,
            selected_area: 0,
            only_overpasses: false,
            selected_group: 0,
        }
    }

    pub fn active_task_labels(&self) -> Vec<&'static str> {
        self.sources
            .iter()
            .filter_map(|source| {
                if source.scanning {
                    Some("index")
                } else if source.metadata_job.is_some() {
                    Some("metadata")
                } else if source.overpass_receiver.is_some()
                    || source.overpass_cache_receiver.is_some()
                {
                    Some("overpasses")
                } else if source.availability_job.is_some() {
                    Some("source check")
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn poll(&mut self, sources: &[DatasetSource]) {
        for (number, (index, config)) in self.sources.iter_mut().zip(sources).enumerate() {
            if let Some(receiver) = &index.overpass_cache_receiver {
                match receiver.try_recv() {
                    Ok((key, results)) => {
                        if index.overpass_area_key.as_deref() == Some(key.as_str()) {
                            index.overpass_done = results.len();
                            index.overpass_hits = count_overpass_hits(&results);
                            index.overpass_total = index.products.len();
                            index.overpass_groups = overpass_group_names(&results);
                            index.overpass_results = results;
                            if number == self.selected_source && !index.overpass_results.is_empty()
                            {
                                self.only_overpasses = true;
                            }
                            self.dirty = true;
                        }
                        index.overpass_cache_receiver = None;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => index.overpass_cache_receiver = None,
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            if let Some(receiver) = &index.overpass_receiver {
                loop {
                    match receiver.try_recv() {
                        Ok(OverpassMessage::Progress { done, total, hits }) => {
                            index.overpass_done = done;
                            index.overpass_total = total;
                            index.overpass_hits = hits;
                        }
                        Ok(OverpassMessage::Finished {
                            results,
                            cancelled,
                            cache_error,
                        }) => {
                            if index.overpass_area_key == index.overpass_running_area_key {
                                index.overpass_done = results.len();
                                index.overpass_hits = count_overpass_hits(&results);
                                index.overpass_groups = overpass_group_names(&results);
                                index.overpass_results = results;
                                index.overpass_cancelled = cancelled;
                                index.overpass_error = cache_error;
                                if number == self.selected_source
                                    && !index.overpass_results.is_empty()
                                {
                                    self.only_overpasses = true;
                                }
                                self.dirty = true;
                            }
                            index.overpass_running_area_key = None;
                            index.overpass_receiver = None;
                            index.overpass_cancel = None;
                            self.dirty = true;
                            break;
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            index.overpass_receiver = None;
                            index.overpass_cancel = None;
                            index.overpass_cancelled = true;
                            break;
                        }
                    }
                }
            }
            if let Some(job) = &index.metadata_job {
                match job.try_recv() {
                    Ok((relative, Ok(fields))) => {
                        if let Some(product) = index
                            .products
                            .iter_mut()
                            .find(|product| product.relative_path == relative)
                        {
                            for (current, value) in [
                                &mut product.year,
                                &mut product.month,
                                &mut product.day,
                                &mut product.orbit,
                            ]
                            .into_iter()
                            .zip(fields)
                            {
                                if current.is_empty() {
                                    *current = value;
                                }
                            }
                            let config = config.clone();
                            let products = index.products.clone();
                            thread::spawn(move || {
                                let _ = write_cache(&config, &products);
                            });
                            self.dirty = true;
                        }
                        index.metadata_status = Some("Product metadata read".into());
                        index.metadata_job = None;
                    }
                    Ok((_, Err(error))) => {
                        index.metadata_status = Some(error);
                        index.metadata_job = None;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        index.metadata_status = Some("Metadata reader stopped".into());
                        index.metadata_job = None;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            if let Some(job) = &index.availability_job {
                match job.try_recv() {
                    Ok(available) => {
                        index.available = Some(available);
                        index.availability_job = None;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => index.availability_job = None,
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            if index.availability_job.is_none()
                && index
                    .last_availability_check
                    .is_none_or(|time| time.elapsed().as_secs() >= 10)
            {
                let root = config.root.clone();
                let (sender, receiver) = mpsc::channel();
                thread::spawn(move || {
                    let _ = sender.send(root.is_dir());
                });
                index.availability_job = Some(receiver);
                index.last_availability_check = Some(std::time::Instant::now());
            }
            let Some(receiver) = &index.receiver else {
                continue;
            };
            loop {
                match receiver.try_recv() {
                    Ok(ScanMessage::Progress(visited)) => index.visited = visited,
                    Ok(ScanMessage::Finished(result)) => {
                        match result {
                            Ok((products, cache_error)) => {
                                index.products = products;
                                index.error = cache_error;
                                index.overpass_area_key = None;
                                index.overpass_cache_receiver = None;
                                index.overpass_results.clear();
                                index.overpass_groups.clear();
                                if number == self.selected_source {
                                    self.only_overpasses = false;
                                    self.selected_group = 0;
                                }
                                if number == self.selected_source
                                    && self.selected_product.as_ref().is_some_and(|selected| {
                                        !index
                                            .products
                                            .iter()
                                            .any(|product| &product.relative_path == selected)
                                    })
                                {
                                    self.selected_product = None;
                                }
                                self.dirty = true;
                            }
                            Err(error) => index.error = Some(error),
                        }
                        index.scanning = false;
                        index.receiver = None;
                        index.cancel = None;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        index.error = Some("Scan worker stopped unexpectedly".into());
                        index.scanning = false;
                        index.receiver = None;
                        index.cancel = None;
                        break;
                    }
                }
            }
        }
    }

    pub fn refresh(&mut self, source_number: usize, source: &DatasetSource) {
        let Some(index) = self.sources.get_mut(source_number) else {
            return;
        };
        if index.scanning {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let source = source.clone();
        thread::spawn(move || {
            let result = scan_source(&source, &sender, &worker_cancel).map(|products| {
                let cache_error = write_cache(&source, &products)
                    .err()
                    .map(|error| format!("Products found, but cache could not be saved: {error}"));
                (products, cache_error)
            });
            let _ = sender.send(ScanMessage::Finished(result));
        });
        index.visited = 0;
        index.error = None;
        index.scanning = true;
        index.cancel = Some(cancel);
        index.receiver = Some(receiver);
    }

    fn check_overpasses(
        &mut self,
        source_number: usize,
        source: &DatasetSource,
        area: &OverpassArea,
    ) {
        let index = &mut self.sources[source_number];
        if index.overpass_receiver.is_some() {
            return;
        }
        let products = index.products.clone();
        let source = source.clone();
        let area = area.clone();
        let area_key = area.key.clone();
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        thread::spawn(move || scan_overpasses(&source, &products, &area, &sender, &worker_cancel));
        index.overpass_area_key = Some(area_key.clone());
        index.overpass_running_area_key = Some(area_key);
        index.overpass_cache_receiver = None;
        index.overpass_receiver = Some(receiver);
        index.overpass_cancel = Some(cancel);
        index.overpass_done = 0;
        index.overpass_total = index.products.len();
        index.overpass_hits = 0;
        index.overpass_cancelled = false;
        index.overpass_error = None;
        self.dirty = true;
    }

    fn show_cached_area(
        &mut self,
        source_number: usize,
        source: &DatasetSource,
        area: &OverpassArea,
    ) {
        let index = &mut self.sources[source_number];
        if index.overpass_area_key.as_deref() == Some(area.key.as_str()) {
            return;
        }
        let source = source.clone();
        let products = index.products.clone();
        let area = area.clone();
        let area_key = area.key.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let results = cached_overpass_results(&source, &products, &area);
            let _ = sender.send((area.key, results));
        });
        index.overpass_area_key = Some(area_key);
        index.overpass_results.clear();
        index.overpass_groups.clear();
        index.overpass_cache_receiver = Some(receiver);
        index.overpass_done = 0;
        index.overpass_total = index.products.len();
        index.overpass_hits = 0;
        self.only_overpasses = false;
        self.selected_group = 0;
        self.dirty = true;
    }

    fn read_selected_metadata(
        &mut self,
        source_number: usize,
        source: &DatasetSource,
        relative: String,
    ) {
        let index = &mut self.sources[source_number];
        if index.metadata_job.is_some() {
            return;
        }
        let path = source.root.join(&relative);
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = read_product_metadata(&path);
            let _ = sender.send((relative, result));
        });
        index.metadata_status = None;
        index.metadata_job = Some(receiver);
    }

    fn rebuild_visible(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let Some(source) = self.sources.get(self.selected_source) else {
            return;
        };
        self.filtered = matching_products(
            &source.products,
            &self.search,
            &self.date_search,
            &self.orbit_search,
            self.sort_column,
            self.sort_descending,
        );
        if self.only_overpasses {
            let selected_group = self
                .selected_group
                .checked_sub(1)
                .and_then(|index| source.overpass_groups.get(index));
            self.filtered.retain(|&position| {
                source
                    .overpass_results
                    .get(&source.products[position].relative_path)
                    .is_some_and(|result| {
                        result.as_ref().is_ok_and(|groups| {
                            groups.iter().any(|group| {
                                group.hit && selected_group.is_none_or(|name| name == &group.name)
                            })
                        })
                    })
            });
        }
    }

    pub fn draw(
        &mut self,
        ui: &Ui,
        sources: &[DatasetSource],
        areas: &[OverpassArea],
        pending_b: Option<&Path>,
        palette: &Palette,
        height: f32,
    ) -> Option<(usize, PathBuf)> {
        self.poll(sources);
        if sources.is_empty() {
            ui.text("No datasets are registered in DIPL.");
            return None;
        }
        let mut open = None;
        let mut any_visible = false;
        let source_surface = palette.background;
        let product_surface = palette.background;
        let source_top = ui.cursor_screen_pos();
        let _source_padding = ui.push_style_var(StyleVar::WindowPadding(BROWSER_PANEL_PADDING));
        let _source_spacing = ui.push_style_var(StyleVar::ItemSpacing([PANEL_GAP, PANEL_GAP]));
        ui.child_window("##dataset_sources")
            .size([250.0, height])
            .child_flags(ChildFlags::ALWAYS_USE_WINDOW_PADDING)
            .build(ui, || {
                ui.set_next_item_width(-1.0);
                ui.combo_simple_string("##dataset_tag_group", &mut self.tag_choice, &self.tags);
                ui.set_next_item_width(-1.0);
                ui.input_text("##dataset_tag_search", &mut self.tag_search)
                    .hint("Search tags")
                    .build();
                ui.separator();
                let selected_tag = self
                    .tags
                    .get(self.tag_choice)
                    .filter(|_| self.tag_choice > 0)
                    .map(String::as_str);
                let tag_query = self.tag_search.trim().to_ascii_lowercase();
                let visible =
                    |source: &DatasetSource| source_matches_tags(source, selected_tag, &tag_query);
                if !visible(&sources[self.selected_source]) {
                    if let Some(first) = sources.iter().position(visible) {
                        self.selected_source = first;
                        self.selected_product = None;
                        self.only_overpasses = false;
                        self.selected_group = 0;
                        self.dirty = true;
                    }
                }
                for (number, source) in sources.iter().enumerate() {
                    if !visible(source) {
                        continue;
                    }
                    any_visible = true;
                    let count = self.sources[number].products.len();
                    let (color, tooltip) = match self.sources[number].available {
                        Some(true) => ([0.25, 0.78, 0.38, 1.0], "Dataset directory available"),
                        Some(false) => ([0.9, 0.3, 0.28, 1.0], "Dataset directory unavailable"),
                        None => ([0.65, 0.65, 0.65, 1.0], "Checking dataset directory"),
                    };
                    let selected = self.selected_source == number;
                    let clicked = ui.invisible_button(
                        format!("##source_{number}"),
                        [ui.content_region_avail()[0], ui.frame_height().max(30.0)],
                    );
                    let min = ui.item_rect_min();
                    let max = ui.item_rect_max();
                    let hovered = ui.is_item_hovered();
                    let background = if selected {
                        mix_color(source_surface, palette.accent, 0.30)
                    } else if hovered {
                        mix_color(source_surface, palette.text, 0.10)
                    } else {
                        source_surface
                    };
                    let draw = ui.get_window_draw_list();
                    draw.add_rect(min, max, background)
                        .filled(true)
                        .rounding(3.0)
                        .build();
                    if selected {
                        draw.add_rect(min, [min[0] + 3.0, max[1]], palette.accent)
                            .filled(true)
                            .build();
                    }
                    let center_y = (min[1] + max[1]) * 0.5;
                    draw.add_circle([min[0] + 14.0, center_y], 4.5, color)
                        .filled(true)
                        .build();
                    let count_text = count.to_string();
                    let text_y = center_y - ui.text_line_height() * 0.5;
                    let count_width = ui.calc_text_size(&count_text)[0];
                    draw.add_text(
                        [max[0] - count_width - 10.0, text_y],
                        palette.muted,
                        &count_text,
                    );
                    draw.with_clip_rect(
                        [min[0] + 24.0, min[1]],
                        [max[0] - count_width - 16.0, max[1]],
                        || {
                            draw.add_text([min[0] + 24.0, text_y], palette.text, &source.name);
                        },
                    );
                    if hovered {
                        ui.set_item_tooltip(format!(
                            "{} | {count} products | {tooltip}",
                            source.name
                        ));
                    }
                    if clicked {
                        self.selected_source = number;
                        self.selected_product = None;
                        self.only_overpasses = false;
                        self.selected_group = 0;
                        self.dirty = true;
                    }
                }
                ui.get_window_draw_list()
                    .add_line(
                        [source_top[0] + 249.0, source_top[1]],
                        [source_top[0] + 249.0, source_top[1] + height],
                        palette.border,
                    )
                    .build();
            });
        drop(_source_spacing);
        drop(_source_padding);
        ui.same_line();
        let _product_padding = ui.push_style_var(StyleVar::WindowPadding(BROWSER_PANEL_PADDING));
        let _product_spacing = ui.push_style_var(StyleVar::ItemSpacing([PANEL_GAP, PANEL_GAP]));
        ui.child_window("##dataset_products")
            .size([0.0, height])
            .child_flags(ChildFlags::ALWAYS_USE_WINDOW_PADDING)
            .build(ui, || {
                if !any_visible {
                    ui.text("No datasets match the selected tags.");
                    return;
                }
                let source_number = self.selected_source;
                let source = &sources[source_number];
                if let Some(area) = areas.get(self.selected_area) {
                    self.show_cached_area(source_number, source, area);
                }
                let index = &self.sources[source_number];
                ui.text(&source.name);
                ui.same_line();
                ui.text_disabled(format!("{} products", index.products.len()));
                ui.text_disabled(format!(
                    "{}  |  {}  |  {}",
                    source.id,
                    source.format,
                    source.tags.join(", ")
                ));
                ui.text_wrapped(source.root.display().to_string());
                if let Some(path) = pending_b {
                    ui.text_wrapped(format!(
                        "File B queued until file A loads: {}",
                        path.display()
                    ));
                }
                if self.sources[source_number].available == Some(false) {
                    ui.text_colored(
                        [0.9, 0.3, 0.28, 1.0],
                        "Dataset directory unavailable; cached index remains visible.",
                    );
                }
                ui.spacing();
                if index.scanning {
                    ui.text(format!("Scanning: {} files visited", index.visited));
                    if ui.button("Cancel scan") {
                        if let Some(cancel) = &index.cancel {
                            cancel.store(true, Ordering::Relaxed);
                        }
                    }
                } else if ui.button(if index.products.is_empty() {
                    "Scan products"
                } else {
                    "Refresh index"
                }) {
                    self.refresh(source_number, source);
                }
                if let Some(error) = &self.sources[source_number].error {
                    ui.text_wrapped(format!("Index: {error}"));
                }
                ui.separator();
                ui.text("FIND PRODUCTS");
                ui.set_next_item_width(-1.0);
                if ui
                    .input_text("##product_name_search", &mut self.search)
                    .hint("Search name or path")
                    .build()
                {
                    self.dirty = true;
                    self.selected_product = None;
                }
                if let Some(_filters) = ui.begin_table("##product_search_filters", 2) {
                    ui.table_next_row();
                    ui.table_next_column();
                    ui.set_next_item_width(-1.0);
                    if ui
                        .input_text("##product_date_search", &mut self.date_search)
                        .hint("Date (YYYY-MM-DD)")
                        .build()
                    {
                        self.dirty = true;
                        self.selected_product = None;
                    }
                    ui.table_next_column();
                    ui.set_next_item_width(-1.0);
                    if ui
                        .input_text("##product_orbit_search", &mut self.orbit_search)
                        .hint("Orbit")
                        .build()
                    {
                        self.dirty = true;
                        self.selected_product = None;
                    }
                }
                if date_query(&self.date_search).is_none() {
                    ui.text_disabled("Date accepts digits, hyphens, or slashes.");
                }
                if !areas.is_empty() {
                    ui.text("Overpass area");
                    ui.same_line();
                    let labels: Vec<&str> = areas.iter().map(|area| area.label.as_str()).collect();
                    ui.set_next_item_width((ui.content_region_avail()[0] * 0.48).max(100.0));
                    if ui.combo_simple_string("##overpass_area", &mut self.selected_area, &labels) {
                        self.only_overpasses = false;
                        self.selected_group = 0;
                        self.dirty = true;
                    }
                    let area = &areas[self.selected_area.min(areas.len() - 1)];
                    self.show_cached_area(source_number, source, area);
                    if self.sources[source_number].overpass_receiver.is_none()
                        && self.sources[source_number].available != Some(false)
                        && !self.sources[source_number].scanning
                    {
                        ui.same_line();
                        if ui.button(if self.sources[source_number].overpass_results.is_empty() {
                            "Check overpasses"
                        } else {
                            "Refresh overpasses"
                        }) {
                            self.check_overpasses(source_number, source, area);
                        }
                    }
                    if let Some(cancel) = &self.sources[source_number].overpass_cancel {
                        ui.same_line();
                        if ui.button("Cancel check") {
                            cancel.store(true, Ordering::Relaxed);
                        }
                    }
                    let index = &self.sources[source_number];
                    if index.overpass_receiver.is_some() {
                        ui.text(format!(
                            "Checking {}/{} products · {} hits",
                            index.overpass_done, index.overpass_total, index.overpass_hits
                        ));
                    } else if !index.overpass_results.is_empty() {
                        let label = if index.overpass_cancelled {
                            "Check stopped"
                        } else {
                            "Saved checks"
                        };
                        ui.text(format!(
                            "{label}: {}/{} products · {} hits",
                            index.overpass_done, index.overpass_total, index.overpass_hits
                        ));
                    }
                    if !index.overpass_results.is_empty() {
                        if index.overpass_groups.len() > 1 {
                            let mut labels = vec!["Any group"];
                            labels.extend(index.overpass_groups.iter().map(String::as_str));
                            ui.set_next_item_width(180.0);
                            if ui.combo_simple_string(
                                "##overpass_group",
                                &mut self.selected_group,
                                &labels,
                            ) {
                                self.dirty = true;
                            }
                            ui.same_line();
                        }
                        let label = if self.only_overpasses {
                            "Show all products"
                        } else {
                            "Show overpasses"
                        };
                        if ui.button(label) {
                            self.only_overpasses = !self.only_overpasses;
                            self.dirty = true;
                        }
                    }
                    if let Some(error) = &self.sources[source_number].overpass_error {
                        ui.text_wrapped(format!("Overpass cache: {error}"));
                    }
                }
                self.rebuild_visible();
                ui.separator();
                ui.text(format!("PRODUCTS ({} matching)", self.filtered.len()));
                let column_widths = [360.0, 250.0, 100.0, 80.0, 85.0];
                let preferred_width = column_widths.iter().sum::<f32>()
                    + column_widths.len() as f32 * TABLE_CELL_PADDING[0] * 2.0;
                let table_width = ui.content_region_avail()[0].min(preferred_width);
                let header_height = ui.frame_height() + 2.0 * TABLE_CELL_PADDING[1];
                let row_height = ui.text_line_height() + 2.0 * TABLE_CELL_PADDING[1];
                // Resized columns can need horizontal scrolling even when defaults fit.
                let horizontal_scrollbar = ui.clone_style().scrollbar_size();
                let content_height = header_height
                    + self.filtered.len() as f32 * row_height
                    + horizontal_scrollbar
                    + 2.0 * TABLE_CELL_PADDING[1];
                let table_height = (ui.content_region_avail()[1] - self.product_footer_height)
                    .max(60.0)
                    .min(content_height);
                if let Some(_table) = ui.begin_table_with_sizing(
                    "products",
                    5,
                    TableOptions::new()
                        .flags(
                            TableFlags::ROW_BG
                                | TableFlags::BORDERS_INNER_H
                                | TableFlags::RESIZABLE
                                | TableFlags::SCROLL_X
                                | TableFlags::SCROLL_Y
                                | TableFlags::SORTABLE,
                        )
                        .sizing_policy(TableSizingPolicy::FixedFit),
                    [table_width, table_height],
                    0.0,
                ) {
                    for (label, width) in ["File name", "Path", "Date", "Orbit", "Size"]
                        .into_iter()
                        .zip(column_widths)
                    {
                        ui.table_setup_column_fixed_width(label, TableColumnFlags::NONE, width);
                    }
                    ui.table_headers_row();
                    if let Some(mut specs) = ui.table_get_sort_specs() {
                        if let Some(spec) = specs.iter().next() {
                            let column = usize::from(spec.column_index);
                            let descending =
                                spec.sort_direction == dear_app::imgui::SortDirection::Descending;
                            if self.sort_column != column
                                || self.sort_descending != descending
                                || specs.is_dirty()
                            {
                                self.sort_column = column;
                                self.sort_descending = descending;
                                self.dirty = true;
                            }
                        }
                        specs.clear_dirty(ui);
                    }
                    self.rebuild_visible();
                    let mut clipper = ListClipper::new(self.filtered.len()).begin(ui);
                    while clipper.step() {
                        for row in clipper.display_range() {
                            let product = &self.sources[source_number].products[self.filtered[row]];
                            ui.table_next_row();
                            ui.table_next_column();
                            let selected =
                                self.selected_product.as_ref() == Some(&product.relative_path);
                            if selected {
                                let selected_bg = mix_color(product_surface, palette.accent, 0.24);
                                for column in 0..5 {
                                    ui.table_set_cell_bg_color(selected_bg, column);
                                }
                            }
                            if ui
                                .selectable_config(format!("{}##product_{row}", product.filename))
                                .selected(selected)
                                .build()
                            {
                                self.selected_product = Some(product.relative_path.clone());
                            }
                            ui.table_next_column();
                            ui.text(&product.relative_path);
                            ui.table_next_column();
                            ui.text(format!(
                                "{}-{}-{}",
                                product.year, product.month, product.day
                            ));
                            ui.table_next_column();
                            ui.text(&product.orbit);
                            ui.table_next_column();
                            ui.text(format!("{:.1} MB", product.size as f64 / 1_000_000.0));
                        }
                    }
                }
                let footer_start = ui.cursor_pos_y();
                ui.separator();
                ui.text("SELECTED PRODUCT");
                if let Some(relative) = &self.selected_product {
                    let path = source.root.join(relative);
                    ui.text_wrapped(path.display().to_string());
                    if let Some(result) = self.sources[source_number].overpass_results.get(relative)
                    {
                        match result {
                            Ok(groups) => ui.text_disabled(format!(
                                "Overpass: {}",
                                groups
                                    .iter()
                                    .map(|group| format!(
                                        "{} {}",
                                        group.name,
                                        if group.hit { "yes" } else { "no" }
                                    ))
                                    .collect::<Vec<_>>()
                                    .join(" · ")
                            )),
                            Err(error) => ui.text_colored(
                                [0.9, 0.3, 0.28, 1.0],
                                format!("Overpass check: {error}"),
                            ),
                        }
                    }
                    if ui.button("Open as A") {
                        open = Some((0, path.clone()));
                    }
                    ui.same_line();
                    if ui.button("Open as B") {
                        open = Some((1, path));
                    }
                    let selected = self.sources[source_number]
                        .products
                        .iter()
                        .find(|product| &product.relative_path == relative);
                    if selected.is_some_and(|product| {
                        product.year.is_empty()
                            || product.month.is_empty()
                            || product.day.is_empty()
                            || product.orbit.is_empty()
                    }) {
                        ui.same_line();
                        let pending = self.sources[source_number].metadata_job.is_some();
                        if pending {
                            ui.text("Reading metadata...");
                        } else if ui.button("Read product metadata") {
                            self.read_selected_metadata(source_number, source, relative.clone());
                        }
                    }
                    if let Some(status) = &self.sources[source_number].metadata_status {
                        ui.text_disabled(status);
                    }
                } else {
                    ui.text("Select a product to assign it to file A or B.");
                }
                self.product_footer_height = (ui.cursor_pos_y() - footer_start).max(1.0);
            });
        drop(_product_spacing);
        drop(_product_padding);
        open
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn registered_example_repositories_are_indexed() {
        let settings = Settings::load().unwrap();
        let gome = settings
            .dataset_sources
            .iter()
            .find(|source| source.id == "gome")
            .unwrap();
        assert_eq!(gome.format, "GOME Level 1B");
        assert!(source_matches_tags(gome, Some("gome"), "uvn"));
        assert!(!source_matches_tags(gome, Some("s5p"), ""));
        assert!(settings
            .dataset_sources
            .iter()
            .all(|source| source.id != "fdr_l1a" && source.id != "fdr_l1b"));
        assert!(settings
            .dataset_rules
            .iter()
            .any(|rule| rule.file_glob == "ESA_FDR_ATMOS__L1A_UVN__*.nc"));
        assert!(settings
            .dataset_rules
            .iter()
            .any(|rule| rule.file_glob == "ESA_FDR_ATMOS__L1B_UVN__*.nc"));
        for source in &settings.dataset_sources {
            if !source.root.is_dir() {
                continue;
            }
            let (sender, _receiver) = mpsc::channel();
            let products = scan_source(source, &sender, &AtomicBool::new(false)).unwrap();
            if products.is_empty() {
                continue;
            }
            let expected = match source.id.as_str() {
                "s5p" => 3,
                _ => 5,
            };
            assert_eq!(products.len(), expected, "{}", source.id);
            assert!(products.iter().all(|product| product.year.len() == 4
                && product.month.len() == 2
                && product.day.len() == 2));
            if source.id == "gome" {
                assert_eq!(products[0].orbit, "73861");
            }
            if source.id == "gome2" {
                assert_eq!(products[0].orbit, "32260");
            }
        }
    }

    #[test]
    fn large_index_filters_and_sorts_without_rendering_all_rows() {
        let mut browser = DatasetBrowser::new(&[]);
        browser.sources.push(SourceIndex {
            products: (0..70_001)
                .rev()
                .map(|number| Product {
                    filename: format!("{number:05}_gome.nc"),
                    relative_path: format!("2013/01/12/{number:05}_gome.nc"),
                    year: "2013".into(),
                    month: "01".into(),
                    day: "12".into(),
                    orbit: format!("{number:05}"),
                    size: number,
                })
                .collect(),
            error: None,
            scanning: false,
            visited: 0,
            receiver: None,
            cancel: None,
            available: None,
            availability_job: None,
            last_availability_check: None,
            metadata_job: None,
            metadata_status: None,
            overpass_area_key: None,
            overpass_running_area_key: None,
            overpass_results: HashMap::new(),
            overpass_groups: Vec::new(),
            overpass_cache_receiver: None,
            overpass_receiver: None,
            overpass_cancel: None,
            overpass_done: 0,
            overpass_total: 0,
            overpass_hits: 0,
            overpass_cancelled: false,
            overpass_error: None,
        });
        browser.rebuild_visible();
        assert_eq!(browser.filtered.len(), 70_001);
        assert_eq!(browser.filtered[0], 70_000);
        browser.search = "69999".into();
        browser.dirty = true;
        browser.rebuild_visible();
        assert_eq!(browser.filtered.len(), 1);
        browser.search.clear();
        browser.date_search = "2013-01-12".into();
        browser.orbit_search = "69999".into();
        browser.dirty = true;
        browser.rebuild_visible();
        assert_eq!(browser.filtered.len(), 1);
        browser.date_search = "2013/01/13".into();
        browser.dirty = true;
        browser.rebuild_visible();
        assert!(browser.filtered.is_empty());
        browser.date_search = "not a date".into();
        browser.dirty = true;
        browser.rebuild_visible();
        assert!(browser.filtered.is_empty());
    }

    #[test]
    #[ignore = "manual performance measurement"]
    fn benchmark_seventy_thousand_product_filter() {
        let mut browser = DatasetBrowser::new(&[]);
        browser.sources.push(SourceIndex {
            products: (0..70_000)
                .map(|number| Product {
                    filename: format!("{number:05}_gome.nc"),
                    relative_path: format!("2013/01/12/{number:05}_gome.nc"),
                    year: "2013".into(),
                    month: "01".into(),
                    day: "12".into(),
                    orbit: format!("{number:05}"),
                    size: number,
                })
                .collect(),
            error: None,
            scanning: false,
            visited: 0,
            receiver: None,
            cancel: None,
            available: None,
            availability_job: None,
            last_availability_check: None,
            metadata_job: None,
            metadata_status: None,
            overpass_area_key: None,
            overpass_running_area_key: None,
            overpass_results: HashMap::new(),
            overpass_groups: Vec::new(),
            overpass_cache_receiver: None,
            overpass_receiver: None,
            overpass_cancel: None,
            overpass_done: 0,
            overpass_total: 0,
            overpass_hits: 0,
            overpass_cancelled: false,
            overpass_error: None,
        });
        for (label, name, date, orbit) in [
            ("all", "", "", ""),
            ("name and date", "gome", "2013-01-12", ""),
            ("orbit", "", "", "69999"),
        ] {
            browser.search = name.into();
            browser.date_search = date.into();
            browser.orbit_search = orbit.into();
            let start = std::time::Instant::now();
            for _ in 0..20 {
                browser.dirty = true;
                browser.rebuild_visible();
                std::hint::black_box(browser.filtered.len());
            }
            eprintln!(
                "filter {label}: {:.2} ms/iteration",
                start.elapsed().as_secs_f64() * 50.0
            );
        }
    }

    #[test]
    fn browser_table_renders_with_a_registered_source() {
        use dear_app::imgui::{Condition, Context};
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([1200.0, 800.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let sources = vec![DatasetSource {
            id: "test".into(),
            name: "Test dataset".into(),
            root: PathBuf::from("/missing-dataset-browser-test"),
            format: "gome_l1b".into(),
            tags: vec!["test".into()],
            product_regex: "[.]nc$".into(),
            cache_path: PathBuf::from("/missing-cache-test.json"),
            overpass: OverpassDiscovery::default(),
        }];
        let mut browser = DatasetBrowser::new(&sources);
        let settings = Settings::load().unwrap();
        let ui = context.frame();
        ui.window("Dataset browser smoke")
            .size([1100.0, 700.0], Condition::Always)
            .build(|| {
                let areas = [OverpassArea {
                    key: "test".into(),
                    label: "Test area".into(),
                    bounds: [-1.0, -1.0, 1.0, 1.0],
                }];
                browser.draw(ui, &sources, &areas, None, &settings.palette, 640.0);
            });
    }

    #[test]
    fn cached_products_survive_a_missing_mount() {
        let base =
            std::env::temp_dir().join(format!("h5jive_dataset_mount_test_{}", std::process::id()));
        let root = base.join("mounted");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("2020_42.nc"), b"mock").unwrap();
        let source = DatasetSource {
            id: "mount_test".into(),
            name: "Mount test".into(),
            root: root.clone(),
            format: "gome_l1b".into(),
            tags: vec!["test".into()],
            product_regex: "^(?P<year>[0-9]{4})_(?P<orbit>[0-9]+)[.]nc$".into(),
            cache_path: base.join("index.json"),
            overpass: OverpassDiscovery::default(),
        };
        let (sender, _receiver) = mpsc::channel();
        let products = scan_source(&source, &sender, &AtomicBool::new(false)).unwrap();
        write_cache(&source, &products).unwrap();
        fs::remove_dir_all(root).unwrap();
        let browser = DatasetBrowser::new(&[source]);
        assert_eq!(browser.sources[0].products.len(), 1);
        assert!(!browser.sources[0].scanning);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn uncached_registration_waits_for_explicit_scan() {
        let base = std::env::temp_dir().join(format!(
            "h5jive_dataset_initial_scan_{}",
            std::process::id()
        ));
        let root = base.join("repository");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("product.nc"), b"mock").unwrap();
        let source = DatasetSource {
            id: "initial_scan_test".into(),
            name: "Initial scan".into(),
            root,
            format: "gome_l1b".into(),
            tags: vec!["test".into()],
            product_regex: "[.]nc$".into(),
            cache_path: base.join("index.json"),
            overpass: OverpassDiscovery::default(),
        };
        let mut browser = DatasetBrowser::new(&[source.clone()]);
        browser.poll(&[source.clone()]);
        assert!(!browser.sources[0].scanning);
        assert!(browser.sources[0].products.is_empty());
        assert!(!source.cache_path.exists());
        browser.refresh(0, &source);
        for _ in 0..100 {
            browser.poll(&[source.clone()]);
            if !browser.sources[0].scanning {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(browser.sources[0].products.len(), 1);
        assert!(source.cache_path.exists());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn product_metadata_can_fill_missing_date_and_orbit() {
        let settings = Settings::load().unwrap();
        let Some(source) = settings
            .dataset_sources
            .iter()
            .find(|source| source.id == "gome2")
        else {
            return;
        };
        let path = source.root.join("01/12/20130112080756Z_32344_gome.nc");
        if !path.exists() {
            return;
        }
        assert_eq!(
            read_product_metadata(&path).unwrap(),
            ["2013", "01", "12", "32344"]
        );
    }
}
