//! Opaque encrypted container ("bbx1") for remote objects.
//!
//! Before a file is pushed to Baidu it is wrapped here; after download it is
//! unwrapped. The goal is to deny Baidu's three 和谐 levers at once:
//!
//! - **Hash matching / 秒传** — the bytes are ciphertext, so the per-slice
//!   MD5s Baidu records never match any known file.
//! - **Format sniffing** — the whole object is high-entropy AEAD output with
//!   no magic string, no header text, no embedded filename or extension.
//! - **Filename matching** — the remote object is stored under an opaque
//!   random token (see `random_token`), never the real name.
//!
//! On-disk layout:
//! ```text
//!   [ 19 bytes ] STREAM nonce (random)
//!   [ frames   ] XChaCha20-Poly1305 STREAM (BE32) AEAD frames,
//!                4 MiB plaintext chunks, each frame = chunk + 16-byte tag
//! ```
//! There is deliberately **no version byte in the file** — the format
//! version lives in the DB (`files.remote_container = 'bbx1'`), so the bytes
//! carry zero static signature.
//!
//! Streaming (chunked) so a multi-hundred-MB comic never loads fully into
//! RAM, matching the 4 MiB slice size the Baidu uploader already uses.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use base64::Engine;
use chacha20poly1305::{
    Key, KeyInit, XChaCha20Poly1305,
    aead::stream::{DecryptorBE32, EncryptorBE32},
};
use rand::{RngCore, rngs::OsRng};

/// Plaintext bytes per AEAD frame. Matches the Baidu uploader's slice size.
const CHUNK: usize = 4 * 1024 * 1024;
/// STREAM-BE32 nonce length for XChaCha20-Poly1305: 24-byte AEAD nonce − 5
/// bytes the STREAM construction reserves for its counter + last-block flag.
const NONCE_LEN: usize = 19;
/// Poly1305 authentication tag appended to every frame.
const TAG_LEN: usize = 16;
/// app_settings key holding the base64-encoded 32-byte container key.
const KEY_SETTING: &str = "remote_container_key";

/// Encrypt `src` into the container at `dst`.
pub fn wrap(src: &Path, dst: &Path, key: &[u8; 32]) -> io::Result<()> {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let enc = EncryptorBE32::from_aead(cipher, nonce.as_ref().into());

    let mut input = File::open(src)?;
    let mut output = File::create(dst)?;
    // Any failure after this point leaves a partial container on disk; remove
    // it so callers never mistake a truncated file for a valid one.
    match wrap_frames(&mut input, &mut output, &nonce, enc) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(dst);
            Err(e)
        }
    }
}

/// Inner loop of [`wrap`]: write the nonce, then the AEAD frames, then flush.
/// Split out so [`wrap`] can clean up a partial `dst` on any error.
fn wrap_frames(
    input: &mut File,
    output: &mut File,
    nonce: &[u8; NONCE_LEN],
    mut enc: EncryptorBE32<XChaCha20Poly1305>,
) -> io::Result<()> {
    output.write_all(nonce)?;

    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = read_fill(input, &mut buf)?;
        if n == CHUNK {
            let frame = enc
                .encrypt_next(&buf[..n])
                .map_err(|e| io::Error::other(format!("encrypt_next: {e}")))?;
            output.write_all(&frame)?;
        } else {
            // Short read = EOF (and the exact-multiple case yields a final
            // empty last frame, which is valid).
            let frame = enc
                .encrypt_last(&buf[..n])
                .map_err(|e| io::Error::other(format!("encrypt_last: {e}")))?;
            output.write_all(&frame)?;
            break;
        }
    }
    output.flush()?;
    Ok(())
}

/// Decrypt the container at `src` back into the original bytes at `dst`.
/// Returns an error (never silent corruption) on a wrong key or any
/// tampered frame — the AEAD tag is verified per frame.
pub fn unwrap(src: &Path, dst: &Path, key: &[u8; 32]) -> io::Result<()> {
    let mut input = File::open(src)?;
    let mut nonce = [0u8; NONCE_LEN];
    input.read_exact(&mut nonce)?;

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let dec = DecryptorBE32::from_aead(cipher, nonce.as_ref().into());

    let mut output = File::create(dst)?;
    // A wrong key or tampered frame errors mid-stream, leaving a partial
    // plaintext on disk; remove it so a failed decrypt never surfaces a
    // truncated file to the caller.
    match unwrap_frames(&mut input, &mut output, dec) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(dst);
            Err(e)
        }
    }
}

/// Inner loop of [`unwrap`]: decrypt AEAD frames into `output`, then flush.
/// Split out so [`unwrap`] can clean up a partial `dst` on any error.
fn unwrap_frames(
    input: &mut File,
    output: &mut File,
    mut dec: DecryptorBE32<XChaCha20Poly1305>,
) -> io::Result<()> {
    let enc_chunk = CHUNK + TAG_LEN;
    let mut buf = vec![0u8; enc_chunk];
    loop {
        let n = read_fill(input, &mut buf)?;
        if n == enc_chunk {
            let plain = dec
                .decrypt_next(&buf[..n])
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("decrypt: {e}")))?;
            output.write_all(&plain)?;
        } else {
            let plain = dec.decrypt_last(&buf[..n]).map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("decrypt_last: {e}"))
            })?;
            output.write_all(&plain)?;
            break;
        }
    }
    output.flush()?;
    Ok(())
}

/// Read into `buf` until it is full or EOF; returns the number of bytes read.
/// `File::read` can return short reads, so loop until the buffer is filled.
fn read_fill(r: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        let n = r.read(&mut buf[total..])?;
        if n == 0 {
            break;
        }
        total += n;
    }
    Ok(total)
}

/// An opaque, extension-less remote object name (128-bit hex token). Used so
/// the remote filename leaks neither the real name nor the format.
pub fn random_token() -> String {
    let mut b = [0u8; 16];
    OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Fetch the device's container key, generating it on first use.
///
/// The key is stored locally in `app_settings` (the same place the Baidu
/// access token already lives) — Baidu, the actual adversary, never receives
/// it. Surface it for backup via the Settings "recovery key" panel
/// (`remote_recovery_key`): losing it without a backup makes every encrypted
/// remote object unrecoverable.
pub async fn get_or_create_key(pool: &sqlx::SqlitePool) -> Result<[u8; 32], String> {
    if let Some((value,)) =
        sqlx::query_as::<_, (String,)>("SELECT value FROM app_settings WHERE key = ?")
            .bind(KEY_SETTING)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
    {
        return decode_key(&value);
    }

    // Insert a freshly generated key only if no row exists yet. ON CONFLICT
    // DO NOTHING makes this safe against a concurrent creator: whoever loses
    // the race leaves the existing row untouched.
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    let encoded = base64::engine::general_purpose::STANDARD.encode(key);
    sqlx::query("INSERT INTO app_settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO NOTHING")
        .bind(KEY_SETTING)
        .bind(&encoded)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Always return the persisted row, never the locally-generated bytes: a
    // concurrent caller may have won the insert, and that row is the one
    // every other reader will see.
    let (value,) = sqlx::query_as::<_, (String,)>("SELECT value FROM app_settings WHERE key = ?")
        .bind(KEY_SETTING)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    decode_key(&value)
}

/// Decode a base64-encoded container key from `app_settings` into a 32-byte array.
fn decode_key(value: &str) -> Result<[u8; 32], String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value.as_bytes())
        .map_err(|e| format!("container key is not valid base64: {e}"))?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| "container key is not 32 bytes".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn tmp(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "biblio-container-test-{}-{}-{}",
            std::process::id(),
            id,
            tag
        ))
    }

    fn roundtrip(data: &[u8]) {
        let key = [9u8; 32];
        let (src, enc, dec) = (tmp("src"), tmp("enc"), tmp("dec"));
        std::fs::write(&src, data).unwrap();

        wrap(&src, &enc, &key).unwrap();
        let cipher = std::fs::read(&enc).unwrap();
        // Container is at least nonce + tag, and never the raw plaintext.
        assert!(cipher.len() >= NONCE_LEN + TAG_LEN);
        assert_ne!(cipher.as_slice(), data);

        unwrap(&enc, &dec, &key).unwrap();
        assert_eq!(std::fs::read(&dec).unwrap(), data);

        for p in [&src, &enc, &dec] {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn roundtrip_empty() {
        roundtrip(&[]);
    }

    #[test]
    fn roundtrip_small() {
        roundtrip(b"the quick brown fox jumps over the lazy dog");
    }

    #[test]
    fn roundtrip_exact_chunk_boundary() {
        roundtrip(&vec![0xABu8; CHUNK]);
    }

    #[test]
    fn roundtrip_just_over_one_chunk() {
        let data: Vec<u8> = (0..CHUNK + 777).map(|i| (i % 251) as u8).collect();
        roundtrip(&data);
    }

    #[test]
    fn roundtrip_multi_chunk() {
        let data: Vec<u8> = (0..2 * CHUNK + 13).map(|i| (i % 251) as u8).collect();
        roundtrip(&data);
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let (src, enc, dec) = (tmp("wk_src"), tmp("wk_enc"), tmp("wk_dec"));
        std::fs::write(&src, b"secret bytes").unwrap();
        wrap(&src, &enc, &[1u8; 32]).unwrap();
        assert!(unwrap(&enc, &dec, &[2u8; 32]).is_err());
        for p in [&src, &enc, &dec] {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = [5u8; 32];
        let (src, enc, dec) = (tmp("t_src"), tmp("t_enc"), tmp("t_dec"));
        std::fs::write(&src, b"authenticated content here").unwrap();
        wrap(&src, &enc, &key).unwrap();

        let mut cipher = std::fs::read(&enc).unwrap();
        let last = cipher.len() - 1;
        cipher[last] ^= 0xFF;
        std::fs::write(&enc, &cipher).unwrap();

        assert!(unwrap(&enc, &dec, &key).is_err());
        for p in [&src, &enc, &dec] {
            let _ = std::fs::remove_file(p);
        }
    }
}
