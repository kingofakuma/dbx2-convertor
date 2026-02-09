use anyhow::{Context, Result};
use std::path::Path;

use crate::{constants, marker, utils, io};

pub fn ps4_to_pcready(data: &[u8], input_path: &str, _dir: &str) -> Result<Vec<u8>> {
    // PS4 save format: [0x20 bytes MD5 ][0x80 bytes with #SAV at 0x20][rest of data ending with Z]
    // PC-ready format: [processed data with marker system][Z_BYTE][SAV_HEADER][MD5_HEADER] where [SAV_HEADER] has #SAV and [MD5_HEADER] is the first 0x20 bytes

    // Extract the 0x20 MD5_HEADER and the original [SAV_HEADER] section (0x80 bytes with #SAV at 0x00 relative to SAV_HEADER)
    let md5_header = &data[0..constants::MD5_HEADER_SIZE];  // First 0x20 bytes (to be moved to end)
    let sav_header = &data[constants::MD5_HEADER_SIZE..constants::MD5_HEADER_SIZE + constants::SAV_HEADER_SIZE];  // Next 0x80 bytes with #SAV (to be moved to end)
    let z_byte = data[data.len() - 1];  // Last byte - preserved during conversion
    let middle = &data[constants::MD5_HEADER_SIZE + constants::SAV_HEADER_SIZE..data.len() - 1];  // Middle part without md5_header/sav_header and z_byte
    let hcd_start_in_middle = constants::HCD_START_PS4 - 0x80;

    if middle.len() <= 8 {
        return Err(anyhow::anyhow!("PS4 structure too small."));
    }

    // Extract data segments for the algorithm:
    // middle_segment = middle data between first_8_bytes and hcd_section
    // hcd_section = HCD data section (starts at HCD_START_PS4 in original PS4 format)
    let first_8_bytes = &middle[0..8];  // first_8_bytes - first 8 bytes of middle data

    // Calculate where hcd_section starts in the middle data
    let hcd_start_pos = hcd_start_in_middle; // Position of hcd_section within the middle slice
    if hcd_start_pos < 8 {
        return Err(anyhow::anyhow!("Bad Hcd1 start offset (middle_segment would be negative)."));
    }
    let middle_segment_len = hcd_start_pos - 8; // Length of middle_segment: middle[8 .. hcd_start_pos-1] (data between first_8_bytes and hcd_section)
    let middle_segment = &middle[8..hcd_start_pos]; // middle_segment - middle segment between first_8_bytes and hcd_section

    let hcd_section_len = middle.len() - hcd_start_pos; // Length of hcd_section: middle[hcd_start_pos .. end] (HCD data)
    if hcd_section_len == 0 {
        return Err(anyhow::anyhow!("Bad Hcd1 start offset (hcd_section empty)."));
    }

    let hcd_section = &middle[hcd_start_pos..]; // hcd_section - HCD data section

    // Calculate fill to place D at HCD_START_PC_READY in the final PC-ready output
    let base_d_start = 8 /*first_8_bytes*/ + 8 /*marker*/ + middle_segment_len; // where hcd_section would start with no fill
    if base_d_start > constants::HCD_START_PC_READY {
        return Err(anyhow::anyhow!(
            "fillLen negative (0x{:X}). ExpectedDStart too early vs data. Refusing.",
            constants::HCD_START_PC_READY - base_d_start
        ));
    }
    let fill_len = constants::HCD_START_PC_READY - base_d_start;

    // Build the processed data section: [first_8_bytes][marker(v2,T)][middle_segment][fill zeros][hcd_section]
    let marker = marker::make_marker(constants::VER_V2, constants::FLAG_NO_LEFTOVERS);
    let prefix_len = 8 + 8 + middle_segment_len + fill_len + hcd_section_len;
    let mut prefix_data = vec![0u8; prefix_len];

    let mut p = 0;
    prefix_data[p..p + 8].copy_from_slice(first_8_bytes);      // [first_8_bytes] - first 8 bytes of middle
    p += 8;
    prefix_data[p..p + 8].copy_from_slice(&marker); // [marker] - version marker
    p += 8;
    prefix_data[p..p + middle_segment_len].copy_from_slice(middle_segment);   // [middle_segment] - middle segment
    p += middle_segment_len;
    // fill zeros already default(0x00)
    p += fill_len;
    prefix_data[p..p + hcd_section_len].copy_from_slice(hcd_section); // [hcd_section] - HCD data

    // Calculate if we need padding or if we have excess data
    let required_main_part_len = constants::EDITOR_SIZE - 1 - constants::SAV_HEADER_SIZE - constants::MD5_HEADER_SIZE; // bytes before Z, A and PREFIX
    let pad = required_main_part_len as isize - prefix_data.len() as isize;

    let mut leftovers = false;

    if pad > 0 {
        // Append zeros padding if we have space left
        prefix_data = utils::append_zeros(prefix_data, pad as usize);
    } else if pad < 0 {
        // Trim excess from prefix_data if we have too much data
        let excess = (-pad) as usize;
        if excess > prefix_data.len() {
            return Err(anyhow::anyhow!("Excess trim larger than prefix; refusing."));
        }

        let removed = prefix_data[prefix_data.len() - excess..].to_vec();
        prefix_data.truncate(prefix_data.len() - excess);

        let all_zero = utils::all_zero(&removed);
        leftovers = !all_zero;

        if leftovers {
            // Write non-zero excess data to a sidecar file
            let leftovers_path = format!("{}.leftovers.dec", input_path);
            std::fs::write(&leftovers_path, &removed)
                .with_context(|| format!("Failed to write leftovers file: {}", leftovers_path))?;
            println!("LEFTOVERS → {} (0x{:X} bytes)",
                     Path::new(&leftovers_path).file_name()
                         .unwrap_or(std::ffi::OsStr::new(""))
                         .to_string_lossy(),
                     excess);
        }
    }

    if prefix_data.len() != required_main_part_len {
        return Err(anyhow::anyhow!(
            "Internal size mismatch: prefix 0x{:X} != required 0x{:X}.",
            prefix_data.len(),
            required_main_part_len
        ));
    }

    // Assemble final PC-ready format: [prefix_data][z_byte][sav_header][md5_header]
    let mut out_data = vec![0u8; constants::EDITOR_SIZE];
    out_data[0..prefix_data.len()].copy_from_slice(&prefix_data); // [prefix_data] - processed data
    out_data[prefix_data.len()] = z_byte; // [z_byte] - last byte from original
    out_data[prefix_data.len() + 1..prefix_data.len() + 1 + constants::SAV_HEADER_SIZE].copy_from_slice(sav_header); // [sav_header] - #SAV section
    let pos = out_data.len() - constants::MD5_HEADER_SIZE;
    out_data[pos..].copy_from_slice(md5_header); // [md5_header] - original first 0x20 bytes

    // Update marker leftovers flag if needed
    if leftovers {
        out_data[constants::MARKER_OFFSET + 5] = constants::FLAG_LEFTOVERS;
    }

    // Final sanity check: [SAV_HEADER] must be at the right position and start with #SAV
    if !marker::has_magic_at(&out_data, out_data.len() - constants::MD5_HEADER_SIZE - constants::SAV_HEADER_SIZE) {
        return Err(anyhow::anyhow!("Packed v2 sanity failed: #SAV not found at start of [SAV_HEADER]."));
    }

    if out_data.len() != constants::EDITOR_SIZE {
        return Err(anyhow::anyhow!("Packed output size mismatch."));
    }

    Ok(out_data)
}

pub fn convert_auto(data: &[u8], input_path: &str, dir: &str) -> Result<Vec<u8>> {
    let (version, flag) = marker::try_read_marker(data)
        .ok_or_else(|| anyhow::anyhow!("Marker not recognized at 0x08."))?;

    let looks_v2 = marker::looks_like_v2(data);

    // Only support v2 now
    if version == constants::VER_V2 || looks_v2 {
        if !looks_v2 {
            return Err(anyhow::anyhow!("Marker says v2 but layout sanity checks failed."));
        }
        let has_leftovers = flag == constants::FLAG_LEFTOVERS;
        pcready_to_ps4(data, input_path, dir, has_leftovers)
    } else {
        Err(anyhow::anyhow!("Only v2 format is supported now."))
    }
}

pub fn pcready_to_ps4(data: &[u8], input_path: &str, _dir: &str, has_leftovers_flag: bool) -> Result<Vec<u8>> {
    if data.len() != constants::EDITOR_SIZE {
        return Err(anyhow::anyhow!(
            "v2 unpack expects editor size 0x{:X}, got 0x{:X}.",
            constants::EDITOR_SIZE,
            data.len()
        ));
    }

    // PC-ready format: [processed data][z_byte][sav_header][md5_header] where [sav_header] has #SAV and [md5_header] is the first 0x20 bytes
    // PS4 format: [md5_header][sav_header][middle][z_byte]
    
    // Extract [md5_header], [sav_header], and [z_byte] from the end of the PC-ready format
    let md5_header_start = data.len() - constants::MD5_HEADER_SIZE;
    let sav_header_start = data.len() - constants::MD5_HEADER_SIZE - constants::SAV_HEADER_SIZE;
    let z_index = sav_header_start - 1;

    if !marker::has_magic_at(data, sav_header_start) {
        return Err(anyhow::anyhow!("v2 unpack sanity failed: #SAV not found at start of [sav_header]."));
    }

    let md5_header = &data[md5_header_start..md5_header_start + constants::MD5_HEADER_SIZE];  // [md5_header] - Last 0x20 bytes (was original [md5_header])
    let sav_header = &data[sav_header_start..sav_header_start + constants::SAV_HEADER_SIZE];  // [sav_header] - Before the last 0x20 bytes (was original [sav_header] section)
    let z_byte = data[z_index];  // [z_byte] - Byte before [sav_header] section (was original z_byte)

    // Extract first_8_bytes (first 8 bytes of the processed data)
    let first_8_bytes = &data[0..8];  // [first_8_bytes] - first 8 bytes of processed data

    let hcd_start_in_middle = constants::HCD_START_PS4 - 0x80;
    if hcd_start_in_middle < 8 {
        return Err(anyhow::anyhow!("Bad constants: middle_segment length negative."));
    }
    let middle_segment_len = hcd_start_in_middle - 8; // Length of middle_segment (minus 8 for [first_8_bytes])

    let middle_segment_start_in_packed = 16; // After [first_8_bytes] + marker (8 + 8 = 16 bytes)
    if middle_segment_start_in_packed + middle_segment_len > z_index {  // z_index is where the trailer starts
        return Err(anyhow::anyhow!("v2 unpack: middle_segment out of range."));
    }

    let middle_segment = &data[middle_segment_start_in_packed..middle_segment_start_in_packed + middle_segment_len];  // [middle_segment] section data

    // [hcd_section] present region is from expected hcd_section start to ZIndex (exclusive)
    if constants::HCD_START_PC_READY > z_index {
        return Err(anyhow::anyhow!("v2 unpack: hcd_section start beyond z_byte."));
    }

    let hcd_present_len = z_index - constants::HCD_START_PC_READY;
    let hcd_present = &data[constants::HCD_START_PC_READY..constants::HCD_START_PC_READY + hcd_present_len];  // [hcd_section] section data

    // Full [hcd_section] length in the middle part (original PS4 without md5_header/sav_header and z_byte): from calculated HCD1 start to end
    // The middle part length in original PS4 would be: PS4_SIZE - MD5_HEADER_SIZE - SAV_HEADER_SIZE - 1 (for z_byte)
    let middle_total_len = constants::PS4_SIZE - constants::MD5_HEADER_SIZE - constants::SAV_HEADER_SIZE - 1;
    let hcd_full_len = middle_total_len - hcd_start_in_middle; // From HCD start to end of middle

    if hcd_present_len > hcd_full_len {
        return Err(anyhow::anyhow!(
            "v2 unpack: hcd_section present is larger than hcd_section full (missing=0x{:X}). Refusing.",
            hcd_present_len - hcd_full_len
        ));
    }
    let missing = hcd_full_len - hcd_present_len;  // Missing bytes that need to be recovered

    let mut hcd_tail = vec![0u8; missing];  // Buffer for missing [hcd_section] bytes
    if missing > 0 {
        let mut filled = false;

        if has_leftovers_flag {
            let leftovers_path = format!("{}.leftovers.dec", input_path);
            if std::path::Path::new(&leftovers_path).exists() {
                let lf = io::read_file_bytes(&leftovers_path)?;
                let take = std::cmp::min(missing, lf.len());
                hcd_tail[0..take].copy_from_slice(&lf[0..take]);
                println!("v2 unpack: used leftovers {} (0x{:X} bytes)",
                         std::path::Path::new(&leftovers_path).file_name()
                             .unwrap_or(std::ffi::OsStr::new(""))
                             .to_string_lossy(),
                         take);
                filled = true;
            }
        }

        if !filled {
            // Fill with zeros if no leftovers file exists
            if has_leftovers_flag {
                println!("v2 unpack: marker indicates leftovers, but leftovers file not found — filling missing with zeros.");
            }
        }
    }

    // Build the middle part: [first_8_bytes][middle_segment][hcd_full]
    let middle_len = constants::PS4_SIZE - constants::MD5_HEADER_SIZE - constants::SAV_HEADER_SIZE - 1;
    let mut middle = vec![0u8; middle_len];

    let mut m = 0;
    middle[m..m + 8].copy_from_slice(first_8_bytes);      // [first_8_bytes] - first 8 bytes of middle
    m += 8;
    middle[m..m + middle_segment_len].copy_from_slice(middle_segment);  // [middle_segment] - middle segment
    m += middle_segment_len;
    middle[m..m + hcd_present_len].copy_from_slice(hcd_present);  // [hcd_section] - present hcd_section data
    m += hcd_present_len;
    if missing > 0 {
        middle[m..m + missing].copy_from_slice(&hcd_tail);  // [hcd_section] - missing hcd_section data (zeros or from leftovers)
        m += missing;
    }

    if m != middle_len {
        return Err(anyhow::anyhow!("v2 unpack: middle length mismatch."));
    }

    // Reconstruct final PS4 format: [md5_header][sav_header][middle][z_byte]
    let mut ps4 = vec![0u8; constants::PS4_SIZE];
    ps4[0..constants::MD5_HEADER_SIZE].copy_from_slice(md5_header);  // [md5_header] - original first 0x20 bytes
    ps4[constants::MD5_HEADER_SIZE..constants::MD5_HEADER_SIZE + constants::SAV_HEADER_SIZE].copy_from_slice(sav_header);  // [sav_header] - #SAV section
    ps4[constants::MD5_HEADER_SIZE + constants::SAV_HEADER_SIZE..constants::MD5_HEADER_SIZE + constants::SAV_HEADER_SIZE + middle_len].copy_from_slice(&middle);  // [middle] - reconstructed middle with [first_8_bytes][middle_segment][hcd_section]
    let ps4_len = ps4.len();  // Store the length in a local variable
    ps4[ps4_len - 1] = z_byte;  // [z_byte] - last byte

    // Final sanity checks: #SAV should be at 0x20 (start of sav_header in PS4 format) and at 0xA0 (0x20 + 0x80)
    if !marker::has_magic_at(&ps4, constants::MD5_HEADER_SIZE) {  // Check at 0x20
        return Err(anyhow::anyhow!("v2 unpack produced PS4 without #SAV at 0x20."));
    }
    if !marker::has_magic_at(&ps4, constants::MD5_HEADER_SIZE + constants::SAV_HEADER_SIZE) {  // Check at 0xA0
        return Err(anyhow::anyhow!("v2 unpack produced PS4 without #SAV at 0xA0."));
    }

    Ok(ps4)
}
