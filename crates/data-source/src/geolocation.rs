//! Discover CF-style auxiliary latitude and longitude variables for a dataset.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Geolocation {
    pub latitude: String,
    pub longitude: String,
    pub latitude_bounds: Option<String>,
    pub longitude_bounds: Option<String>,
    pub scan_direction: Option<ScanDirection>,
}

/// One per-pixel variable that distinguishes forward and backward scans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanDirection {
    pub path: String,
    pub forward_values: Vec<i64>,
    pub backward_values: Vec<i64>,
    pub default_filter: ScanFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanDirectionClass {
    Forward,
    Backward,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanFilter {
    Both,
    Forward,
    Backward,
}

impl ScanFilter {
    pub fn allows(self, direction: Option<ScanDirectionClass>) -> bool {
        match self {
            Self::Both => true,
            Self::Forward => direction.is_none() || direction == Some(ScanDirectionClass::Forward),
            Self::Backward => {
                direction.is_none() || direction == Some(ScanDirectionClass::Backward)
            }
        }
    }
}

#[cfg(test)]
mod scan_filter_tests {
    use super::{ScanDirectionClass, ScanFilter};

    #[test]
    fn datasets_without_scan_flags_remain_visible() {
        assert!(ScanFilter::Forward.allows(None));
        assert!(ScanFilter::Backward.allows(None));
        assert!(!ScanFilter::Forward.allows(Some(ScanDirectionClass::Backward)));
        assert!(!ScanFilter::Backward.allows(Some(ScanDirectionClass::Forward)));
    }
}

impl ScanDirection {
    pub fn classify(&self, value: f64, fill: Option<f64>) -> ScanDirectionClass {
        if !value.is_finite() || fill == Some(value) || value.fract() != 0.0 {
            return ScanDirectionClass::Other;
        }
        if self.forward_values.iter().any(|&code| code as f64 == value) {
            ScanDirectionClass::Forward
        } else if self
            .backward_values
            .iter()
            .any(|&code| code as f64 == value)
        {
            ScanDirectionClass::Backward
        } else {
            ScanDirectionClass::Other
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScanDirectionRule {
    pub group: String,
    pub variable: String,
    pub forward_values: Vec<i64>,
    pub backward_values: Vec<i64>,
    pub default_filter: ScanFilter,
}

#[derive(Debug, Clone)]
pub struct GeolocationRule {
    pub file_glob: String,
    pub platform: String,
    pub sensor: String,
    pub product_type: String,
    pub observation_group: String,
    pub coordinate_group: String,
    pub latitude: String,
    pub longitude: String,
    pub latitude_bounds: String,
    pub longitude_bounds: String,
    pub scan_direction: Option<ScanDirectionRule>,
}

impl File {
    pub fn geolocation_with_rules(
        &self,
        path: &str,
        rules: &[GeolocationRule],
    ) -> Result<Option<Geolocation>> {
        let standard = self.geolocation(path)?;
        let shape = self.dataset_info(path)?.shape;
        let Some(parent) = path.rsplit_once('/').map(|(parent, _)| parent) else {
            return Ok(standard);
        };
        let Some((base, group)) = parent.rsplit_once('/') else {
            return Ok(standard);
        };
        let root_attributes = self.attributes("/").unwrap_or_default();
        for rule in rules {
            if group != rule.observation_group || !rule.matches(&self.name, &root_attributes) {
                continue;
            }
            let scan_direction = rule.scan_direction.as_ref().and_then(|config| {
                let group = match config.group.as_str() {
                    "observation" => &rule.observation_group,
                    "coordinate" => &rule.coordinate_group,
                    _ => return None,
                };
                let path = format!("{base}/{group}/{}", config.variable);
                self.dataset_info(&path)
                    .ok()
                    .filter(|info| info.numeric && compatible_shape(&info.shape, &shape))?;
                Some(ScanDirection {
                    path,
                    forward_values: config.forward_values.clone(),
                    backward_values: config.backward_values.clone(),
                    default_filter: config.default_filter,
                })
            });
            if let Some(mut geolocation) = standard.clone() {
                geolocation.scan_direction = scan_direction;
                return Ok(Some(geolocation));
            }
            let coordinate_path = |name: &str| format!("{base}/{}/{name}", rule.coordinate_group);
            let latitude = coordinate_path(&rule.latitude);
            let longitude = coordinate_path(&rule.longitude);
            let valid_coordinate = |candidate: &str| {
                self.dataset_info(candidate)
                    .ok()
                    .is_some_and(|info| info.numeric && compatible_shape(&info.shape, &shape))
            };
            if !valid_coordinate(&latitude) || !valid_coordinate(&longitude) {
                continue;
            }
            let bounds = |name: &str, coordinate: &str| {
                let candidate = coordinate_path(name);
                let coordinate_shape = self.dataset_info(coordinate).ok()?.shape;
                let info = self.dataset_info(&candidate).ok()?;
                (info.numeric
                    && info.shape.len() == coordinate_shape.len() + 1
                    && info.shape.starts_with(&coordinate_shape)
                    && info.shape.last() == Some(&4))
                .then_some(candidate)
            };
            return Ok(Some(Geolocation {
                latitude_bounds: bounds(&rule.latitude_bounds, &latitude),
                longitude_bounds: bounds(&rule.longitude_bounds, &longitude),
                latitude,
                longitude,
                scan_direction,
            }));
        }
        Ok(standard)
    }

    pub fn geolocation(&self, path: &str) -> Result<Option<Geolocation>> {
        let shape = self.dataset_info(path)?.shape;
        if !matches!(shape.len(), 2..=4) {
            return Ok(None);
        }
        let attributes = self.attributes(path)?;
        let names: Vec<String> = attributes
            .iter()
            .find(|attribute| attribute.name == "coordinates")
            .map(|attribute| {
                attribute
                    .value
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_else(|| {
                ["latitude", "longitude", "lat", "lon"]
                    .map(str::to_owned)
                    .into()
            });
        let mut latitude = None;
        let mut longitude = None;
        for name in names {
            let Some(candidate) = resolve_variable(self, path, &name) else {
                continue;
            };
            let Ok(info) = self.dataset_info(&candidate) else {
                continue;
            };
            if !info.numeric || !compatible_shape(&info.shape, &shape) {
                continue;
            }
            let role = coordinate_role(&name, &self.attributes(&candidate).unwrap_or_default());
            match role {
                Some("latitude") if latitude.is_none() => latitude = Some(candidate),
                Some("longitude") if longitude.is_none() => longitude = Some(candidate),
                _ => {}
            }
        }
        Ok(latitude.zip(longitude).and_then(|(latitude, longitude)| {
            // Coordinate variables are not observations to plot on the map.
            if latitude.trim_start_matches('/') == path.trim_start_matches('/')
                || longitude.trim_start_matches('/') == path.trim_start_matches('/')
            {
                None
            } else {
                let bounds = |coordinate: &str, fallback_name: &str| {
                    let bounds_name = self
                        .attributes(coordinate)
                        .ok()?
                        .into_iter()
                        .find(|attribute| attribute.name == "bounds")
                        .map(|attribute| attribute.value.trim().to_owned());
                    let candidate = bounds_name
                        .and_then(|name| resolve_variable(self, coordinate, &name))
                        .or_else(|| resolve_variable(self, coordinate, fallback_name))?;
                    let coordinate_shape = self.dataset_info(coordinate).ok()?.shape;
                    let bounds_info = self.dataset_info(&candidate).ok()?;
                    (bounds_info.numeric
                        && bounds_info.shape.starts_with(&coordinate_shape)
                        && bounds_info.shape.get(coordinate_shape.len()) == Some(&4)
                        && bounds_info.shape.len() == coordinate_shape.len() + 1)
                        .then_some(candidate)
                };
                let latitude_bounds = bounds(&latitude, "latitude_bounds");
                let longitude_bounds = bounds(&longitude, "longitude_bounds");
                Some(Geolocation {
                    latitude,
                    longitude,
                    latitude_bounds,
                    longitude_bounds,
                    scan_direction: None,
                })
            }
        }))
    }
}

impl GeolocationRule {
    fn matches(&self, filename: &str, attributes: &[AttributeValue]) -> bool {
        let mut matched = 0;
        for (name, pattern) in [
            ("platform", &self.platform),
            ("sensor", &self.sensor),
            ("product_type", &self.product_type),
        ] {
            if let Some(attribute) = attributes.iter().find(|attribute| attribute.name == name) {
                if !glob_matches(pattern, &attribute.value) {
                    return false;
                }
                matched += 1;
            }
        }
        matched >= 2 || glob_matches(&self.file_glob, filename)
    }
}

fn resolve_variable(file: &File, source: &str, name: &str) -> Option<String> {
    let name = name.trim();
    let parent = source.rsplit_once('/').map_or("", |(parent, _)| parent);
    let mut candidates = Vec::new();
    if name.starts_with('/') {
        candidates.push(name.to_owned());
    } else if name.contains('/') {
        let mut parts: Vec<&str> = parent.split('/').filter(|part| !part.is_empty()).collect();
        for part in name.split('/') {
            match part {
                ".." => {
                    parts.pop();
                }
                "." | "" => {}
                _ => parts.push(part),
            }
        }
        candidates.push(format!("/{}", parts.join("/")));
    } else {
        candidates.push(format!("{parent}/{name}"));
        let mut ancestor = parent;
        while let Some((next, _)) = ancestor.rsplit_once('/') {
            candidates.push(format!("{next}/{name}"));
            ancestor = next;
            if ancestor.is_empty() {
                break;
            }
        }
        candidates.push(format!("/coordinates/{name}"));
    }
    candidates
        .into_iter()
        .find(|candidate| file.dataset_info(candidate).is_ok())
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut previous = vec![false; value.len() + 1];
    previous[0] = true;
    for &part in pattern {
        let mut current = vec![false; value.len() + 1];
        if part == b'*' {
            current[0] = previous[0];
            for index in 1..=value.len() {
                current[index] = previous[index] || current[index - 1];
            }
        } else {
            for index in 1..=value.len() {
                current[index] = previous[index - 1] && (part == b'?' || part == value[index - 1]);
            }
        }
        previous = current;
    }
    previous[value.len()]
}

fn compatible_shape(coordinate: &[usize], values: &[usize]) -> bool {
    coordinate == values
        || (values.len() == 3 && coordinate == &values[1..])
        || (values.len() == 4 && (coordinate == &values[..3] || coordinate == &values[1..3]))
}

fn coordinate_role<'a>(name: &str, attributes: &[AttributeValue]) -> Option<&'a str> {
    let basename = name.rsplit('/').next().unwrap_or(name).to_ascii_lowercase();
    let standard_name = attributes
        .iter()
        .find(|attribute| attribute.name == "standard_name")
        .map(|attribute| attribute.value.to_ascii_lowercase());
    let units = attributes
        .iter()
        .find(|attribute| attribute.name == "units")
        .map(|attribute| attribute.value.to_ascii_lowercase());
    if basename == "lat"
        || basename.contains("latitude")
        || standard_name.as_deref() == Some("latitude")
        || units
            .as_deref()
            .is_some_and(|value| value.contains("degrees_north"))
    {
        Some("latitude")
    } else if basename == "lon"
        || basename.contains("longitude")
        || standard_name.as_deref() == Some("longitude")
        || units
            .as_deref()
            .is_some_and(|value| value.contains("degrees_east"))
    {
        Some("longitude")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auxiliary_coordinates_may_share_all_axes_or_only_spatial_axes() {
        assert!(compatible_shape(&[8, 32, 48], &[8, 32, 48]));
        assert!(compatible_shape(&[32, 48], &[8, 32, 48]));
        assert!(!compatible_shape(&[8, 32], &[8, 32, 48]));
        assert!(!compatible_shape(&[32], &[32, 48]));
        assert!(compatible_shape(&[1, 219, 3], &[1, 219, 3, 1024]));
        assert!(compatible_shape(&[219, 3], &[1, 219, 3, 1024]));
        assert!(!compatible_shape(&[219, 1024], &[1, 219, 3, 1024]));
    }

    #[test]
    fn filename_glob_matches_only_selected_products() {
        assert!(glob_matches(
            "ER2_RPRO_GOM_L1B*.nc",
            "ER2_RPRO_GOM_L1B_foo.nc"
        ));
        assert!(!glob_matches("ER2_RPRO_GOM_L1B*.nc", "SCI_L1B_foo.nc"));
    }

    #[test]
    fn gome_l1b_rule_finds_sibling_geodata_when_fixture_is_available() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../PROPOSALS/ER2_RPRO_GOM_L1B____20090606T091303_20090606T093454_73866_02_051000_20171108T192200.nc",
        );
        if !fixture.exists() {
            return;
        }
        let file = File::open(&fixture).unwrap();
        let path = "/MODE_NADIR/BAND_3/OBSERVATIONS/radiance";
        let rule = GeolocationRule {
            file_glob: "ER2_RPRO_GOM_L1B*.nc".into(),
            platform: "ERS-2".into(),
            sensor: "GOME".into(),
            product_type: "GOM_L1B___".into(),
            observation_group: "OBSERVATIONS".into(),
            coordinate_group: "GEODATA".into(),
            latitude: "latitude".into(),
            longitude: "longitude".into(),
            latitude_bounds: "latitude_bounds".into(),
            longitude_bounds: "longitude_bounds".into(),
            scan_direction: None,
        };
        let geo = file.geolocation_with_rules(path, &[rule]).unwrap().unwrap();
        assert_eq!(geo.latitude, "/MODE_NADIR/BAND_3/GEODATA/latitude");
        assert_eq!(geo.longitude, "/MODE_NADIR/BAND_3/GEODATA/longitude");
        assert_eq!(
            geo.latitude_bounds.as_deref(),
            Some("/MODE_NADIR/BAND_3/GEODATA/latitude_bounds")
        );
        assert_eq!(
            geo.longitude_bounds.as_deref(),
            Some("/MODE_NADIR/BAND_3/GEODATA/longitude_bounds")
        );
        let axes = file.coordinate_axes(path).unwrap();
        assert_eq!(
            axes.iter()
                .map(|axis| axis.name.as_str())
                .collect::<Vec<_>>(),
            ["time", "scanline", "ground_pixel", "spectral_channel"]
        );
        assert_eq!(
            axes[0].variable.as_deref(),
            Some("/MODE_NADIR/BAND_3/OBSERVATIONS/time")
        );
        assert_eq!(
            axes[1].variable.as_deref(),
            Some("/MODE_NADIR/BAND_3/OBSERVATIONS/scanline")
        );
        assert_eq!(axes[1].last, Some(218.0));
        assert!(axes[2].variable.is_none());
        assert!(axes[3].variable.is_none());
    }

    #[test]
    fn format_metadata_takes_priority_over_filename() {
        let rule = GeolocationRule {
            file_glob: "*_gome.nc".into(),
            platform: "MetOp M0?".into(),
            sensor: "GOME-2".into(),
            product_type: "GO2_L1B___".into(),
            observation_group: "OBSERVATIONS".into(),
            coordinate_group: "GEODATA".into(),
            latitude: "latitude".into(),
            longitude: "longitude".into(),
            latitude_bounds: "latitude_bounds".into(),
            longitude_bounds: "longitude_bounds".into(),
            scan_direction: None,
        };
        let attrs = |platform: &str, product_type: &str| {
            vec![
                AttributeValue {
                    name: "platform".into(),
                    value: platform.into(),
                    float_values: None,
                },
                AttributeValue {
                    name: "sensor".into(),
                    value: "GOME-2".into(),
                    float_values: None,
                },
                AttributeValue {
                    name: "product_type".into(),
                    value: product_type.into(),
                    float_values: None,
                },
            ]
        };
        assert!(rule.matches("other_name.nc", &attrs("MetOp M02", "GO2_L1B___")));
        assert!(rule.matches("other_name.nc", &attrs("MetOp M01", "GO2_L1B___")));
        assert!(rule.matches("other_name.nc", &attrs("MetOp M03", "GO2_L1B___")));
        assert!(!rule.matches("20130101_gome.nc", &attrs("MetOp M99", "GO2_L1B___")));
        assert!(!rule.matches("20130101_gome.nc", &attrs("MetOp M02", "OTHER")));
        let mut partial = attrs("MetOp M02", "GO2_L1B___");
        partial.remove(0);
        assert!(rule.matches("other_name.nc", &partial));
        partial.remove(0);
        assert!(!rule.matches("other_name.nc", &partial));
        assert!(rule.matches("20130101_gome.nc", &[]));
        assert!(!rule.matches("other_name.nc", &[]));
    }
}
