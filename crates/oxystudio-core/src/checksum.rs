use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use crate::error::{Error, Result};

/// Parse a standard sha256sums text file into a map of filename -> hex hash.
pub fn parse_checksums(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Formats typically: "<hash>  <filename>" or "<hash> *<filename>"
        let mut parts = line.split_whitespace();
        let hash = match parts.next() {
            Some(h) if h.len() == 64 => h.to_lowercase(),
            _ => continue,
        };

        // Rejoin remainder for filename (handling possible spaces or leading '*')
        let filename_raw = parts.collect::<Vec<_>>().join(" ");
        let filename = filename_raw.trim_start_matches('*').trim();
        if !filename.is_empty() {
            // Also store just the file name without any directory path
            let clean_name = Path::new(filename)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(filename);
            map.insert(clean_name.to_string(), hash);
        }
    }
    map
}

pub fn calculate_bytes_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

pub fn calculate_file_sha256(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub fn verify_file(path: &Path, expected_hex: &str) -> Result<()> {
    let calculated = calculate_file_sha256(path)?;
    let expected = expected_hex.trim().to_lowercase();
    if calculated.eq_ignore_ascii_case(&expected) {
        Ok(())
    } else {
        Err(Error::ChecksumMismatch {
            file: path.display().to_string(),
            expected,
            calculated,
        })
    }
}

pub fn verify_bytes(data: &[u8], filename: &str, expected_hex: &str) -> Result<()> {
    let calculated = calculate_bytes_sha256(data);
    let expected = expected_hex.trim().to_lowercase();
    if calculated.eq_ignore_ascii_case(&expected) {
        Ok(())
    } else {
        Err(Error::ChecksumMismatch {
            file: filename.to_string(),
            expected,
            calculated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sha256_sums() {
        let sample = "
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  empty.txt
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad *test.tar.gz
";
        let map = parse_checksums(sample);
        assert_eq!(
            map.get("empty.txt"),
            Some(&"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string())
        );
        assert_eq!(
            map.get("test.tar.gz"),
            Some(&"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string())
        );
    }

    #[test]
    fn verifies_bytes() {
        let empty: &[u8] = b"";
        assert!(verify_bytes(
            empty,
            "empty.txt",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        )
        .is_ok());
    }
}
