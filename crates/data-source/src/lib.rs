//! Read-only HDF5/NetCDF-4 access through `hdf5-metno`, plus classic NetCDF through `netcdf-reader`.
use hdf5_metno::{
    types::{FixedAscii, FixedUnicode, TypeDescriptor, VarLenArray, VarLenAscii, VarLenUnicode},
    Attribute as H5Attribute, Dataset as H5Dataset, File as H5File, Group, Hyperslab, LocationType,
    ObjectReference1, ReferencedObject, SliceOrIndex,
};
use ndarray::IxDyn;
use netcdf_reader::{NcAttrValue, NcFile, NcSliceInfo, NcSliceInfoElem, NcType};
use std::{cell::RefCell, error::Error, fmt, path::Path};
mod geolocation;
mod statistics;
pub use geolocation::{
    Geolocation, GeolocationRule, ScanDirection, ScanDirectionClass, ScanDirectionRule, ScanFilter,
};
pub use statistics::{DatasetDistribution, DatasetStatistics};

#[derive(Debug)]
pub struct DataError(pub String);
impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for DataError {}
type Result<T> = std::result::Result<T, DataError>;
fn error(value: impl fmt::Display) -> DataError {
    DataError(value.to_string())
}

#[derive(Debug, Clone)]
pub enum Kind {
    Group,
    Dataset { shape: Vec<usize>, numeric: bool },
    Other,
}
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: Kind,
    pub children: Vec<Entry>,
}
#[derive(Debug)]
pub struct Values {
    pub data: Vec<f64>,
    pub fill_value: Option<f64>,
}
#[derive(Debug)]
pub struct Region {
    pub data: Vec<f64>,
    pub shape: Vec<usize>,
    pub fill_value: Option<f64>,
}
#[derive(Debug, Clone)]
pub struct DatasetInfo {
    pub shape: Vec<usize>,
    pub elements: u128,
    pub byte_size: u128,
    pub datatype: String,
    pub numeric: bool,
}
#[derive(Debug, Clone)]
pub struct AttributeValue {
    pub name: String,
    pub value: String,
    pub float_values: Option<Vec<f64>>,
}
#[derive(Debug, Clone)]
pub struct CoordinateAxis {
    pub name: String,
    pub length: usize,
    pub variable: Option<String>,
    pub units: Option<String>,
    pub first: Option<f64>,
    pub last: Option<f64>,
}

enum Backend {
    Hdf5(H5File),
    ClassicNetcdf(NcFile),
}
pub struct File {
    inner: Backend,
    region_datasets: RefCell<Vec<(String, H5Dataset)>>,
    netcdf_metadata: Option<NcFile>,
    pub name: String,
    pub tree: Entry,
}
#[derive(Debug, Clone, Copy)]
pub struct OpenOptions {
    pub max_depth: usize,
    pub max_objects: usize,
}
impl Default for OpenOptions {
    fn default() -> Self {
        Self {
            max_depth: 24,
            max_objects: 10_000,
        }
    }
}
impl File {
    fn region_h5_dataset(&self, file: &H5File, path: &str) -> Result<H5Dataset> {
        let mut cached = self.region_datasets.borrow_mut();
        if let Some(index) = cached.iter().position(|(name, _)| name == path) {
            let entry = cached.remove(index);
            let dataset = entry.1.clone();
            cached.push(entry);
            return Ok(dataset);
        }
        let dataset = file.dataset(path).map_err(error)?;
        if cached.len() == 6 {
            cached.remove(0);
        }
        cached.push((path.to_owned(), dataset.clone()));
        Ok(dataset)
    }
    pub fn statistics(&self, path: &str, sample_limit: usize) -> Result<DatasetStatistics> {
        statistics::calculate(self, path, sample_limit)
    }
    pub fn distribution(
        &self,
        path: &str,
        statistics: &DatasetStatistics,
        bin_count: usize,
    ) -> Result<DatasetDistribution> {
        statistics::distribution(self, path, statistics, bin_count)
    }
    pub fn dataset_info(&self, path: &str) -> Result<DatasetInfo> {
        let (shape, element_size, datatype, numeric) = match &self.inner {
            Backend::Hdf5(file) => {
                let dataset = file.dataset(path).map_err(error)?;
                let datatype = dataset.dtype().map_err(error)?;
                (
                    dataset.shape(),
                    datatype.size(),
                    datatype.to_descriptor().map_err(error)?.to_string(),
                    datatype.conv_to::<f64>().is_some(),
                )
            }
            Backend::ClassicNetcdf(file) => {
                let variable = file.variable(path.trim_start_matches('/')).map_err(error)?;
                let datatype = variable.dtype();
                (
                    variable
                        .shape()
                        .iter()
                        .map(|&length| length as usize)
                        .collect(),
                    datatype.size().map_err(error)?,
                    format!("{datatype:?}"),
                    netcdf_type_is_numeric(datatype),
                )
            }
        };
        let elements = shape
            .iter()
            .try_fold(1_u128, |total, &length| total.checked_mul(length as u128))
            .ok_or_else(|| error("dataset element count overflow"))?;
        let byte_size = elements
            .checked_mul(element_size as u128)
            .ok_or_else(|| error("dataset byte size overflow"))?;
        Ok(DatasetInfo {
            shape,
            elements,
            byte_size,
            datatype,
            numeric,
        })
    }

    pub fn attributes(&self, path: &str) -> Result<Vec<AttributeValue>> {
        match &self.inner {
            Backend::Hdf5(file) => {
                if let Ok(group) = file.group(path) {
                    read_h5_attributes(group.attr_names().map_err(error)?, |name| group.attr(name))
                } else {
                    let dataset = file.dataset(path).map_err(error)?;
                    let mut attributes =
                        read_h5_attributes(dataset.attr_names().map_err(error)?, |name| {
                            dataset.attr(name)
                        })?;
                    if let Some(dimension_list) = attributes
                        .iter_mut()
                        .find(|attribute| attribute.name == "DIMENSION_LIST")
                    {
                        let names = self.axis_names(path)?;
                        if !names.iter().any(|name| name.starts_with("Dimension ")) {
                            dimension_list.value = names.join(", ");
                        }
                    }
                    Ok(attributes)
                }
            }
            Backend::ClassicNetcdf(file) => {
                if path == "/" {
                    return Ok(Vec::new());
                }
                let variable = file.variable(path.trim_start_matches('/')).map_err(error)?;
                Ok(variable
                    .attributes()
                    .iter()
                    .map(|attribute| AttributeValue {
                        name: attribute.name.clone(),
                        value: format_netcdf_attribute(&attribute.value),
                        float_values: match &attribute.value {
                            NcAttrValue::Floats(values) => {
                                Some(values.iter().map(|&value| value as f64).collect())
                            }
                            NcAttrValue::Doubles(values) => Some(values.clone()),
                            _ => None,
                        },
                    })
                    .collect())
            }
        }
    }
    pub fn axis_names(&self, path: &str) -> Result<Vec<String>> {
        match &self.inner {
            Backend::Hdf5(file) => {
                let dataset = file.dataset(path).map_err(error)?;
                let count = dataset.ndim();
                if let Some(netcdf) = &self.netcdf_metadata {
                    if let Ok(variable) = netcdf.variable(path.trim_start_matches('/')) {
                        let names: Vec<String> = variable
                            .dimensions
                            .iter()
                            .map(|dimension| dimension.name.clone())
                            .collect();
                        if names.len() == count {
                            return Ok(names);
                        }
                    }
                }
                if let Ok(references) = dataset
                    .attr("DIMENSION_LIST")
                    .and_then(|attribute| attribute.read_raw::<VarLenArray<ObjectReference1>>())
                {
                    let names: Vec<String> = references
                        .iter()
                        .filter_map(|references| references.as_slice().first())
                        .filter_map(|reference| match dataset.dereference(reference).ok()? {
                            ReferencedObject::Dataset(scale) => {
                                Some(scale.name().rsplit('/').next()?.to_owned())
                            }
                            _ => None,
                        })
                        .collect();
                    if names.len() == count {
                        return Ok(names);
                    }
                }
                let names = dataset
                    .attr("dimensions")
                    .or_else(|_| dataset.attr("coordinates"))
                    .ok()
                    .and_then(|attribute| {
                        attribute
                            .read_scalar::<VarLenUnicode>()
                            .ok()
                            .map(|value| value.as_str().to_owned())
                            .or_else(|| {
                                attribute
                                    .read_scalar::<VarLenAscii>()
                                    .ok()
                                    .map(|value| value.as_str().to_owned())
                            })
                    });
                let names: Vec<String> = names
                    .unwrap_or_default()
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect();
                if names.len() == count {
                    Ok(names)
                } else {
                    Ok((0..count).map(|axis| format!("Dimension {axis}")).collect())
                }
            }
            Backend::ClassicNetcdf(file) => Ok(file
                .variable(path.trim_start_matches('/'))
                .map_err(error)?
                .dimensions
                .iter()
                .map(|dimension| dimension.name.clone())
                .collect()),
        }
    }
    pub fn coordinate_axes(&self, path: &str) -> Result<Vec<CoordinateAxis>> {
        let shape = self.dataset_info(path)?.shape;
        let names = self.axis_names(path)?;
        Ok(names
            .into_iter()
            .zip(shape)
            .map(|(name, length)| {
                let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
                let mut candidates = vec![format!("{parent}/{name}")];
                let mut ancestor = parent;
                while let Some((next, _)) = ancestor.rsplit_once('/') {
                    candidates.push(format!("{next}/{name}"));
                    ancestor = next;
                    if ancestor.is_empty() {
                        break;
                    }
                }
                candidates.push(format!("/coordinates/{name}"));
                let variable = candidates.into_iter().find(|candidate| {
                    let Some(Entry {
                        kind:
                            Kind::Dataset {
                                shape,
                                numeric: true,
                            },
                        ..
                    }) = find_entry(&self.tree, candidate)
                    else {
                        return false;
                    };
                    if shape.as_slice() != [length] && shape.as_slice() != [1, length] {
                        return false;
                    }
                    !self.attributes(candidate).ok().is_some_and(|attributes| {
                        attributes.iter().any(|attribute| {
                            attribute.name == "NAME"
                                && attribute.value.starts_with(
                                    "This is a netCDF dimension but not a netCDF variable",
                                )
                        })
                    })
                });
                let (units, first, last) = if let Some(variable) = &variable {
                    let units = self.attributes(variable).ok().and_then(|attributes| {
                        attributes
                            .into_iter()
                            .find(|attribute| attribute.name == "units")
                            .map(|attribute| attribute.value)
                    });
                    let variable_rank = self
                        .dataset_info(variable)
                        .ok()
                        .map_or(1, |info| info.shape.len());
                    let sample = |index| {
                        let (start, count) = if variable_rank == 2 {
                            (vec![0, index], vec![1, 1])
                        } else {
                            (vec![index], vec![1])
                        };
                        self.region(variable, &start, &count)
                            .ok()
                            .and_then(|region| region.data.into_iter().next())
                    };
                    let first = (length > 0).then(|| sample(0)).flatten();
                    let last = (length > 0).then(|| sample(length - 1)).flatten();
                    (units, first, last)
                } else {
                    (None, None, None)
                };
                CoordinateAxis {
                    name,
                    length,
                    variable,
                    units,
                    first,
                    last,
                }
            })
            .collect())
    }
    /// Read only the requested rectangular region; all axes retain their rank.
    pub fn region(&self, path: &str, start: &[usize], count: &[usize]) -> Result<Region> {
        if start.len() != count.len() || count.is_empty() || count.iter().any(|&n| n == 0) {
            return Err(error("invalid region dimensions"));
        }
        let shape = match &self.inner {
            Backend::Hdf5(file) => self.region_h5_dataset(file, path)?.shape(),
            Backend::ClassicNetcdf(file) => file
                .variable(path.trim_start_matches('/'))
                .map_err(error)?
                .shape()
                .iter()
                .map(|&n| n as usize)
                .collect(),
        };
        if shape.len() != count.len()
            || start
                .iter()
                .zip(count)
                .zip(&shape)
                .any(|((&first, &length), &total)| first >= total || length > total - first)
            || count
                .iter()
                .try_fold(1usize, |total, &n| total.checked_mul(n))
                .is_none_or(|n| n > 1_000_000)
        {
            return Err(error("region exceeds dataset bounds or 1,000,000 values"));
        }
        let (data, fill_value) = match &self.inner {
            Backend::Hdf5(file) => {
                let slices: Vec<SliceOrIndex> = start
                    .iter()
                    .zip(count)
                    .map(|(&first, &length)| (first..first + length).into())
                    .collect();
                let dataset = self.region_h5_dataset(file, path)?;
                let fill_value = dataset
                    .attr("_FillValue")
                    .ok()
                    .and_then(|attr| read_h5_fill_value(&attr));
                let data = dataset
                    .read_slice::<f64, _, IxDyn>(Hyperslab::from(slices))
                    .map_err(error)?
                    .into_raw_vec_and_offset()
                    .0;
                (data, fill_value)
            }
            Backend::ClassicNetcdf(file) => {
                let fill_value = file
                    .variable(path.trim_start_matches('/'))
                    .map_err(error)?
                    .attribute("_FillValue")
                    .and_then(|attribute| attribute.value.as_f64());
                let selections = start
                    .iter()
                    .zip(count)
                    .map(|(&first, &length)| NcSliceInfoElem::Slice {
                        start: first as u64,
                        end: (first + length) as u64,
                        step: 1,
                    })
                    .collect();
                let data = file
                    .read_variable_slice_as_f64(
                        path.trim_start_matches('/'),
                        &NcSliceInfo { selections },
                    )
                    .map_err(error)?
                    .into_raw_vec_and_offset()
                    .0;
                (data, fill_value)
            }
        };
        Ok(Region {
            data,
            shape: count.to_vec(),
            fill_value,
        })
    }
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_options(path, OpenOptions::default())
    }
    pub fn open_with_options(path: &Path, options: OpenOptions) -> Result<Self> {
        if options.max_depth == 0 || options.max_objects == 0 {
            return Err(error("tree limits must be positive"));
        }
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        // Keep enough decoded chunks for repeated slices through multi-time products.
        let mut hdf5_builder = H5File::with_options();
        hdf5_builder.with_fapl(|fapl| fapl.chunk_cache(521, 16 * 1024 * 1024, 0.75));
        let (inner, netcdf_metadata, children) = if let Ok(hdf5) = hdf5_builder.open(path) {
            let root = hdf5.group("/").map_err(error)?;
            let netcdf_metadata = if root.attr("_NCProperties").is_ok() {
                NcFile::open(path).ok()
            } else {
                None
            };
            let mut count = 0;
            let children = children(&root, "/", 0, &mut count, options)?;
            (Backend::Hdf5(hdf5), netcdf_metadata, children)
        } else {
            let nc = NcFile::open(path).map_err(error)?;
            let mut entries: Vec<Entry> = nc
                .variables()
                .map_err(error)?
                .iter()
                .map(|variable| Entry {
                    name: variable.name().to_owned(),
                    path: format!("/{}", variable.name()),
                    kind: Kind::Dataset {
                        shape: variable.shape().iter().map(|n| *n as usize).collect(),
                        numeric: netcdf_type_is_numeric(variable.dtype()),
                    },
                    children: vec![],
                })
                .collect();
            entries.sort_by(|a, b| {
                let group_rank = |e: &Entry| {
                    if matches!(e.kind, Kind::Group) {
                        0u8
                    } else {
                        1u8
                    }
                };
                group_rank(a)
                    .cmp(&group_rank(b))
                    .then_with(|| a.name.cmp(&b.name))
            });
            (Backend::ClassicNetcdf(nc), None, entries)
        };
        Ok(Self {
            inner,
            region_datasets: RefCell::new(Vec::new()),
            netcdf_metadata,
            name,
            tree: Entry {
                name: "/".into(),
                path: "/".into(),
                kind: Kind::Group,
                children,
            },
        })
    }
    pub fn values(&self, path: &str, limit: usize) -> Result<Values> {
        match &self.inner {
            Backend::Hdf5(file) => {
                let dataset = file.dataset(path).map_err(error)?;
                if dataset.size() > limit {
                    return Err(error(format!(
                        "dataset has {} values; limit is {limit}",
                        dataset.size()
                    )));
                }
                let fill_value = dataset
                    .attr("_FillValue")
                    .ok()
                    .and_then(|attr| read_h5_fill_value(&attr));
                Ok(Values {
                    data: dataset.read_raw::<f64>().map_err(error)?,
                    fill_value,
                })
            }
            Backend::ClassicNetcdf(file) => {
                let name = path.trim_start_matches('/');
                let variable = file.variable(name).map_err(error)?;
                let count = variable
                    .shape()
                    .iter()
                    .try_fold(1usize, |value, dimension| {
                        value.checked_mul(*dimension as usize)
                    })
                    .ok_or_else(|| error("variable size overflow"))?;
                if count > limit {
                    return Err(error(format!(
                        "variable has {count} values; limit is {limit}"
                    )));
                }
                let array = file.read_variable_as_f64(name).map_err(error)?;
                Ok(Values {
                    data: array.iter().copied().collect(),
                    fill_value: None,
                })
            }
        }
    }
}
fn find_entry<'a>(root: &'a Entry, path: &str) -> Option<&'a Entry> {
    if root.path == path {
        return Some(root);
    }
    root.children
        .iter()
        .find_map(|child| find_entry(child, path))
}
fn netcdf_type_is_numeric(datatype: &NcType) -> bool {
    matches!(
        datatype,
        NcType::Byte
            | NcType::Short
            | NcType::Int
            | NcType::Float
            | NcType::Double
            | NcType::UByte
            | NcType::UShort
            | NcType::UInt
            | NcType::Int64
            | NcType::UInt64
    )
}
fn read_h5_fill_value(attribute: &H5Attribute) -> Option<f64> {
    if attribute.size() == 1 {
        attribute.read_raw::<f64>().ok()?.into_iter().next()
    } else {
        None
    }
}

fn read_h5_attributes(
    names: Vec<String>,
    mut open: impl FnMut(&str) -> hdf5_metno::Result<H5Attribute>,
) -> Result<Vec<AttributeValue>> {
    names
        .into_iter()
        .map(|name| {
            let attribute = open(&name).map_err(error)?;
            let datatype = attribute.dtype().map_err(error)?;
            let descriptor = datatype.to_descriptor().map_err(error)?;
            let float_values = if matches!(&descriptor, TypeDescriptor::Float(_)) {
                attribute.read_raw::<f64>().ok()
            } else {
                None
            };
            let value = if let Some(values) = &float_values {
                format_numbers(values)
            } else if let TypeDescriptor::FixedAscii(length) = descriptor {
                if length <= 256 {
                    attribute
                        .read_raw::<FixedAscii<256>>()
                        .ok()
                        .map(|values| format_strings(values.iter().map(|value| value.as_str())))
                        .unwrap_or_else(|| format!("<{descriptor}; shape {:?}>", attribute.shape()))
                } else {
                    format!("<{descriptor}; shape {:?}>", attribute.shape())
                }
            } else if let TypeDescriptor::FixedUnicode(length) = descriptor {
                if length <= 256 {
                    attribute
                        .read_raw::<FixedUnicode<256>>()
                        .ok()
                        .map(|values| format_strings(values.iter().map(|value| value.as_str())))
                        .unwrap_or_else(|| format!("<{descriptor}; shape {:?}>", attribute.shape()))
                } else {
                    format!("<{descriptor}; shape {:?}>", attribute.shape())
                }
            } else if matches!(descriptor, TypeDescriptor::Integer(_)) {
                attribute
                    .read_raw::<i64>()
                    .ok()
                    .map(|values| format_values(&values))
                    .unwrap_or_else(|| format!("<{descriptor}; shape {:?}>", attribute.shape()))
            } else if matches!(descriptor, TypeDescriptor::Unsigned(_)) {
                attribute
                    .read_raw::<u64>()
                    .ok()
                    .map(|values| format_values(&values))
                    .unwrap_or_else(|| format!("<{descriptor}; shape {:?}>", attribute.shape()))
            } else if matches!(descriptor, TypeDescriptor::Boolean) {
                attribute
                    .read_raw::<bool>()
                    .ok()
                    .map(|values| format_values(&values))
                    .unwrap_or_else(|| format!("<{descriptor}; shape {:?}>", attribute.shape()))
            } else if let Ok(values) = attribute.read_raw::<VarLenUnicode>() {
                format_strings(values.iter().map(|value| value.as_str()))
            } else if let Ok(values) = attribute.read_raw::<VarLenAscii>() {
                format_strings(values.iter().map(|value| value.as_str()))
            } else if let Ok(values) = attribute.read_raw::<f64>() {
                format_numbers(&values)
            } else {
                format!("<{descriptor}; shape {:?}>", attribute.shape())
            };
            Ok(AttributeValue {
                name,
                value,
                float_values,
            })
        })
        .collect()
}
fn format_strings<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let values: Vec<&str> = values.take(65).collect();
    if values.len() == 1 {
        values[0].to_owned()
    } else {
        format!(
            "[{}{}]",
            values
                .iter()
                .take(64)
                .copied()
                .collect::<Vec<_>>()
                .join(", "),
            if values.len() > 64 { ", …" } else { "" }
        )
    }
}
fn format_numbers(values: &[f64]) -> String {
    format_values(values)
}
fn format_values<T: ToString>(values: &[T]) -> String {
    if values.len() == 1 {
        values[0].to_string()
    } else {
        format!(
            "[{}{}]",
            values
                .iter()
                .take(64)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            if values.len() > 64 { ", …" } else { "" }
        )
    }
}
fn format_netcdf_attribute(value: &NcAttrValue) -> String {
    match value {
        NcAttrValue::Chars(text) => text.clone(),
        NcAttrValue::Strings(values) => format_values(values),
        NcAttrValue::Bytes(values) => format_values(values),
        NcAttrValue::Shorts(values) => format_values(values),
        NcAttrValue::Ints(values) => format_values(values),
        NcAttrValue::Floats(values) => format_values(values),
        NcAttrValue::Doubles(values) => format_values(values),
        NcAttrValue::UBytes(values) => format_values(values),
        NcAttrValue::UShorts(values) => format_values(values),
        NcAttrValue::UInts(values) => format_values(values),
        NcAttrValue::Int64s(values) => format_values(values),
        NcAttrValue::UInt64s(values) => format_values(values),
    }
}
fn children(
    group: &Group,
    path: &str,
    depth: usize,
    count: &mut usize,
    options: OpenOptions,
) -> Result<Vec<Entry>> {
    if depth >= options.max_depth || *count >= options.max_objects {
        return Ok(vec![]);
    }
    let mut entries = Vec::new();
    let mut names = group.member_names().map_err(error)?;
    names.sort();
    for name in names {
        if *count >= options.max_objects {
            break;
        }
        *count += 1;
        let child_path = if path == "/" {
            format!("/{name}")
        } else {
            format!("{path}/{name}")
        };
        let kind = match group.loc_type_by_name(&name) {
            Ok(LocationType::Group) => Kind::Group,
            Ok(LocationType::Dataset) => match group.dataset(&name) {
                Ok(dataset) => Kind::Dataset {
                    shape: dataset.shape(),
                    numeric: dataset
                        .dtype()
                        .map(|dtype| dtype.conv_to::<f64>().is_some())
                        .unwrap_or(false),
                },
                Err(_) => Kind::Other,
            },
            _ => Kind::Other,
        };
        let descendants = if matches!(kind, Kind::Group) {
            group
                .group(&name)
                .ok()
                .map(|child| children(&child, &child_path, depth + 1, count, options))
                .transpose()?
                .unwrap_or_default()
        } else {
            vec![]
        };
        entries.push(Entry {
            name,
            path: child_path,
            kind,
            children: descendants,
        });
    }
    // Groups first, then datasets (and other entries), each section sorted alphabetically.
    entries.sort_by(|a, b| {
        let group_rank = |e: &Entry| {
            if matches!(e.kind, Kind::Group) {
                0u8
            } else {
                1u8
            }
        };
        group_rank(a)
            .cmp(&group_rank(b))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(entries)
}
