use super::huffman::HuffTable;
// ── Marker Constants ──────────────────────────────────────────────────────

pub(crate) const M_SOI: u16 = 0xFFD8;
pub(crate) const M_EOI: u16 = 0xFFD9;
pub(crate) const M_SOS: u16 = 0xFFDA;
pub(crate) const M_SOF0: u16 = 0xFFC0;
pub(crate) const M_SOF2: u16 = 0xFFC2;
pub(crate) const M_DHT: u16 = 0xFFC4;
pub(crate) const M_DQT: u16 = 0xFFDB;
pub(crate) const M_DRI: u16 = 0xFFDD;

// ── JPEG Structures ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub(super) struct FrameComponent {
    pub(super) id: u8,
    pub(super) h_samp: u8,
    pub(super) v_samp: u8,
    pub(super) quant_tbl: u8,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ScanComponent {
    pub(super) comp_index: usize,
    pub(super) dc_tbl: u8,
    pub(super) ac_tbl: u8,
}

#[derive(Debug, Clone)]
pub(super) struct ScanInfo {
    pub(super) components: Vec<ScanComponent>,
    pub(super) entropy_start: usize,
    pub(super) entropy_end: usize,
    pub(super) ss: u8,
    pub(super) se: u8,
    pub(super) ah: u8,
    pub(super) al: u8,
    pub(super) dc_huff_tables: Vec<Option<HuffTable>>,
    pub(super) ac_huff_tables: Vec<Option<HuffTable>>,
}

pub(super) struct JpegInfo {
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) num_components: u8,
    pub(super) components: Vec<FrameComponent>,
    pub(super) quant_tables: Vec<Option<[u16; 64]>>,
    pub(super) dc_huff_tables: Vec<Option<HuffTable>>,
    pub(super) ac_huff_tables: Vec<Option<HuffTable>>,
    pub(super) scan_components: Vec<ScanComponent>,
    pub(super) restart_interval: u16,
    pub(super) entropy_start: usize,
    pub(super) eoi_pos: usize,
    pub(super) max_h_samp: u8,
    pub(super) max_v_samp: u8,
    pub(super) progressive: bool,
    pub(super) scans: Vec<ScanInfo>,
}

// ── JPEG Parser ───────────────────────────────────────────────────────────

pub(super) fn read_u16(data: &[u8], pos: &mut usize) -> Option<u16> {
    if *pos + 1 >= data.len() {
        return None;
    }
    let val = (data[*pos] as u16) << 8 | data[*pos + 1] as u16;
    *pos += 2;
    Some(val)
}

pub(super) fn read_u8(data: &[u8], pos: &mut usize) -> Option<u8> {
    if *pos >= data.len() {
        return None;
    }
    let val = data[*pos];
    *pos += 1;
    Some(val)
}

pub(super) fn find_next_marker(data: &[u8], pos: &mut usize) -> Option<u16> {
    while *pos < data.len() {
        while *pos < data.len() && data[*pos] != 0xFF {
            *pos += 1;
        }
        if *pos >= data.len() {
            return None;
        }
        if *pos + 1 >= data.len() {
            return None;
        }
        let marker_byte = data[*pos + 1];
        if marker_byte == 0x00 || marker_byte == 0xFF {
            *pos += 1;
            continue;
        }
        let marker = 0xFF00u16 | marker_byte as u16;
        *pos += 2;
        return Some(marker);
    }
    None
}

pub(super) fn find_entropy_end(data: &[u8], mut pos: usize) -> usize {
    while pos + 1 < data.len() {
        if data[pos] == 0xFF {
            let next = data[pos + 1];
            if next == 0x00 {
                pos += 2;
            } else if next >= 0xD0 && next <= 0xD7 {
                pos += 2;
            } else {
                return pos;
            }
        } else {
            pos += 1;
        }
    }
    data.len()
}

pub(super) fn find_eoi(data: &[u8], mut pos: usize) -> Option<usize> {
    while pos + 1 < data.len() {
        if data[pos] == 0xFF && data[pos + 1] == 0xD9 {
            return Some(pos);
        }
        pos += 1;
    }
    None
}

pub(super) fn parse_sof0(
    data: &[u8],
    pos: &mut usize,
) -> Option<(u16, u16, Vec<FrameComponent>, u8, u8)> {
    let _length = read_u16(data, pos)?;
    let precision = read_u8(data, pos)?;
    if precision != 8 {
        return None;
    }
    let height = read_u16(data, pos)?;
    let width = read_u16(data, pos)?;
    let num_components = read_u8(data, pos)?;
    if num_components != 1 && num_components != 3 {
        return None;
    }

    let mut components = Vec::with_capacity(num_components as usize);
    let mut max_h_samp = 0u8;
    let mut max_v_samp = 0u8;

    for _ in 0..num_components {
        let id = read_u8(data, pos)?;
        let sampling = read_u8(data, pos)?;
        let h_samp = sampling >> 4;
        let v_samp = sampling & 0x0F;
        let quant_tbl = read_u8(data, pos)?;
        if h_samp < 1 || h_samp > 4 || v_samp < 1 || v_samp > 4 {
            return None;
        }
        if quant_tbl > 3 {
            return None;
        }
        max_h_samp = max_h_samp.max(h_samp);
        max_v_samp = max_v_samp.max(v_samp);
        components.push(FrameComponent {
            id,
            h_samp,
            v_samp,
            quant_tbl,
        });
    }

    Some((width, height, components, max_h_samp, max_v_samp))
}

pub(super) fn parse_dqt(
    data: &[u8],
    pos: &mut usize,
    quant_tables: &mut Vec<Option<[u16; 64]>>,
) -> Option<()> {
    let length = read_u16(data, pos)? as usize;
    let end = *pos + length - 2;

    while *pos < end {
        let info = read_u8(data, pos)?;
        let precision = (info >> 4) as usize;
        let table_id = (info & 0x0F) as usize;
        if table_id >= 4 {
            return None;
        }

        let mut table_zigzag = [0u16; 64];
        for entry in &mut table_zigzag {
            *entry = if precision == 0 {
                read_u8(data, pos)? as u16
            } else {
                read_u16(data, pos)?
            };
        }
        while quant_tables.len() <= table_id {
            quant_tables.push(None);
        }
        quant_tables[table_id] = Some(table_zigzag);
    }
    Some(())
}

pub(super) fn parse_dht(
    data: &[u8],
    pos: &mut usize,
    dc_tables: &mut Vec<Option<HuffTable>>,
    ac_tables: &mut Vec<Option<HuffTable>>,
) -> Option<()> {
    let length = read_u16(data, pos)? as usize;
    let end = *pos + length - 2;

    while *pos < end {
        let info = read_u8(data, pos)?;
        let table_class = info >> 4;
        let table_id = (info & 0x0F) as usize;
        if table_id >= 4 {
            return None;
        }

        let mut counts = [0u8; 16];
        let mut total_values = 0usize;
        for entry in &mut counts {
            *entry = read_u8(data, pos)?;
            total_values += *entry as usize;
        }

        let mut values = Vec::with_capacity(total_values);
        for _ in 0..total_values {
            values.push(read_u8(data, pos)?);
        }

        let table = HuffTable::build(&counts, &values);
        if table_class == 0 {
            while dc_tables.len() <= table_id {
                dc_tables.push(None);
            }
            dc_tables[table_id] = Some(table);
        } else {
            while ac_tables.len() <= table_id {
                ac_tables.push(None);
            }
            ac_tables[table_id] = Some(table);
        }
    }
    Some(())
}

pub(super) fn parse_sos(
    data: &[u8],
    pos: &mut usize,
    components: &[FrameComponent],
) -> Option<(Vec<ScanComponent>, usize, u8, u8, u8, u8)> {
    let _len = read_u16(data, pos)?;
    let num_scan_comps = read_u8(data, pos)?;
    if num_scan_comps == 0 {
        return None;
    }

    let mut scan_comps = Vec::with_capacity(num_scan_comps as usize);
    for _ in 0..num_scan_comps {
        let comp_id = read_u8(data, pos)?;
        let tbl_info = read_u8(data, pos)?;
        let dc_tbl = tbl_info >> 4;
        let ac_tbl = tbl_info & 0x0F;
        let comp_index = components.iter().position(|c| c.id == comp_id)?;
        if dc_tbl > 3 || ac_tbl > 3 {
            return None;
        }
        scan_comps.push(ScanComponent {
            comp_index,
            dc_tbl,
            ac_tbl,
        });
    }

    let ss = read_u8(data, pos)?;
    let se = read_u8(data, pos)?;
    let ah_al = read_u8(data, pos)?;
    let ah = ah_al >> 4;
    let al = ah_al & 0x0F;
    let entropy_start = *pos;

    Some((scan_comps, entropy_start, ss, se, ah, al))
}

pub(super) fn parse_dri(data: &[u8], pos: &mut usize) -> Option<u16> {
    let _len = read_u16(data, pos)?;
    let restart_interval = read_u16(data, pos)?;
    Some(restart_interval)
}

pub(super) fn parse_jpeg(data: &[u8]) -> Option<JpegInfo> {
    let mut pos = 0usize;

    let soi = read_u16(data, &mut pos)?;
    if soi != M_SOI {
        return None;
    }

    let mut width = 0u16;
    let mut height = 0u16;
    let mut components: Vec<FrameComponent> = Vec::new();
    let mut num_components = 0u8;
    let mut max_h_samp = 0u8;
    let mut max_v_samp = 0u8;
    let mut quant_tables: Vec<Option<[u16; 64]>> = Vec::new();
    let mut dc_huff_tables: Vec<Option<HuffTable>> = Vec::new();
    let mut ac_huff_tables: Vec<Option<HuffTable>> = Vec::new();
    let mut scan_components: Vec<ScanComponent> = Vec::new();
    let mut restart_interval: u16 = 0;
    let mut entropy_start: Option<usize> = None;
    let mut saw_sof = false;
    let mut saw_sos = false;
    let mut progressive = false;
    let mut scans: Vec<ScanInfo> = Vec::new();

    loop {
        let marker = find_next_marker(data, &mut pos)?;

        match marker {
            M_SOF0 | M_SOF2 => {
                if saw_sof {
                    return None;
                }
                progressive = marker == M_SOF2;
                let result = parse_sof0(data, &mut pos)?;
                width = result.0;
                height = result.1;
                components = result.2;
                max_h_samp = result.3;
                max_v_samp = result.4;
                num_components = components.len() as u8;
                saw_sof = true;
            }
            M_DQT => {
                parse_dqt(data, &mut pos, &mut quant_tables)?;
            }
            M_DHT => {
                parse_dht(data, &mut pos, &mut dc_huff_tables, &mut ac_huff_tables)?;
            }
            M_SOS => {
                if !saw_sof {
                    return None;
                }
                let result = parse_sos(data, &mut pos, &components)?;
                let comps = result.0;
                let scan_start = result.1;
                let ss = result.2;
                let se = result.3;
                let ah = result.4;
                let al = result.5;
                let scan_end = find_entropy_end(data, pos);

                let scan_info = ScanInfo {
                    components: comps.clone(),
                    entropy_start: scan_start,
                    entropy_end: scan_end,
                    ss,
                    se,
                    ah,
                    al,
                    dc_huff_tables: dc_huff_tables.clone(),
                    ac_huff_tables: ac_huff_tables.clone(),
                };
                scans.push(scan_info);

                if !progressive {
                    if scan_components.is_empty() {
                        scan_components = comps;
                        entropy_start = Some(scan_start);
                    }
                    saw_sos = true;
                    if let Some(eoi) = find_eoi(data, pos) {
                        let _pos = eoi;
                    } else {
                        let _pos = data.len();
                    }
                    break;
                } else {
                    saw_sos = true;
                    if scan_components.is_empty() {
                        scan_components = comps;
                        entropy_start = Some(scan_start);
                    }
                    pos = scan_end;
                }
            }
            M_DRI => {
                restart_interval = parse_dri(data, &mut pos)?;
            }
            M_EOI => {
                break;
            }
            0xFFD0..=0xFFD7 => {}
            0xFF01 => {}
            _ => {
                let length = read_u16(data, &mut pos)? as usize;
                pos += length - 2;
            }
        }
    }

    if !saw_sos {
        return None;
    }
    let eoi_pos = find_eoi(data, 0).unwrap_or(data.len());

    Some(JpegInfo {
        width,
        height,
        num_components,
        components,
        quant_tables,
        dc_huff_tables,
        ac_huff_tables,
        scan_components,
        restart_interval,
        entropy_start: entropy_start?,
        eoi_pos,
        max_h_samp,
        max_v_samp,
        progressive,
        scans,
    })
}
