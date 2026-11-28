//! Attribute, tree, and numeric formatting helpers for the file adapters.
use super::*;

pub(super) fn numeric_cell_selection(
    dataset: &H5Dataset,
    coordinates: &[usize],
) -> Result<Hyperslab> {
    let shape = dataset.shape();
    if coordinates.len() != shape.len()
        || coordinates.is_empty()
        || coordinates
            .iter()
            .zip(&shape)
            .any(|(&index, &length)| index >= length)
    {
        return Err(error("Cell coordinates are outside the dataset"));
    }
    Ok(Hyperslab::from(
        coordinates
            .iter()
            .map(|&index| SliceOrIndex::from(index..index + 1))
            .collect::<Vec<_>>(),
    ))
}

pub(super) fn normalize_numeric_value(descriptor: &TypeDescriptor, text: &str) -> Result<String> {
    macro_rules! parse_as {
        ($ty:ty) => {
            text.trim()
                .parse::<$ty>()
                .map(|value| value.to_string())
                .map_err(|_| error(format!("Invalid {descriptor} value: {text}")))
        };
    }
    match descriptor {
        TypeDescriptor::Integer(IntSize::U1) => parse_as!(i8),
        TypeDescriptor::Integer(IntSize::U2) => parse_as!(i16),
        TypeDescriptor::Integer(IntSize::U4) => parse_as!(i32),
        TypeDescriptor::Integer(IntSize::U8) => parse_as!(i64),
        TypeDescriptor::Unsigned(IntSize::U1) => parse_as!(u8),
        TypeDescriptor::Unsigned(IntSize::U2) => parse_as!(u16),
        TypeDescriptor::Unsigned(IntSize::U4) => parse_as!(u32),
        TypeDescriptor::Unsigned(IntSize::U8) => parse_as!(u64),
        TypeDescriptor::Float(FloatSize::U4) => parse_as!(f32),
        TypeDescriptor::Float(FloatSize::U8) => parse_as!(f64),
        _ => Err(error(format!(
            "Editing {descriptor} values is not supported"
        ))),
    }
}

pub(super) fn normalize_dataset_numeric_value(
    dataset: &H5Dataset,
    descriptor: &TypeDescriptor,
    text: &str,
) -> Result<String> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("fill") {
        return read_dataset_fill_value(dataset, descriptor);
    }
    let special = if text.eq_ignore_ascii_case("nan") {
        Some(f64::NAN)
    } else if text.eq_ignore_ascii_case("+inf")
        || text.eq_ignore_ascii_case("inf")
        || text.eq_ignore_ascii_case("+infinity")
        || text.eq_ignore_ascii_case("infinity")
    {
        Some(f64::INFINITY)
    } else if text.eq_ignore_ascii_case("-inf") || text.eq_ignore_ascii_case("-infinity") {
        Some(f64::NEG_INFINITY)
    } else {
        None
    };
    if let Some(value) = special {
        return match descriptor {
            TypeDescriptor::Float(FloatSize::U4) => Ok((value as f32).to_string()),
            TypeDescriptor::Float(FloatSize::U8) => Ok(value.to_string()),
            _ => Err(error("NaN and infinities require a floating-point dataset")),
        };
    }
    normalize_numeric_value(descriptor, text)
}

pub(super) fn read_dataset_fill_value(
    dataset: &H5Dataset,
    descriptor: &TypeDescriptor,
) -> Result<String> {
    let attribute = dataset
        .attr("_FillValue")
        .map_err(|_| error("This dataset has no _FillValue attribute"))?;
    if attribute.size() != 1 {
        return Err(error("The _FillValue attribute must contain one value"));
    }
    let fill_type = attribute
        .dtype()
        .map_err(error)?
        .to_descriptor()
        .map_err(error)?;
    if &fill_type != descriptor {
        return Err(error("The _FillValue type does not match the dataset type"));
    }
    macro_rules! read_as {
        ($ty:ty) => {
            attribute
                .read_raw::<$ty>()
                .map_err(error)?
                .into_iter()
                .next()
                .map(|value| value.to_string())
                .ok_or_else(|| error("The _FillValue attribute is empty"))
        };
    }
    match descriptor {
        TypeDescriptor::Integer(IntSize::U1) => read_as!(i8),
        TypeDescriptor::Integer(IntSize::U2) => read_as!(i16),
        TypeDescriptor::Integer(IntSize::U4) => read_as!(i32),
        TypeDescriptor::Integer(IntSize::U8) => read_as!(i64),
        TypeDescriptor::Unsigned(IntSize::U1) => read_as!(u8),
        TypeDescriptor::Unsigned(IntSize::U2) => read_as!(u16),
        TypeDescriptor::Unsigned(IntSize::U4) => read_as!(u32),
        TypeDescriptor::Unsigned(IntSize::U8) => read_as!(u64),
        TypeDescriptor::Float(FloatSize::U4) => read_as!(f32),
        TypeDescriptor::Float(FloatSize::U8) => read_as!(f64),
        _ => Err(error(format!(
            "Editing {descriptor} values is not supported"
        ))),
    }
}

pub(super) fn read_numeric_cell(dataset: &H5Dataset, coordinates: &[usize]) -> Result<String> {
    let descriptor = dataset
        .dtype()
        .map_err(error)?
        .to_descriptor()
        .map_err(error)?;
    macro_rules! read_as {
        ($ty:ty) => {{
            dataset
                .read_slice::<$ty, _, IxDyn>(numeric_cell_selection(dataset, coordinates)?)
                .map_err(error)?
                .into_raw_vec_and_offset()
                .0
                .into_iter()
                .next()
                .ok_or_else(|| error("Cell read returned no value"))
                .map(|value| value.to_string())
        }};
    }
    match descriptor {
        TypeDescriptor::Integer(IntSize::U1) => read_as!(i8),
        TypeDescriptor::Integer(IntSize::U2) => read_as!(i16),
        TypeDescriptor::Integer(IntSize::U4) => read_as!(i32),
        TypeDescriptor::Integer(IntSize::U8) => read_as!(i64),
        TypeDescriptor::Unsigned(IntSize::U1) => read_as!(u8),
        TypeDescriptor::Unsigned(IntSize::U2) => read_as!(u16),
        TypeDescriptor::Unsigned(IntSize::U4) => read_as!(u32),
        TypeDescriptor::Unsigned(IntSize::U8) => read_as!(u64),
        TypeDescriptor::Float(FloatSize::U4) => read_as!(f32),
        TypeDescriptor::Float(FloatSize::U8) => read_as!(f64),
        _ => Err(error(format!(
            "Editing {descriptor} values is not supported"
        ))),
    }
}

pub(super) fn write_numeric_cell(
    dataset: &H5Dataset,
    coordinates: &[usize],
    descriptor: &TypeDescriptor,
    text: &str,
) -> Result<()> {
    let shape = vec![1; coordinates.len()];
    macro_rules! write_as {
        ($ty:ty) => {{
            let value = text.parse::<$ty>().map_err(error)?;
            let values = ArrayD::from_elem(IxDyn(&shape), value);
            dataset
                .write_slice(values.view(), numeric_cell_selection(dataset, coordinates)?)
                .map_err(error)
        }};
    }
    match descriptor {
        TypeDescriptor::Integer(IntSize::U1) => write_as!(i8),
        TypeDescriptor::Integer(IntSize::U2) => write_as!(i16),
        TypeDescriptor::Integer(IntSize::U4) => write_as!(i32),
        TypeDescriptor::Integer(IntSize::U8) => write_as!(i64),
        TypeDescriptor::Unsigned(IntSize::U1) => write_as!(u8),
        TypeDescriptor::Unsigned(IntSize::U2) => write_as!(u16),
        TypeDescriptor::Unsigned(IntSize::U4) => write_as!(u32),
        TypeDescriptor::Unsigned(IntSize::U8) => write_as!(u64),
        TypeDescriptor::Float(FloatSize::U4) => write_as!(f32),
        TypeDescriptor::Float(FloatSize::U8) => write_as!(f64),
        _ => Err(error(format!(
            "Editing {descriptor} values is not supported"
        ))),
    }
}

pub(super) fn find_entry<'a>(root: &'a Entry, path: &str) -> Option<&'a Entry> {
    if root.path == path {
        return Some(root);
    }
    root.children
        .iter()
        .find_map(|child| find_entry(child, path))
}
pub(super) fn netcdf_type_is_numeric(datatype: &NcType) -> bool {
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
pub(super) fn read_h5_fill_value(attribute: &H5Attribute) -> Option<f64> {
    if attribute.size() == 1 {
        attribute.read_raw::<f64>().ok()?.into_iter().next()
    } else {
        None
    }
}

pub(super) fn read_h5_attributes(
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
pub(super) fn format_strings<'a>(values: impl Iterator<Item = &'a str>) -> String {
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
pub(super) fn format_numbers(values: &[f64]) -> String {
    format_values(values)
}
pub(super) fn format_values<T: ToString>(values: &[T]) -> String {
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
pub(super) fn format_netcdf_attribute(value: &NcAttrValue) -> String {
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
pub(super) fn children(
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
