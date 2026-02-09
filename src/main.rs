mod constants;
mod utils;
mod io;
mod marker;
mod conversion;

use clap::Parser;
use std::path::Path;
use anyhow::{Context, Result};

#[derive(Parser)]
#[command(name = "XV2_PS4toPC")]
#[command(about = "Converts Xenoverse 2 save files between PS4 and PC formats")]
struct Args {
    /// Input file path - PS4 save file (with 0x20 MD5 header + 0x80 #SAV section) or PC-ready save file
    input_file: String,

    /// Operation mode: ps4topc (PS4 to PC-ready), pctops4 (PC-ready to PS4), or auto (default: auto)
    #[arg(value_parser = ["ps4topc", "pctops4", "auto"])]
    mode: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mode = if let Some(m) = args.mode {
        m
    } else {
        "auto".to_string()
    };
    let input_path = args.input_file;

    if !std::path::Path::new(&input_path).exists() {
        eprintln!("Input not found: {}", input_path);
        std::process::exit(2);
    }

    use std::path::PathBuf;

    let input_path_buf = PathBuf::from(&input_path);
    let dir = input_path_buf.parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());

    let data = io::read_file_bytes(&input_path)
        .with_context(|| format!("Failed to read input file: {}", input_path))?;

    let (out_data, out_path, chosen) = if mode == "ps4topc" || (mode == "auto" && marker::has_dual_magic(&data)) {
        if !marker::has_dual_magic(&data) {
            eprintln!("Refusing to pack: '#SAV' not present at both 0x20 and 0xA0.");
            std::process::exit(1);
        }

        if data.len() != constants::PS4_SIZE {
            eprintln!("Refusing to pack: PS4 size expected 0x{:X}, got 0x{:X}.",
                     constants::PS4_SIZE, data.len());
            std::process::exit(1);
        }

        // Convert PS4 save format [MD5_HEADER][SAV_HEADER][middle][Z_BYTE] to PC-ready format [processed][Z_BYTE][SAV_HEADER][MD5_HEADER]
        let out_data = conversion::ps4_to_pcready(&data, &input_path, &dir)?;
        let output_filename = "EditorReady.sav".to_string();
        let out_path = std::path::PathBuf::from(&dir).join(output_filename).to_string_lossy().to_string();
        (out_data, out_path, "PS4→PC".to_string())
    } else if mode == "pctops4" || (mode == "auto" && marker::has_any_marker_at_08(&data)) {
        if !marker::has_any_marker_at_08(&data) {
            eprintln!("Refusing to unpack: marker not found at 0x08.");
            std::process::exit(1);
        }

        // Convert PC-ready format [processed][Z_BYTE][SAV_HEADER][MD5_HEADER] back to PS4 format [MD5_HEADER][SAV_HEADER][middle][Z_BYTE]
        let out_data = conversion::convert_auto(&data, &input_path, &dir)?;
        let output_filename = "SDATA000.DAT".to_string();
        let out_path = std::path::PathBuf::from(&dir).join(output_filename).to_string_lossy().to_string();
        (out_data, out_path, "PC→PS4".to_string())
    } else {
        eprintln!("Unknown format detected");
        std::process::exit(1);
    };

    io::write_output_file(&out_path, &out_data)?;

    println!("{} → {}", chosen, Path::new(&out_path).file_name()
        .unwrap_or(std::ffi::OsStr::new(""))
        .to_string_lossy());
    println!("Input  SHA1: {}", utils::sha1_hex(&data));
    println!("Output SHA1: {}", utils::sha1_hex(&out_data));

    Ok(())
}
