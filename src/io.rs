use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn read_file_bytes<P: AsRef<Path>>(path: P) -> Result<Vec<u8>> {
    let data = fs::read(path)?;
    Ok(data)
}

pub fn write_output_file<P: AsRef<Path>>(path: P, data: &[u8]) -> Result<()> {
    fs::write(path, data)
        .with_context(|| "Failed to write output file".to_string())?;
    Ok(())
}