//! Independently bounded, checksummed Zstandard frames. Encoding is not accounting.
use crate::dto::operation_error;
use anyhow::{Result, ensure};
use serde::{Serialize, de::DeserializeOwned};

pub(super) const TARGET: usize = 256 * 1024;
pub(super) const MAX: usize = 4 * 1024 * 1024;
pub(super) const VERSION: i64 = 1;

pub(super) fn encode<T: Serialize + ?Sized>(value: &T) -> Result<(Vec<u8>, usize, String)> {
    let raw = serde_json::to_vec(value)?;
    ensure!(raw.len() <= MAX, "index block exceeds decoded size limit");
    let digest = crate::hash(&raw);
    Ok((zstd::bulk::compress(&raw, 3)?, raw.len(), digest))
}

pub(super) fn decode<T: DeserializeOwned>(
    data: &[u8],
    size: i64,
    digest: &str,
    version: i64,
) -> Result<T> {
    if version != VERSION {
        return Err(operation_error(
            "INDEX_UNSUPPORTED_VERSION",
            "Unsupported index block codec",
        ));
    }
    decode_current(data, size, digest).map_err(|error| {
        operation_error("INDEX_UNAVAILABLE", format!("Invalid index block: {error}"))
    })
}
fn decode_current<T: DeserializeOwned>(data: &[u8], size: i64, digest: &str) -> Result<T> {
    ensure!(
        size > 0 && size <= MAX as i64 && data.len() <= MAX + 65536,
        "invalid index block bounds"
    );
    let raw = zstd::bulk::decompress(data, size as usize)?;
    ensure!(
        raw.len() == size as usize && crate::hash(&raw) == digest,
        "index block checksum mismatch"
    );
    Ok(serde_json::from_slice(&raw)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_numbers_unicode_and_corrupt_frames() {
        let value = serde_json::json!({"n":u64::MAX,"f":1.23456789012345e-250,"s":"中文😀"});
        let (data, size, digest) = encode(&value).unwrap();
        assert_eq!(
            decode::<serde_json::Value>(&data, size as i64, &digest, VERSION).unwrap(),
            value
        );
        assert!(decode::<serde_json::Value>(&data, size as i64, &digest, 99).is_err());
        assert!(decode::<serde_json::Value>(&data, size as i64, "bad", VERSION).is_err());
        assert!(decode::<serde_json::Value>(&data, MAX as i64 + 1, &digest, VERSION).is_err());
        assert!(
            decode::<serde_json::Value>(&data[..data.len() / 2], size as i64, &digest, VERSION)
                .is_err()
        );
    }
}
