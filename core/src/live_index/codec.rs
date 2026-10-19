//! Independently bounded, checksummed Zstandard frames. Encoding is not accounting.
use crate::dto::operation_error;
use anyhow::{Result, ensure};
use serde::{Serialize, de::DeserializeOwned};

pub(super) const TARGET: usize = 256 * 1024;
pub(super) const MAX: usize = 4 * 1024 * 1024;
pub(super) const VERSION: i64 = 1;

/// One compression context per write pass, never one per frame or a global cache.
#[derive(Default)]
pub(super) struct Encoder(Option<zstd::bulk::Compressor<'static>>);
impl Encoder {
    pub(super) fn encode<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(Vec<u8>, usize, String)> {
        let raw = serde_json::to_vec(value)?;
        self.encode_raw(&raw)
    }
    pub(super) fn encode_raw(&mut self, raw: &[u8]) -> Result<(Vec<u8>, usize, String)> {
        ensure!(raw.len() <= MAX, "index block exceeds decoded size limit");
        let digest = crate::hash(raw);
        if self.0.is_none() {
            self.0 = Some(zstd::bulk::Compressor::new(3)?);
        }
        Ok((self.0.as_mut().unwrap().compress(raw)?, raw.len(), digest))
    }
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
    if size > 0 && size <= MAX as i64 {
        crate::usage_app::query_work::charge(0, size as usize)?;
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
    fn reused_compressor_emits_independent_current_frames() {
        let mut encoder = Encoder::default();
        for n in [1, 1024, 8192, 10] {
            let value = serde_json::json!({"payload": "字".repeat(n), "n": n});
            let (data, size, digest) = encoder.encode(&value).unwrap();
            assert_eq!(
                decode::<serde_json::Value>(&data, size as i64, &digest, VERSION).unwrap(),
                value
            );
            assert_eq!(
                data,
                zstd::bulk::compress(&serde_json::to_vec(&value).unwrap(), 3).unwrap()
            );
        }
    }
    #[test]
    fn exact_numbers_unicode_and_corrupt_frames() {
        let value = serde_json::json!({"n":u64::MAX,"f":1.23456789012345e-250,"s":"中文😀"});
        let (data, size, digest) = Encoder::default().encode(&value).unwrap();
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
