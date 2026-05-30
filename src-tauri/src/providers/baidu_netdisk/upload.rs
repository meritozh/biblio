use md5::{Digest, Md5};
use reqwest::multipart::{Form, Part};
use std::io::Read;
use std::path::Path;

use super::types::{
    BaiduError, CreateResponse, PrecreateResponse, SliceUploadResponse, UploadResult,
};

/// Block size for chunked upload. Baidu's free tier caps at 4 MB per
/// slice; VIP tiers allow 16 MB (and SVIP 32 MB), but detecting the tier
/// requires an extra `xpan/nas?method=uinfo` round-trip and isn't worth
/// the complexity for biblio's use case — 4 MB is accepted everywhere.
const SLICE_SIZE: usize = 4 * 1024 * 1024;

const PRECREATE_URL: &str = "https://pan.baidu.com/rest/2.0/xpan/file";
const CREATE_URL: &str = "https://pan.baidu.com/rest/2.0/xpan/file";
const SUPERFILE_URL: &str = "https://d.pcs.baidu.com/rest/2.0/pcs/superfile2";

/// Upload a local file to Baidu Netdisk under `remote_path`. Runs the
/// three-step xpan upload flow (precreate → superfile2 per slice →
/// create) and returns the persistent `fs_id` + MD5 + size.
///
/// `remote_path` must be absolute (starting with `/`) and must live under
/// a directory the user's app has write permission to (typically
/// `/apps/<your-app-name>/`).
pub async fn upload_file(
    access_token: &str,
    local_path: &Path,
    remote_path: &str,
) -> Result<UploadResult, BaiduError> {
    let (slice_md5s, total_size) = hash_file_in_slices(local_path)?;
    let block_list_json = serde_json::to_string(&slice_md5s)
        .map_err(|e| BaiduError(format!("Failed to serialize block_list: {e}")))?;

    // ── 1. Precreate ────────────────────────────────────────────────
    let client = reqwest::Client::new();
    let precreate_url = format!("{PRECREATE_URL}?method=precreate&access_token={access_token}");
    let precreate_form = [
        ("path", remote_path),
        ("size", &total_size.to_string()),
        ("isdir", "0"),
        ("rtype", "3"), // 3 = overwrite conflicts; the UI warns before we upload
        ("autoinit", "1"),
        ("block_list", &block_list_json),
    ];

    let precreate: PrecreateResponse = client
        .post(&precreate_url)
        .form(&precreate_form)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if precreate.errno != 0 {
        return Err(BaiduError(format!(
            "Baidu precreate failed: errno {}",
            precreate.errno
        )));
    }
    let uploadid = precreate
        .uploadid
        .ok_or_else(|| BaiduError("precreate returned no uploadid".into()))?;

    // `return_type == 2` from precreate means "rapid-uploaded" — Baidu
    // already has an identical file (matched by MD5) and no bytes need to
    // be sent. We deliberately do NOT early-return here: xpan precreate
    // doesn't hand back an fs_id on a rapid hit, and persisting a row with
    // an empty fs_id yields an undownloadable file. Instead we fall through
    // to the normal create flow below, which returns a real fs_id (the
    // slice loop short-circuits cheaply since Baidu already has the bytes).

    // ── 2. Upload slices ────────────────────────────────────────────
    // Baidu requires blocks be uploaded in partseq=0..N order; we stream
    // straight from disk so the whole file never lives in memory at once.
    let mut file = std::fs::File::open(local_path)?;
    let mut buf = vec![0u8; SLICE_SIZE];
    let mut partseq = 0usize;

    loop {
        let mut read_total = 0usize;
        // Read up to SLICE_SIZE bytes; short reads from std::fs are
        // possible near EOF, so loop until we hit EOF or fill the buffer.
        while read_total < SLICE_SIZE {
            let n = file.read(&mut buf[read_total..])?;
            if n == 0 {
                break;
            }
            read_total += n;
        }
        if read_total == 0 {
            break;
        }

        let slice_bytes = buf[..read_total].to_vec();
        let slice_url = format!(
            "{SUPERFILE_URL}?method=upload&access_token={access_token}\
             &type=tmpfile&path={}&uploadid={}&partseq={}",
            urlencoding::encode(remote_path),
            uploadid,
            partseq,
        );

        let form = Form::new().part(
            "file",
            Part::bytes(slice_bytes).file_name(format!("part{partseq}")),
        );

        let resp: SliceUploadResponse = client
            .post(&slice_url)
            .multipart(form)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        if let Some(errno) = resp.errno {
            if errno != 0 {
                return Err(BaiduError(format!(
                    "superfile2 upload failed at partseq {partseq}: errno {errno} — {}",
                    resp.error_msg.unwrap_or_default()
                )));
            }
        }
        if resp.md5.is_none() {
            return Err(BaiduError(format!(
                "superfile2 returned no md5 for partseq {partseq}"
            )));
        }

        partseq += 1;
        if read_total < SLICE_SIZE {
            break;
        }
    }

    // ── 3. Create ───────────────────────────────────────────────────
    let create_url = format!("{CREATE_URL}?method=create&access_token={access_token}");
    let create_form = [
        ("path", remote_path),
        ("size", &total_size.to_string()),
        ("isdir", "0"),
        ("rtype", "3"),
        ("uploadid", uploadid.as_str()),
        ("block_list", &block_list_json),
    ];

    let create: CreateResponse = client
        .post(&create_url)
        .form(&create_form)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if create.errno != 0 {
        return Err(BaiduError(format!(
            "Baidu create failed: errno {}",
            create.errno
        )));
    }

    Ok(UploadResult {
        fs_id: create
            .fs_id
            .map(|v| v.to_string().trim_matches('"').to_string())
            .unwrap_or_default(),
        md5: create.md5.unwrap_or_default(),
        size: create.size.unwrap_or(total_size),
        path: create.path.unwrap_or_else(|| remote_path.to_string()),
    })
}

/// Hash a file in fixed-size slices, returning the per-slice MD5s (hex)
/// in order and the total byte count. The per-slice MD5s form Baidu's
/// `block_list`; Baidu recomputes the overall MD5 from them at create
/// time, so we don't need to send a whole-file MD5 separately.
pub(super) fn hash_file_in_slices(path: &Path) -> Result<(Vec<String>, i64), BaiduError> {
    let mut file = std::fs::File::open(path)?;
    let mut buf = vec![0u8; SLICE_SIZE];
    let mut hashes = Vec::new();
    let mut total: i64 = 0;

    loop {
        let mut read_total = 0usize;
        while read_total < SLICE_SIZE {
            let n = file.read(&mut buf[read_total..])?;
            if n == 0 {
                break;
            }
            read_total += n;
        }
        if read_total == 0 {
            break;
        }
        total += read_total as i64;

        let mut hasher = Md5::new();
        hasher.update(&buf[..read_total]);
        hashes.push(hex_encode(&hasher.finalize()));

        if read_total < SLICE_SIZE {
            break;
        }
    }

    // Baidu's precreate requires block_list to be non-empty even for an
    // empty file — send the MD5 of zero bytes.
    if hashes.is_empty() {
        let hasher = Md5::new();
        hashes.push(hex_encode(&hasher.finalize()));
    }

    Ok((hashes, total))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn hash_empty_file_returns_empty_block_md5() {
        let tmp = tempfile_with_contents(&[]);
        let (hashes, size) = hash_file_in_slices(&tmp).unwrap();
        assert_eq!(size, 0);
        // MD5 of zero bytes — Baidu accepts this placeholder for empty files.
        assert_eq!(hashes, vec!["d41d8cd98f00b204e9800998ecf8427e".to_string()]);
    }

    #[test]
    fn hash_small_file_single_block() {
        let tmp = tempfile_with_contents(b"hello world");
        let (hashes, size) = hash_file_in_slices(&tmp).unwrap();
        assert_eq!(size, 11);
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes[0], "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn hash_multi_block_file_produces_per_slice_md5s() {
        // Generate 10 MB of deterministic bytes (> 2 slices of 4 MB).
        let data: Vec<u8> = (0..10 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        let tmp = tempfile_with_contents(&data);
        let (hashes, size) = hash_file_in_slices(&tmp).unwrap();
        assert_eq!(size, data.len() as i64);
        assert_eq!(hashes.len(), 3); // 4 MB + 4 MB + 2 MB
        // All 32-char hex.
        for h in &hashes {
            assert_eq!(h.len(), 32);
            assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    fn tempfile_with_contents(contents: &[u8]) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "biblio-upload-test-{}-{}",
            std::process::id(),
            id
        ));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(contents).unwrap();
        f.sync_all().unwrap();
        path
    }
}
