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
use std::io::{self, Read, Seek, SeekFrom, Write};
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

/// Encrypt `src` into the container at `dst`, invoking `on_progress(done,
/// total)` after each chunk so a long encryption of a multi-GB file can drive
/// a UI progress bar. `total` is the source file's byte length (stat'd once);
/// `done` is plaintext bytes consumed so far. Pass `|_, _| {}` when progress
/// isn't needed.
pub fn wrap_with_progress(
    src: &Path,
    dst: &Path,
    key: &[u8; 32],
    on_progress: impl FnMut(u64, u64),
) -> io::Result<()> {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let enc = EncryptorBE32::from_aead(cipher, nonce.as_ref().into());

    let mut input = File::open(src)?;
    let total = input.metadata()?.len();
    let mut output = File::create(dst)?;
    // Any failure after this point leaves a partial container on disk; remove
    // it so callers never mistake a truncated file for a valid one.
    match wrap_frames(&mut input, &mut output, &nonce, enc, total, on_progress) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(dst);
            Err(e)
        }
    }
}

/// Encrypt the byte range `[offset, offset + len)` of `src` into a standalone
/// container at `dst`. Used to split an oversized upload artifact into parts
/// that each stay under the remote's per-object size cap: each part is a
/// complete, independently-decryptable `.bbx` (its own nonce, its own
/// `encrypt_last` final frame), so download just unwraps each part and
/// concatenates the plaintext. `total`/`done` reported to `on_progress` are
/// plaintext bytes within this range.
pub fn wrap_range_with_progress(
    src: &Path,
    dst: &Path,
    key: &[u8; 32],
    offset: u64,
    len: u64,
    on_progress: impl FnMut(u64, u64),
) -> io::Result<()> {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let enc = EncryptorBE32::from_aead(cipher, nonce.as_ref().into());

    let mut input = File::open(src)?;
    input.seek(SeekFrom::Start(offset))?;
    // `take(len)` bounds the frame loop to exactly this part's bytes; the read
    // hitting the limit looks like EOF, which fires `encrypt_last` — same path
    // as a whole-file wrap reaching the real EOF.
    let mut bounded = input.take(len);
    let mut output = File::create(dst)?;
    match wrap_frames(&mut bounded, &mut output, &nonce, enc, len, on_progress) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(dst);
            Err(e)
        }
    }
}

/// Inner loop of [`wrap`]: write the nonce, then the AEAD frames, then flush.
/// Split out so [`wrap`] can clean up a partial `dst` on any error. `input` is
/// a reader (a whole `File`, or a length-bounded `Take<File>` for a part).
fn wrap_frames(
    input: &mut impl Read,
    output: &mut File,
    nonce: &[u8; NONCE_LEN],
    mut enc: EncryptorBE32<XChaCha20Poly1305>,
    total: u64,
    mut on_progress: impl FnMut(u64, u64),
) -> io::Result<()> {
    output.write_all(nonce)?;

    let mut buf = vec![0u8; CHUNK];
    let mut done: u64 = 0;
    loop {
        let n = read_fill(input, &mut buf)?;
        if n == CHUNK {
            let frame = enc
                .encrypt_next(&buf[..n])
                .map_err(|e| io::Error::other(format!("encrypt_next: {e}")))?;
            output.write_all(&frame)?;
            done += n as u64;
            on_progress(done, total);
        } else {
            // Short read = EOF (and the exact-multiple case yields a final
            // empty last frame, which is valid).
            let frame = enc
                .encrypt_last(&buf[..n])
                .map_err(|e| io::Error::other(format!("encrypt_last: {e}")))?;
            output.write_all(&frame)?;
            done += n as u64;
            on_progress(done, total);
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

/// Decrypt the container at `src` and append its plaintext to the already-open
/// `output` (positioned at its current end). Used to reassemble a multi-part
/// upload: each part is unwrapped in order onto the tail of the cache file.
/// Unlike [`unwrap`], this does NOT delete `output` on error — `output` holds
/// the prior parts and is owned by the caller, which cleans up the whole file
/// if any part fails.
pub fn unwrap_append(src: &Path, output: &mut File, key: &[u8; 32]) -> io::Result<()> {
    let mut input = File::open(src)?;
    let mut nonce = [0u8; NONCE_LEN];
    input.read_exact(&mut nonce)?;

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let dec = DecryptorBE32::from_aead(cipher, nonce.as_ref().into());
    unwrap_frames(&mut input, output, dec)
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

        wrap_with_progress(&src, &enc, &key, |_, _| {}).unwrap();
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
        wrap_with_progress(&src, &enc, &[1u8; 32], |_, _| {}).unwrap();
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
        wrap_with_progress(&src, &enc, &key, |_, _| {}).unwrap();

        let mut cipher = std::fs::read(&enc).unwrap();
        let last = cipher.len() - 1;
        cipher[last] ^= 0xFF;
        std::fs::write(&enc, &cipher).unwrap();

        assert!(unwrap(&enc, &dec, &key).is_err());
        for p in [&src, &enc, &dec] {
            let _ = std::fs::remove_file(p);
        }
    }

    /// The load-bearing multi-part invariant: splitting plaintext into ranges,
    /// wrapping each range independently, then unwrap-appending the parts in
    /// order reconstructs the source byte-for-byte. A boundary off-by-one here
    /// would silently corrupt a reassembled multi-GB upload, so this pins it on
    /// a tiny fixture (part size deliberately not a multiple of the 4 MiB frame).
    fn roundtrip_split(data: &[u8], part_size: u64) {
        use std::io::Write as _;
        let key = [7u8; 32];
        let src = tmp("split-src");
        let dec = tmp("split-dec");
        std::fs::write(&src, data).unwrap();

        let total = data.len() as u64;
        let n_parts = total.div_ceil(part_size).max(1);
        let mut part_files = Vec::new();
        for i in 0..n_parts {
            let offset = i * part_size;
            let len = part_size.min(total - offset);
            let part = tmp(&format!("split-part{i}"));
            wrap_range_with_progress(&src, &part, &key, offset, len, |_, _| {}).unwrap();
            // Each part is an independent, complete container.
            let cipher = std::fs::read(&part).unwrap();
            assert!(cipher.len() >= NONCE_LEN + TAG_LEN);
            part_files.push(part);
        }

        // Reassemble by unwrap-appending each part in order.
        {
            let mut out = std::fs::File::create(&dec).unwrap();
            for part in &part_files {
                unwrap_append(part, &mut out, &key).unwrap();
            }
            out.flush().unwrap();
        }
        assert_eq!(std::fs::read(&dec).unwrap(), data, "reassembly must be byte-exact");

        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dec);
        for p in &part_files {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn split_roundtrip_multi_part_reassembles_exactly() {
        // 2.5 frames of data, split at 1 MiB → parts cross frame boundaries and
        // the final part is partial; exercises encrypt_last on every part.
        let data: Vec<u8> = (0..(2 * CHUNK + CHUNK / 2)).map(|i| (i * 31 % 251) as u8).collect();
        roundtrip_split(&data, 1024 * 1024);
    }

    #[test]
    fn split_roundtrip_part_size_exact_multiple_of_frame() {
        // Part size == one frame, total == 3 frames: every part ends exactly on
        // a frame boundary (the empty-final-frame case must still round-trip).
        let data: Vec<u8> = (0..(3 * CHUNK)).map(|i| (i % 256) as u8).collect();
        roundtrip_split(&data, CHUNK as u64);
    }

    #[test]
    fn split_roundtrip_single_part_when_data_fits() {
        // Data smaller than the part size → exactly one part, still correct.
        roundtrip_split(b"a small payload that fits in one part", 1024 * 1024);
    }
}
