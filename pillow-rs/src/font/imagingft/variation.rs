use super::layout::native_long_to_i64;
use super::*;
use fontdone::tt;

pub(crate) fn has_variations(font: &FreeTypeFont) -> bool {
    native_long_to_i64(font.engine.face.face_flags) & ffi::FT_FACE_FLAG_MULTIPLE_MASTERS != 0
}

pub(crate) fn font_variant(
    font: &FreeTypeFont,
    size: Option<f32>,
) -> Result<FreeTypeFont, PilError> {
    font_variant_with_options(
        font,
        &ImageFontVariantOptions {
            size,
            ..ImageFontVariantOptions::default()
        },
    )
}

pub(crate) fn font_variant_with_options(
    font: &FreeTypeFont,
    options: &ImageFontVariantOptions,
) -> Result<FreeTypeFont, PilError> {
    let load_options = ImageFontLoadOptions {
        index: Some(options.index.unwrap_or(font.engine.face_index)),
        encoding: options
            .encoding
            .clone()
            .or_else(|| font.engine.encoding.clone()),
        layout_engine: options
            .layout_engine
            .clone()
            .or_else(|| font.engine.layout_engine.clone()),
    };
    load_truetype_with_index(
        options
            .font_bytes
            .clone()
            .map(Rc::new)
            .unwrap_or_else(|| Rc::clone(&font.engine.font_bytes)),
        options.size.unwrap_or(font.engine.size_pt),
        load_options.index.unwrap_or(0),
        load_options.encoding,
        load_options.layout_engine,
        Some(font),
    )
}

pub(crate) fn get_variation_axes(
    font: &FreeTypeFont,
) -> Result<Vec<ImageFontVariationAxis>, PilError> {
    if type1_mm_axis_count(font)?.is_some() {
        // Pillow's ImageFont.get_variation_axes() unconditionally reads
        // axis["name"] after the native getvaraxes call.  FreeType's Type 1
        // MM records do not carry an SFNT name, so the version-matched Pillow
        // wrapper raises KeyError('name') before returning the numeric axes.
        return Err(PilError::KeyError("name".into()));
    }
    ensure_variation_metadata(font)?;
    Ok(font
        .engine
        .variation_metadata
        .borrow()
        .as_ref()
        .map(|metadata| metadata.axes.clone())
        .unwrap_or_default())
}

pub(crate) fn get_variation_names(font: &FreeTypeFont) -> Result<Vec<Vec<u8>>, PilError> {
    ensure_variation_metadata(font)?;
    Ok(font
        .engine
        .variation_metadata
        .borrow()
        .as_ref()
        .map(|metadata| metadata.names.clone())
        .unwrap_or_default())
}

pub(super) fn ensure_variation_metadata(font: &FreeTypeFont) -> Result<(), PilError> {
    if font.engine.variation_metadata.borrow().is_some() {
        return Ok(());
    }
    if let Some(metadata) = cached_source_variation_metadata(
        font.engine.font_bytes.as_slice(),
        font.engine.source_face_index,
    ) {
        *font.engine.variation_metadata.borrow_mut() = Some(metadata);
        return Ok(());
    }
    let metadata = build_variation_metadata(font)?;
    *font.engine.variation_metadata.borrow_mut() = Some(metadata.clone());
    remember_source_variation_metadata(font, metadata);
    Ok(())
}

pub(super) fn cached_source_variation_metadata(
    data: &[u8],
    face_index: ffi::FT_Long,
) -> Option<VariationMetadata> {
    CACHED_VARIATION_METADATA.with(|cache| {
        let cache = cache.borrow();
        let cached = cache.as_ref()?;
        (data.len() <= MAX_CACHED_SOURCE_BYTES
            && cached.face_index == face_index
            && cached.source_bytes.as_slice() == data)
            .then(|| cached.metadata.clone())
    })
}

fn remember_source_variation_metadata(font: &FreeTypeFont, metadata: VariationMetadata) {
    if font.engine.font_bytes.len() > MAX_CACHED_SOURCE_BYTES {
        return;
    }
    CACHED_VARIATION_METADATA.with(|cache| {
        let mut cache = cache.borrow_mut();
        *cache = Some(CachedVariationMetadata {
            face_index: font.engine.source_face_index,
            source_bytes: Rc::clone(&font.engine.font_bytes),
            metadata,
        });
    });
}

fn build_variation_metadata(font: &FreeTypeFont) -> Result<VariationMetadata, PilError> {
    if type1_mm_axes(font)?.is_some() {
        return Ok(VariationMetadata::default());
    }
    let (fvar, name_table) = variation_tables(font)?;
    let axes = fvar
        .axes
        .iter()
        .map(|axis| ImageFontVariationAxis {
            minimum: fixed_16_16_to_pillow_int(axis.min_value),
            default: fixed_16_16_to_pillow_int(axis.default_value),
            maximum: fixed_16_16_to_pillow_int(axis.max_value),
            name: name_bytes(&name_table, axis.name_id)
                .unwrap_or_default()
                .into_iter()
                .filter(|byte| *byte != 0)
                .collect(),
        })
        .collect();
    let instance_names = fvar
        .instances
        .iter()
        .map(|instance| {
            name_bytes(&name_table, instance.subfamily_name_id)
                .unwrap_or_default()
                .into_iter()
                .filter(|byte| *byte != 0)
                .collect()
        })
        .collect::<Vec<Vec<u8>>>();
    let mut names = Vec::with_capacity(instance_names.len());
    for name in instance_names {
        // Pillow's ImageFont.get_variation_names() preserves first-seen order
        // while removing duplicate subfamily names before public lookup.
        if !names.contains(&name) {
            names.push(name);
        }
    }
    // `set_variation_by_name` maps the deduplicated public name position to
    // FreeType's one-based named-instance index. Cache coordinates in that
    // same order so it can use the cheaper design-coordinate setter without
    // reparsing the variable face for every selection.
    let named_instance_coordinates = fvar
        .instances
        .iter()
        .take(names.len())
        .map(|instance| {
            instance
                .coords
                .iter()
                .copied()
                .map(ffi::FT_Fixed::from)
                .collect()
        })
        .collect();
    let named_instance_style_names = fvar
        .instances
        .iter()
        .take(names.len())
        .map(|instance| freetype_named_instance_style_name(&name_table, instance.subfamily_name_id))
        .collect();
    Ok(VariationMetadata {
        names,
        axes,
        named_instance_coordinates,
        named_instance_style_names,
    })
}

fn variation_instance_names(font: &FreeTypeFont) -> Result<Vec<Vec<u8>>, PilError> {
    if type1_mm_axes(font)?.is_some() {
        return Ok(Vec::new());
    }
    let (fvar, name_table) = variation_tables(font)?;
    Ok(fvar
        .instances
        .iter()
        .map(|instance| {
            name_bytes(&name_table, instance.subfamily_name_id)
                .unwrap_or_default()
                .into_iter()
                .filter(|byte| *byte != 0)
                .collect()
        })
        .collect())
}

pub(crate) fn set_variation_by_name(font: &mut FreeTypeFont, name: &[u8]) -> Result<(), PilError> {
    ensure_variation_metadata(font)?;
    let (index, coordinates, style_name) = {
        let metadata = font.engine.variation_metadata.borrow();
        let names = metadata
            .as_ref()
            .map(|metadata| metadata.names.as_slice())
            .unwrap_or_default();
        let Some(index) = names.iter().position(|candidate| candidate == name) else {
            return Err(PilError::ValueError(format!(
                "b'{}' is not in list",
                String::from_utf8_lossy(name)
            )));
        };
        let coordinates = metadata
            .as_ref()
            .and_then(|metadata| metadata.named_instance_coordinates.get(index))
            .cloned()
            .ok_or_else(|| PilError::OsError("invalid argument".into()))?;
        let style_name = metadata
            .as_ref()
            .and_then(|metadata| metadata.named_instance_style_names.get(index))
            .cloned()
            .ok_or_else(|| PilError::OsError("invalid argument".into()))?;
        (index, coordinates, style_name)
    };
    let instance_index = index + 1;
    if font.engine.last_variation_index == Some(instance_index) {
        return Ok(());
    }
    // Pillow updates this cache before calling the native setter. Preserve
    // that ordering so a failed native call has the same subsequent no-op
    // behavior as the reference wrapper.
    font.engine.last_variation_index = Some(instance_index);
    font.engine
        .basic_layout_cache
        .borrow_mut()
        .clear_variable_metrics();

    let base_face_index = font.engine.source_face_index & 0xFFFF;
    let instance_index_long = ffi::FT_Long::try_from(instance_index)
        .map_err(|_| PilError::OsError("invalid argument".into()))?;
    let instance_multiplier = ffi::FT_Long::from(1_i8)
        .checked_shl(16)
        .ok_or_else(|| PilError::OsError("invalid argument".into()))?;
    let encoded_instance = instance_index_long
        .checked_mul(instance_multiplier)
        .ok_or_else(|| PilError::OsError("invalid argument".into()))?;
    let selected_face_index = base_face_index | encoded_instance;
    let coordinates_count = ffi::FT_UInt::try_from(coordinates.len())
        .map_err(|_| PilError::OsError("invalid argument".into()))?;

    // fontdone's FT_Set_Named_Instance currently reconstructs the complete
    // variable face. The fvar coordinates are already cached above, and
    // FT_Set_Var_Design_Coordinates reuses the parsed SFNT tables when it can.
    // Temporarily expose the collection face index so that refresh keeps the
    // existing face and size handles; restore FreeType's encoded instance
    // index after the coordinate update for font_variant() and public state.
    let previous_face_index = font.engine.face.face_index;
    font.engine.face.face_index = base_face_index;
    let coordinate_status = ffi::FT_Set_Var_Design_Coordinates(
        Some(&mut font.engine.face),
        coordinates_count,
        Some(&coordinates),
    );
    if coordinate_status == ffi::FT_Err_Ok {
        font.engine.face.face_index = selected_face_index;
    } else {
        font.engine.face.face_index = previous_face_index;
        let status = ffi::FT_Set_Named_Instance(
            Some(&mut font.engine.face),
            ffi::FT_UInt::try_from(instance_index)
                .map_err(|_| PilError::OsError("invalid argument".into()))?,
        );
        check_ft_error(status)?;
    }
    refresh_engine_metadata(font);
    font.engine.style_name = Some(style_name);
    Ok(())
}

pub(crate) fn set_variation_by_axes(font: &mut FreeTypeFont, axes: &[f32]) -> Result<(), PilError> {
    if !has_variations(font) {
        return Err(PilError::OsError("invalid argument".into()));
    }
    let coords = axes
        .iter()
        .map(|axis| pillow_axis_to_fixed(*axis))
        .collect::<Vec<_>>();
    font.engine
        .basic_layout_cache
        .borrow_mut()
        .clear_variable_metrics();
    check_ft_error(ffi::FT_Set_Var_Design_Coordinates(
        Some(&mut font.engine.face),
        coords.len() as ffi::FT_UInt,
        Some(&coords),
    ))?;
    refresh_engine_metadata(font);
    Ok(())
}

pub(crate) fn native_getvaraxes(
    font: &FreeTypeFont,
) -> Result<Vec<ImageFontVariationAxis>, PilError> {
    if let Some(axes) = type1_mm_axes(font)? {
        return Ok(axes);
    }
    let (fvar, name_table) = variation_tables(font)?;
    Ok(fvar
        .axes
        .iter()
        .map(|axis| ImageFontVariationAxis {
            minimum: fixed_16_16_to_pillow_int(axis.min_value),
            default: fixed_16_16_to_pillow_int(axis.default_value),
            maximum: fixed_16_16_to_pillow_int(axis.max_value),
            name: raw_name_bytes(&name_table, axis.name_id).unwrap_or_default(),
        })
        .collect())
}

pub(crate) fn native_getvarnames(font: &FreeTypeFont) -> Result<Vec<Vec<u8>>, PilError> {
    if type1_mm_axes(font)?.is_some() {
        return Ok(Vec::new());
    }
    let (fvar, name_table) = variation_tables(font)?;
    Ok(fvar
        .instances
        .iter()
        .map(|instance| raw_name_bytes(&name_table, instance.subfamily_name_id).unwrap_or_default())
        .collect())
}

pub(crate) fn native_setvarname(
    font: &mut FreeTypeFont,
    instance_index: i64,
) -> Result<(), PilError> {
    let instance_index =
        u32::try_from(instance_index).map_err(|_| PilError::OsError("invalid argument".into()))?;
    if type1_mm_axis_count(font)?.is_some() {
        font.engine
            .basic_layout_cache
            .borrow_mut()
            .clear_variable_metrics();
        let status =
            ffi::FT_Set_Named_Instance(Some(&mut font.engine.face), instance_index as ffi::FT_UInt);
        check_ft_error(status)?;
        refresh_engine_metadata(font);
        return Ok(());
    }
    let names = if instance_index == 0 {
        Vec::new()
    } else {
        // FreeType indexes the raw fvar instance array. The public
        // get_variation_names() path intentionally removes duplicate names,
        // so using it here made a valid native index address the wrong entry
        // and could panic when duplicate subfamily names were present.
        variation_instance_names(font)?
    };
    let name_index = instance_index.saturating_sub(1) as usize;
    if instance_index != 0 && name_index >= names.len() {
        return Err(PilError::OsError("invalid argument".into()));
    }
    font.engine
        .basic_layout_cache
        .borrow_mut()
        .clear_variable_metrics();
    let status = ffi::FT_Set_Named_Instance(Some(&mut font.engine.face), instance_index);
    check_ft_error(status)?;
    refresh_engine_metadata(font);
    if instance_index != 0 {
        let name = &names[name_index];
        if name.is_empty() {
            // Pillow 12.2.0 `_imagingft.c::font_setvarname` accepts the
            // FreeType named instance first. If the selected instance has
            // no usable subfamily name, public `getname()` preserves
            // `None` rather than FreeType's refreshed empty style string.
            font.engine.style_name = None;
        } else {
            font.engine.style_name = Some(String::from_utf8_lossy(name).into_owned());
        }
    }
    Ok(())
}

pub(crate) fn native_setvaraxes(font: &mut FreeTypeFont, axes: &[f32]) -> Result<(), PilError> {
    set_variation_by_axes(font, axes)
}

/// Return the Type 1 Multiple Master axis count when the active face exposes
/// the Adobe MM service.
///
/// `fontdone` keeps the public axis names behind the safe facade's owned face
/// state.  Pillow only needs the numeric MM descriptor here; keeping the
/// adapter on the numeric `FT_*` records also avoids introducing raw-pointer
/// reads into this safe Rust crate.
fn type1_mm_axis_count(font: &FreeTypeFont) -> Result<Option<usize>, PilError> {
    if ffi::FT_Get_Font_Format(Some(&font.engine.face)) != Some("Type 1") {
        return Ok(None);
    }
    let mut multi_master = ffi::FT_Multi_Master::default();
    let status = ffi::FT_Get_Multi_Master(Some(&font.engine.face), Some(&mut multi_master));
    if status != ffi::FT_Err_Ok {
        return Ok(None);
    }
    let count = usize::try_from(multi_master.num_axis)
        .map_err(|_| PilError::OsError("invalid argument".into()))?;
    if count > multi_master.axis.len() {
        return Err(PilError::OsError("invalid argument".into()));
    }
    Ok(Some(count))
}

/// Read numeric Type 1 Multiple Master axes through fontdone's safe facade.
fn type1_mm_axes(font: &FreeTypeFont) -> Result<Option<Vec<ImageFontVariationAxis>>, PilError> {
    let Some(axis_count) = type1_mm_axis_count(font)? else {
        return Ok(None);
    };
    let mut descriptor = ffi::FT_MM_Var::default();
    let mut storage = vec![ffi::FT_Var_Axis::default(); axis_count];
    check_ft_error(ffi::FT_Get_MM_Var(
        Some(&font.engine.face),
        Some(&mut descriptor),
        Some(&mut storage),
        None,
        None,
    ))?;
    let axes = storage
        .into_iter()
        .map(|axis| {
            Ok(ImageFontVariationAxis {
                minimum: type1_fixed_16_16_to_pillow_int(axis.minimum)?,
                default: type1_fixed_16_16_to_pillow_int(axis.def)?,
                maximum: type1_fixed_16_16_to_pillow_int(axis.maximum)?,
                // Type 1 MM axes do not have SFNT name IDs.  The native
                // FreeType record consequently omits the name field; the
                // core record represents that absence as an empty byte name.
                name: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>, PilError>>()?;
    Ok(Some(axes))
}

fn type1_fixed_16_16_to_pillow_int(value: ffi::FT_Fixed) -> Result<i32, PilError> {
    let value = i32::try_from(value).map_err(|_| PilError::OsError("invalid argument".into()))?;
    Ok(fixed_16_16_to_pillow_int(value))
}

fn pillow_axis_to_fixed(axis: f32) -> ffi::FT_Fixed {
    let scaled = f64::from(axis) * 65536.0;
    let fixed = if scaled > ffi::FT_Long::MAX as f64 {
        // Pillow's `_imagingft.c::font_setvaraxes` first converts the input
        // to `FT_Fixed` and only then multiplies by 65536. On the versioned
        // 64-bit oracle, an out-of-range positive float follows FreeType's
        // signed conversion overflow path and becomes a below-minimum design
        // coordinate, just like the negative overflow path.
        i32::MIN
    } else if scaled < ffi::FT_Long::MIN as f64 {
        i32::MIN
    } else if scaled > f64::from(i32::MAX) {
        i32::MAX
    } else if scaled < f64::from(i32::MIN) {
        i32::MIN
    } else {
        scaled as i32
    };
    fixed as ffi::FT_Fixed
}

fn refresh_engine_metadata(font: &mut FreeTypeFont) {
    if let Ok(face_index) = usize::try_from(font.engine.face.face_index) {
        // `fontdone` refreshes the public face record after named-instance or
        // design-coordinate changes. Keep the adapter's constructor state in
        // sync so a later `font_variant()` reopens the same collection face
        // and variation instance instead of silently falling back to face 0.
        font.engine.face_index = face_index;
    }
    font.engine.family_name = font.engine.face.family_name.clone();
    font.engine.style_name = font.engine.face.style_name.clone();
    font.engine.metrics = font.engine.face.size_metrics;
}

fn variation_tables(
    font: &FreeTypeFont,
) -> Result<(tt::fvar::FvarTable, tt::name::NameTable), PilError> {
    let data = &font.engine.font_bytes;
    // `fontdone` encodes a selected named instance in bits 16..30 of the
    // face index; the collection face remains in the low 16 bits. The table
    // directory is face-specific, so resolving face 0 here returns the wrong
    // variation metadata for a non-zero TTC face.
    let collection_face_index = font.engine.face_index & 0xFFFF;
    let (_, face_offset) = tt::resolve_face_index(data, collection_face_index)
        .map_err(|_| PilError::OsError("invalid argument".into()))?;
    let directory = tt::parse_table_directory_at(data, face_offset)
        .map_err(|_| PilError::OsError("invalid argument".into()))?;
    let fvar = directory
        .find(data, tag(b"fvar"))
        .ok_or_else(|| PilError::OsError("invalid argument".into()))
        .and_then(|bytes| {
            tt::fvar::parse_fvar(bytes).map_err(|_| PilError::OsError("invalid argument".into()))
        })?;
    let name_table = directory
        .find(data, tag(b"name"))
        .ok_or_else(|| PilError::OsError("invalid argument".into()))
        .and_then(|bytes| {
            tt::name::parse_name(bytes).map_err(|_| PilError::OsError("invalid argument".into()))
        })?;
    Ok((fvar, name_table))
}

fn tag(bytes: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*bytes)
}

fn fixed_16_16_to_pillow_int(value: i32) -> i32 {
    value / 65536
}

fn name_bytes(table: &tt::name::NameTable, name_id: u16) -> Option<Vec<u8>> {
    let record = preferred_name_record(table, name_id)?;
    if record.platform_id == 3 {
        Some(decode_utf16be_to_utf8(&record.string).into_bytes())
    } else {
        Some(record.string.clone())
    }
}

fn raw_name_bytes(table: &tt::name::NameTable, name_id: u16) -> Option<Vec<u8>> {
    preferred_name_record(table, name_id).map(|record| record.string.clone())
}

fn preferred_name_record(
    table: &tt::name::NameTable,
    name_id: u16,
) -> Option<&tt::name::SfntNameRecord> {
    let preferred = table
        .records
        .iter()
        .position(|record| {
            record.name_id == name_id
                && record.platform_id == 3
                && matches!(record.encoding_id, 1 | 10)
                && record.language_id == 0x0409
        })
        .or_else(|| {
            table.records.iter().position(|record| {
                record.name_id == name_id
                    && record.platform_id == 3
                    && matches!(record.encoding_id, 1 | 10)
            })
        })
        .or_else(|| {
            table
                .records
                .iter()
                .position(|record| record.name_id == name_id && record.platform_id == 1)
        })?;
    Some(&table.records[preferred])
}

fn freetype_named_instance_style_name(table: &tt::name::NameTable, name_id: u16) -> String {
    // FreeType's named-instance setter resolves style names from Windows SFNT
    // records; its face style remains empty when an instance name exists only
    // in the Macintosh records used as a fallback by Pillow's variation-name
    // list. Cache that distinction so coordinate updates match the setter's
    // public metadata without rebuilding the face.
    let preferred = table
        .records
        .iter()
        .position(|record| {
            record.name_id == name_id
                && record.platform_id == 3
                && matches!(record.encoding_id, 0 | 1 | 10)
                && (record.language_id & 0x03ff) == 0x0009
        })
        .or_else(|| {
            table.records.iter().position(|record| {
                record.name_id == name_id
                    && record.platform_id == 3
                    && matches!(record.encoding_id, 0 | 1 | 10)
            })
        });
    preferred
        .map(|index| decode_utf16be_to_utf8(&table.records[index].string))
        .unwrap_or_default()
}

fn decode_utf16be_to_utf8(bytes: &[u8]) -> String {
    char::decode_utf16(
        bytes
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]])),
    )
    .map(|value| value.unwrap_or(char::REPLACEMENT_CHARACTER))
    .collect()
}
