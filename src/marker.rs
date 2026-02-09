use crate::constants::*;

pub fn has_dual_magic(data: &[u8]) -> bool {
    data.len() >= 0xA4  // Need at least 0xA4 bytes to check both positions
        && data[0x20] == MAGIC[0]  // Check for '#SAV' at offset 0x20 (start of A section in PS4 format)
        && data[0x21] == MAGIC[1]
        && data[0x22] == MAGIC[2]
        && data[0x23] == MAGIC[3]
        && data[0xA0] == MAGIC[0]  // Check for '#SAV' at offset 0xA0 (A section in PS4 format)
        && data[0xA1] == MAGIC[1]
        && data[0xA2] == MAGIC[2]
        && data[0xA3] == MAGIC[3]
}

pub fn has_magic_at(data: &[u8], offset: usize) -> bool {
    if offset + 4 > data.len() {
        return false;
    }
    // Check for '#SAV' magic at the specified offset
    data[offset] == MAGIC[0]
        && data[offset + 1] == MAGIC[1]
        && data[offset + 2] == MAGIC[2]
        && data[offset + 3] == MAGIC[3]
}

pub fn make_marker(version: u8, leftovers_flag: u8) -> [u8; 8] {
    [
        MARK0, MARK1, MARK2, MARK3, MARK4,
        leftovers_flag,
        MARK6,
        version
    ]
}

pub fn has_any_marker_at_08(d: &[u8]) -> bool {
    try_read_marker(d).is_some()
}

pub fn try_read_marker(d: &[u8]) -> Option<(u8, u8)> {
    if d.len() < MARKER_OFFSET + 8 {
        return None;
    }

    let o = MARKER_OFFSET;
    if d[o] != MARK0 { return None; }
    if d[o + 1] != MARK1 { return None; }
    if d[o + 2] != MARK2 { return None; }
    if d[o + 3] != MARK3 { return None; }
    if d[o + 4] != MARK4 { return None; }
    if d[o + 6] != MARK6 { return None; }

    let leftovers_flag = d[o + 5];
    let version = d[o + 7];

    let flag_ok = leftovers_flag == FLAG_NO_LEFTOVERS || leftovers_flag == FLAG_LEFTOVERS;
    let ver_ok = version == VER_V2;

    if flag_ok && ver_ok {
        Some((version, leftovers_flag))
    } else {
        None
    }
}


pub fn looks_like_v2(d: &[u8]) -> bool {
    if let Some((ver, _)) = try_read_marker(d) {
        if ver != VER_V2 { return false; }

        // v2: [SAV_HEADER] section starts at EOF-PREFIX_SIZE-HEADER_SIZE, [Z_BYTE] is at EOF-PREFIX_SIZE-HEADER_SIZE-1
        if d.len() != EDITOR_SIZE { return false; }
        let a_start = d.len() - MD5_HEADER_SIZE - SAV_HEADER_SIZE;  // Position of A section in PC-ready format
        if !has_magic_at(d, a_start) { return false; }
        // Z byte must be preserved as-is, so we accept any value
        return true;
    }
    false
}