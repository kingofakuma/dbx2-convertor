use sha1::{Sha1, Digest};
use std::fmt::Write;

pub fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    let hash = hasher.finalize();
    
    let mut result = String::with_capacity(40);
    for byte in hash {
        write!(result, "{:02x}", byte).unwrap();
    }
    result
}


pub fn all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| b == 0)
}


pub fn append_zeros(mut src: Vec<u8>, count: usize) -> Vec<u8> {
    if count == 0 {
        return src;
    }
    src.resize(src.len() + count, 0);
    src
}