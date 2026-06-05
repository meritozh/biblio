use md5::{Digest, Md5};
use reqwest::multipart::{Form, Part};
use std::io::Read;
use std::path::Path;
use std::time::Duration;

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

/// How many times to retry a single transient request before giving up.
/// A 10 GB game is ~2,600 slice POSTs; without retry the cumulative odds of
/// one transient 5xx / connection reset aborting the whole upload approach
/// certainty. Each `(uploadid, partseq)` slice POST is idempotent on Baidu's
/// side, so re-sending a failed slice is safe.
const MAX_ATTEMPTS: usize = 5;

/// Base backoff; doubles each attempt (0.5s, 1s, 2s, 4s) with a cap.
const RETRY_BASE_DELAY: Duration = Duration::from_millis(500);
const RETRY_MAX_DELAY: Duration = Duration::from_secs(8);

/// Per-request network timeout. The default reqwest client has no timeout, so
/// a stalled slice POST could hang the whole upload worker indefinitely.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Whether a reqwest failure is worth retrying. Transient transport problems
/// (timeouts, connection resets mid-send, and HTTP 5xx from Baidu's servers)
/// are retryable; a 4xx (bad request, auth) is deterministic and is not — it
/// would just fail again. Errors without a status (connect/send/timeout) are
/// treated as transient.
fn is_transient(e: &reqwest::Error) -> bool {
    if e.is_timeout() || e.is_connect() || e.is_request() {
        return true;
    }
    match e.status() {
        Some(s) => s.is_server_error(), // 5xx retryable, 4xx not
        None => true,                   // no status → transport-level, retry
    }
}

/// Run `op` up to `MAX_ATTEMPTS` times, retrying only transient reqwest
/// errors with exponential backoff. `op` is re-invoked from scratch each
/// attempt (it rebuilds the request/body), so callers must keep their inputs
/// cloneable. A non-transient error, or exhausting the attempts, returns the
/// last error mapped to a `BaiduError`. `label` is included in the give-up
/// log line.
async fn with_retry<T, F, Fut>(label: &str, op: F) -> Result<T, BaiduError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, reqwest::Error>>,
{
    // `BaiduError::from` strips the token-bearing URL before stringifying.
    retry_loop(label, RETRY_BASE_DELAY, is_transient, BaiduError::from, op).await
}

/// Generic retry core: loops `op` up to `MAX_ATTEMPTS`, retrying while
/// `retryable(&err)` holds, sleeping `base_delay` (doubling, capped) between
/// attempts. `to_err` maps the operation's error into the returned error type.
/// Generic over the error so tests can drive it without constructing a real
/// `reqwest::Error` (which the dev proxy makes nondeterministic), and so the
/// base delay can be `Duration::ZERO` to avoid real sleeping.
async fn retry_loop<T, E, BE, F, Fut, R, M>(
    label: &str,
    base_delay: Duration,
    retryable: R,
    to_err: M,
    mut op: F,
) -> Result<T, BE>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    R: Fn(&E) -> bool,
    M: Fn(E) -> BE,
    BE: std::fmt::Display,
{
    let mut delay = base_delay;
    for attempt in 1..=MAX_ATTEMPTS {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                let transient = retryable(&e);
                let err = to_err(e);
                if attempt == MAX_ATTEMPTS || !transient {
                    if attempt > 1 {
                        eprintln!("Baidu {label} failed after {attempt} attempts: {err}");
                    }
                    return Err(err);
                }
                eprintln!(
                    "Baidu {label} transient failure (attempt {attempt}/{MAX_ATTEMPTS}), \
                     retrying in {}ms: {err}",
                    delay.as_millis(),
                );
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                delay = (delay * 2).min(RETRY_MAX_DELAY);
            }
        }
    }
    unreachable!("loop returns on the final attempt")
}

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
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| BaiduError(format!("Failed to build HTTP client: {}", e.without_url())))?;
    let precreate_url = format!("{PRECREATE_URL}?method=precreate&access_token={access_token}");
    let size_str = total_size.to_string();
    let precreate_form = [
        ("path", remote_path),
        ("size", size_str.as_str()),
        ("isdir", "0"),
        ("rtype", "3"), // 3 = overwrite conflicts; the UI warns before we upload
        ("autoinit", "1"),
        ("block_list", &block_list_json),
    ];

    let precreate: PrecreateResponse = with_retry("precreate", || async {
        client
            .post(&precreate_url)
            .form(&precreate_form)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    })
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

        // Retry the slice POST on transient failures. The multipart Form
        // consumes its bytes, so rebuild it (cheap clone of one 4 MB slice)
        // each attempt. Baidu keys tmpfile slices by (uploadid, partseq), so
        // re-sending the same slice is idempotent.
        let resp: SliceUploadResponse =
            with_retry(&format!("slice {partseq}"), || async {
                let form = Form::new().part(
                    "file",
                    Part::bytes(slice_bytes.clone()).file_name(format!("part{partseq}")),
                );
                client
                    .post(&slice_url)
                    .multipart(form)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await
            })
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
        ("size", size_str.as_str()),
        ("isdir", "0"),
        ("rtype", "3"),
        ("uploadid", uploadid.as_str()),
        ("block_list", &block_list_json),
    ];

    let create: CreateResponse = with_retry("create", || async {
        client
            .post(&create_url)
            .form(&create_form)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    })
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Test double: a transient or permanent error, with a Display impl so it
    /// satisfies `retry_loop`'s `BE: Display` bound. Lets the retry control
    /// flow be tested without constructing a real `reqwest::Error` (which the
    /// dev HTTP proxy makes nondeterministic).
    #[derive(Debug, PartialEq)]
    struct TestErr {
        transient: bool,
    }
    impl std::fmt::Display for TestErr {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TestErr(transient={})", self.transient)
        }
    }

    /// retry_loop retries transient errors with zero delay and stops at the
    /// first success without exhausting the budget.
    #[tokio::test]
    async fn retry_loop_recovers_from_transient_then_succeeds() {
        let calls = AtomicUsize::new(0);
        let result: Result<u32, TestErr> = retry_loop(
            "test",
            Duration::ZERO,
            |e: &TestErr| e.transient,
            |e| e,
            || {
                let n = calls.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    if n < 3 {
                        Err(TestErr { transient: true })
                    } else {
                        Ok(42)
                    }
                }
            },
        )
        .await;
        assert_eq!(result, Ok(42));
        assert_eq!(calls.load(Ordering::SeqCst), 3, "should stop retrying on first success");
    }

    /// retry_loop gives up after exactly MAX_ATTEMPTS when every attempt is
    /// transient, rather than looping forever.
    #[tokio::test]
    async fn retry_loop_gives_up_after_max_attempts() {
        let calls = AtomicUsize::new(0);
        let result: Result<u32, TestErr> = retry_loop(
            "test",
            Duration::ZERO,
            |e: &TestErr| e.transient,
            |e| e,
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                async move { Err::<u32, _>(TestErr { transient: true }) }
            },
        )
        .await;
        assert_eq!(result, Err(TestErr { transient: true }));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            MAX_ATTEMPTS,
            "should attempt exactly MAX_ATTEMPTS times before giving up"
        );
    }

    /// retry_loop does NOT retry a non-transient (permanent) error — it
    /// returns immediately on the first attempt.
    #[tokio::test]
    async fn retry_loop_does_not_retry_permanent_error() {
        let calls = AtomicUsize::new(0);
        let result: Result<u32, TestErr> = retry_loop(
            "test",
            Duration::ZERO,
            |e: &TestErr| e.transient,
            |e| e,
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                async move { Err::<u32, _>(TestErr { transient: false }) }
            },
        )
        .await;
        assert_eq!(result, Err(TestErr { transient: false }));
        assert_eq!(calls.load(Ordering::SeqCst), 1, "permanent error must not be retried");
    }

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
