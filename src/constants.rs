// ===== Fixed sizes (known save file sizes) =====
pub const PS4_SIZE: usize = 0x12A200;  // Total size of PS4 save file (includes 0x20 byte prefix)
pub const EDITOR_SIZE: usize = 0x12A1F8;  // Total size of PC-ready save file

// ===== Layout constants =====
pub const MD5_HEADER_SIZE: usize = 0x20; // Size of the MD5 header section (first 0x20 bytes to move)
pub const SAV_HEADER_SIZE: usize = 0x80; // Size of the #SAV section (0x80 bytes with #SAV marker)
pub const MARKER_OFFSET: usize = 0x08;   // Offset where the 8-byte format marker is located

// Hero Coliseum Data start in PS4 format (absolute offset in PS4 file where HCD data begins)
// Used for aligning coliseum and mentor data sections
pub const HCD_START_PS4: usize = 0x07BCA0;

// Where PC-ready expects HCD start in output (absolute offset in converted file)
// Used for aligning coliseum and mentor data sections
pub const HCD_START_PC_READY: usize = 0x07BCB8;

// Magic bytes for identifying save sections: "#SAV"
pub const MAGIC: [u8; 4] = [0x23, 0x53, 0x41, 0x56];  // '#SAV' - identifies save data sections

// Marker signature: 58 56 32 53 41 ?? D6 ??  (spells "XV2SA" + ?? D6 ??) (?? = leftover flag, version)
// This is an 8-byte signature used to identify the save format with variable bytes for metadata
pub const MARK0: u8 = 0x58;  // 'X' - part of "XV2SA" signature (Xenoverse 2 Save Align)
pub const MARK1: u8 = 0x56;  // 'V' - part of "XV2SA" signature
pub const MARK2: u8 = 0x32;  // '2' - part of "XV2SA" signature
pub const MARK3: u8 = 0x53;  // 'S' - part of "XV2SA" signature
pub const MARK4: u8 = 0x41;  // 'A' - part of "XV2SA" signature
pub const MARK6: u8 = 0xD6;  // Fixed part of alignment signature

// leftover flags - indicate whether extra data was stored in a sidecar file
pub const FLAG_NO_LEFTOVERS: u8 = 0x54; // 'T' - no extra data in sidecar file
pub const FLAG_LEFTOVERS: u8 = 0x2B;   // '+' - extra data exists in sidecar file

// versions (last marker byte)
pub const VER_V2: u8 = 0x31;
