use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

pub const HASH_BUFFER_SIZE: usize = 64 * 1024; // 64 KB

pub fn compute_file_sha256(path: &Path) -> std::io::Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(HASH_BUFFER_SIZE, file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; HASH_BUFFER_SIZE];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_streaming_hasher() {
        let temp_dir = tempfile::tempdir().unwrap();

        // 1. Empty file
        let empty_path = temp_dir.path().join("empty.txt");
        File::create(&empty_path).unwrap();
        let empty_hash = compute_file_sha256(&empty_path).unwrap();
        // SHA-256 of empty string is e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        assert_eq!(
            empty_hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        // 2. Small file
        let small_path = temp_dir.path().join("hello.txt");
        {
            let mut f = File::create(&small_path).unwrap();
            f.write_all(b"hello world\n").unwrap();
        }
        let small_hash = compute_file_sha256(&small_path).unwrap();
        let expected_small_hash = {
            let mut h = Sha256::new();
            h.update(b"hello world\n");
            format!("{:x}", h.finalize())
        };
        assert_eq!(small_hash, expected_small_hash);

        // 3. Multi-chunk file (> 64 KB)
        let large_path = temp_dir.path().join("multi_chunk.bin");
        let data = vec![0x42u8; 150 * 1024]; // 150 KB
        {
            let mut f = File::create(&large_path).unwrap();
            f.write_all(&data).unwrap();
        }
        let streaming_hash = compute_file_sha256(&large_path).unwrap();
        let expected_large_hash = {
            let mut h = Sha256::new();
            h.update(&data);
            format!("{:x}", h.finalize())
        };
        assert_eq!(streaming_hash, expected_large_hash);
    }
}
