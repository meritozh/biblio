use md5::{Digest, Md5};
use reqwest::multipart::{Form, Part};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

use super::types::{
    BaiduError, CreateResponse, PrecreateResponse, SliceUploadResponse, UploadResult,
};

/// Slice size for chunked upload. We always use 32 MB — the 超级会员 (SVIP)
/// per-slice maximum. The old fixed 4 MB capped uploads at Baidu's ~2048-slice
/// block_list limit (an 8 GB file = 2065 slices died at partseq 2048 with errno
/// 31299), and 32 MB also minimises round-trips (20 GB ÷ 32 MB = 640 slices).
/// NOTE: this REQUIRES an SVIP account — 普通会员 caps at 16 MB and 普通用户 at
/// 4 MB, either of which would reject 32 MB slices. The account this app uploads
/// from is SVIP; revisit (per-tier detection via xpan/nas?method=uinfo) if that
/// ever changes. See https://pan.baidu.com/union/doc/nksg0s9vi
const SLICE_SIZE: usize = 32 * 1024 * 1024;

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

/// How many slice POSTs run concurrently. MUST be 1: Baidu's `superfile2`
/// endpoint cannot reliably handle concurrent slice uploads for a single file
/// — with ≥2 in-flight POSTs the request body is intermittently dropped,
/// yielding errno 31299 ("Invalid param"). AList hit "almost certain failure"
/// at 3 threads and resolved it (PR #5693) by enforcing sequential slices.
/// The JoinSet machinery below is kept (it works correctly with a window of
/// one) so this can be bumped if Baidu ever fixes the endpoint, but it stays
/// at 1 today. Wall-clock for large files is recovered by resume + retry, not
/// parallelism.
/// See https://github.com/AlistGo/alist/issues/5628
const SLICE_CONCURRENCY: usize = 1;

/// How many times the whole precreate→slices→create flow is re-attempted when
/// Baidu signals the upload session went stale mid-flight (uploadid expired,
/// or a block went missing at create time). Each re-attempt re-precreates and
/// resumes only the slices Baidu still reports as needed.
const MAX_SESSION_ATTEMPTS: usize = 3;

/// Baidu errno at `create` time meaning one or more slices the session
/// expected are missing server-side ("block miss in superfile2"). Recoverable
/// by re-precreating and re-uploading the missing slices.
const ERRNO_BLOCK_MISS: i32 = 31363;

/// Outcome of one precreate→slices→create session attempt that the outer
/// resume loop branches on. `Expired` is recoverable (re-precreate + resume);
/// `Fatal` is a hard failure that should surface to the user immediately.
enum SessionError {
    /// The upload session went stale (uploadid expired → HTTP 403 on a slice,
    /// or create reported a block miss). Retry the whole flow.
    Expired(String),
    /// Unrecoverable: a 4xx other than expiry, a non-retryable errno, or
    /// transient retries exhausted. Do not re-precreate.
    Fatal(BaiduError),
}

/// Per-request network timeout for the small control calls (precreate, create).
/// The default reqwest client has no timeout, so a stalled call could hang the
/// whole upload worker indefinitely.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Total timeout for a single slice POST. Slices are large (up to `SLICE_SIZE`
/// = 32 MB), so the 120 s control-call timeout would reject a slice on any
/// uplink slower than ~273 KB/s (32 MB ÷ 120 s) — re-introducing failures on
/// the slow asymmetric connections the bigger slices were meant to help. 600 s
/// tolerates down to ~55 KB/s while still bounding a truly stalled transfer.
const SLICE_TIMEOUT: Duration = Duration::from_secs(600);

/// A failed Baidu request, carrying everything the retry/classification logic
/// and the user-facing error need: the HTTP status (when the failure was a
/// non-2xx response) and a redacted message. Unlike `error_for_status()`, the
/// builder for the HTTP-status case READS THE RESPONSE BODY — Baidu returns its
/// real reason (errno + error_msg) in the 400/403 body, not in the status, so
/// discarding it leaves "HTTP 400" with no cause. The message never contains
/// the request URL (which carries `access_token`); only the body, which does
/// not.
struct RequestError {
    /// `Some` when the failure was a non-success HTTP response; `None` for a
    /// transport-level failure (connect/timeout/send) that never got a status.
    status: Option<reqwest::StatusCode>,
    /// Redacted, user-facing message (HTTP status + decoded Baidu body, or the
    /// transport error with its URL stripped).
    message: String,
}

impl RequestError {
    /// Build from a transport-level reqwest error (no HTTP response arrived).
    fn from_transport(e: reqwest::Error) -> Self {
        RequestError {
            status: e.status(),
            // `without_url` strips the access_token-bearing URL.
            message: format!("HTTP error: {}", e.without_url()),
        }
    }

    /// Build from a non-success HTTP response, consuming its body so Baidu's
    /// errno/error_msg surfaces instead of a bare status code.
    async fn from_response(resp: reqwest::Response) -> Self {
        let status = resp.status();
        // Body read can itself fail (connection dropped mid-body); fall back to
        // the status line in that case.
        let body = resp.text().await.unwrap_or_default();
        let body = body.trim();
        let message = if body.is_empty() {
            format!("HTTP {status}")
        } else {
            // Cap the body so a stray HTML error page can't flood the UI/log.
            let snippet: String = body.chars().take(300).collect();
            format!("HTTP {status} — {snippet}")
        };
        RequestError {
            status: Some(status),
            message,
        }
    }

    /// Whether retrying is worth it. Transient transport problems (timeouts,
    /// connection resets, no status) and HTTP 5xx are retryable; a 4xx is
    /// deterministic (it would just fail again) and is not.
    fn is_transient(&self) -> bool {
        match self.status {
            Some(s) => s.is_server_error(), // 5xx retryable, 4xx not
            None => true,                   // transport-level → retry
        }
    }

    fn is_forbidden(&self) -> bool {
        self.status == Some(reqwest::StatusCode::FORBIDDEN)
    }
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<RequestError> for BaiduError {
    fn from(e: RequestError) -> Self {
        BaiduError(e.message)
    }
}

/// Send a request, and on a non-success status read the body into a
/// `RequestError` (capturing Baidu's real errno/error_msg) rather than
/// discarding it via `error_for_status()`. On success, deserialize the JSON
/// body into `T`.
async fn send_and_parse<T: serde::de::DeserializeOwned>(
    req: reqwest::RequestBuilder,
) -> Result<T, RequestError> {
    let resp = req.send().await.map_err(RequestError::from_transport)?;
    if !resp.status().is_success() {
        return Err(RequestError::from_response(resp).await);
    }
    resp.json::<T>().await.map_err(RequestError::from_transport)
}

/// Run `op` up to `MAX_ATTEMPTS` times, retrying only transient failures with
/// exponential backoff, and parse a success body into `T`. `op` returns a
/// fresh `RequestBuilder` each attempt (the body is consumed on send), so
/// callers keep their inputs cloneable. A non-transient failure, or exhausting
/// the attempts, returns the last error as a redacted `BaiduError`. `label` is
/// included in the give-up log line.
async fn with_retry<T, F>(label: &str, mut op: F) -> Result<T, BaiduError>
where
    T: serde::de::DeserializeOwned,
    F: FnMut() -> reqwest::RequestBuilder,
{
    retry_loop(
        label,
        RETRY_BASE_DELAY,
        RequestError::is_transient,
        BaiduError::from,
        || send_and_parse::<T>(op()),
    )
    .await
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
/// Which long-running phase of the upload a progress tick belongs to. The
/// worker maps these to the upload-list status label ("Hashing…" /
/// "Uploading…"). Encryption happens in the worker *before* `upload_file`, so
/// that phase is reported by the worker directly, not through this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadPhase {
    /// Reading + MD5-ing the (encrypted) file to build Baidu's block_list.
    Hashing,
    /// POSTing slices to superfile2.
    Uploading,
}

/// `on_progress(phase, done_bytes, total_bytes)` is invoked during the two
/// long phases: hashing the file (to build the block_list) and uploading
/// slices. `done`/`total` are byte counts for THIS file (already-present
/// slices on a resume are counted up front for the upload phase). Called from
/// the worker to drive the upload-list progress bar; pass a no-op closure when
/// progress isn't needed.
pub async fn upload_file(
    access_token: &str,
    local_path: &Path,
    remote_path: &str,
    mut on_progress: impl FnMut(UploadPhase, i64, i64),
) -> Result<UploadResult, BaiduError> {
    // 32 MB slices (SVIP max) keep the block_list well under Baidu's ~2048-slice
    // cap — the old fixed 4 MB died at partseq 2048 on files > 8 GB.
    let slice_size = SLICE_SIZE;

    let (slice_md5s, total_size) =
        hash_file_in_slices_with_progress(local_path, slice_size, |done, total| {
            on_progress(UploadPhase::Hashing, done as i64, total as i64);
        })?;
    let total_slices = slice_md5s.len();
    let block_list_json = serde_json::to_string(&slice_md5s)
        .map_err(|e| BaiduError(format!("Failed to serialize block_list: {e}")))?;

    // Shared client carries the required `User-Agent: pan.baidu.com`; each
    // request below adds its own total timeout via `.timeout(REQUEST_TIMEOUT)`.
    let client = super::http_client()?;

    // Resume loop: a single session can go stale mid-flight (uploadid expiry →
    // 403, or a block-miss at create). On `Expired` we re-precreate — Baidu's
    // precreate returns the slice indices still needed, so we resume rather
    // than re-send everything. `Fatal` and success both exit immediately.
    // The session/slice layers report raw (done, total); inject the
    // `Uploading` phase here so they stay phase-agnostic.
    let mut on_slice_progress = |done: i64, total: i64| {
        on_progress(UploadPhase::Uploading, done, total);
    };

    let mut last_expired: Option<String> = None;
    for session in 1..=MAX_SESSION_ATTEMPTS {
        match run_upload_session(
            &client,
            access_token,
            local_path,
            remote_path,
            total_size,
            total_slices,
            slice_size,
            &block_list_json,
            &mut on_slice_progress,
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(SessionError::Fatal(e)) => return Err(e),
            Err(SessionError::Expired(why)) => {
                eprintln!(
                    "Baidu upload session {session}/{MAX_SESSION_ATTEMPTS} expired ({why}); \
                     re-precreating to resume"
                );
                last_expired = Some(why);
            }
        }
    }

    Err(BaiduError(format!(
        "Baidu upload failed after {MAX_SESSION_ATTEMPTS} session attempts (last: {})",
        last_expired.unwrap_or_else(|| "unknown".into())
    )))
}

/// One precreate → upload-needed-slices → create attempt. Returns the final
/// `UploadResult` on success, or a `SessionError` the caller branches on:
/// `Expired` to re-precreate and resume, `Fatal` to give up.
#[allow(clippy::too_many_arguments)]
async fn run_upload_session(
    client: &reqwest::Client,
    access_token: &str,
    local_path: &Path,
    remote_path: &str,
    total_size: i64,
    total_slices: usize,
    slice_size: usize,
    block_list_json: &str,
    on_progress: &mut impl FnMut(i64, i64),
) -> Result<UploadResult, SessionError> {
    // ── 1. Precreate ────────────────────────────────────────────────
    let precreate_url = format!("{PRECREATE_URL}?method=precreate&access_token={access_token}");
    let size_str = total_size.to_string();
    let precreate_form = [
        ("path", remote_path),
        ("size", size_str.as_str()),
        ("isdir", "0"),
        ("rtype", "3"), // 3 = overwrite conflicts; the UI warns before we upload
        ("autoinit", "1"),
        ("block_list", block_list_json),
    ];

    let precreate: PrecreateResponse = with_retry("precreate", || {
        client
            .post(&precreate_url)
            .timeout(REQUEST_TIMEOUT)
            .form(&precreate_form)
    })
    .await
    .map_err(SessionError::Fatal)?;

    if precreate.errno != 0 {
        return Err(SessionError::Fatal(BaiduError(format!(
            "Baidu precreate failed: errno {}",
            precreate.errno
        ))));
    }
    let uploadid = precreate
        .uploadid
        .ok_or_else(|| SessionError::Fatal(BaiduError("precreate returned no uploadid".into())))?;

    // `return_type == 2` from precreate means "rapid-uploaded" — Baidu
    // already has an identical file (matched by MD5) and no bytes need to
    // be sent. We deliberately do NOT early-return here: xpan precreate
    // doesn't hand back an fs_id on a rapid hit, and persisting a row with
    // an empty fs_id yields an undownloadable file. Instead we fall through
    // to the normal create flow below, which returns a real fs_id.

    // Which slice indices Baidu still needs. On a fresh precreate this is
    // every index 0..total_slices; on a re-precreate of a partially-uploaded
    // file it's the remaining subset — that's what makes this a resume. Baidu
    // may also return an empty list (everything already present, e.g. a rapid
    // hit), in which case we skip straight to create.
    let needed = needed_indices(&precreate.block_list, total_slices);

    // ── 2. Upload needed slices, bounded-concurrent ─────────────────
    // Slices Baidu already has (total − needed) count as done up front, so the
    // progress bar reflects true completion across a resume rather than
    // restarting at 0.
    let already_present = total_slices.saturating_sub(needed.len());
    upload_slices(
        client,
        access_token,
        local_path,
        remote_path,
        &uploadid,
        &needed,
        total_size,
        total_slices,
        slice_size,
        already_present,
        on_progress,
    )
    .await?;

    // ── 3. Create ───────────────────────────────────────────────────
    let create_url = format!("{CREATE_URL}?method=create&access_token={access_token}");
    let create_form = [
        ("path", remote_path),
        ("size", size_str.as_str()),
        ("isdir", "0"),
        ("rtype", "3"),
        ("uploadid", uploadid.as_str()),
        ("block_list", block_list_json),
    ];

    let create: CreateResponse = with_retry("create", || {
        client
            .post(&create_url)
            .timeout(REQUEST_TIMEOUT)
            .form(&create_form)
    })
    .await
    .map_err(SessionError::Fatal)?;

    if create.errno != 0 {
        // Block-miss means the session lost slices server-side — recoverable by
        // re-precreating. Any other errno is a hard failure.
        if create.errno == ERRNO_BLOCK_MISS {
            return Err(SessionError::Expired(format!(
                "create reported block miss (errno {ERRNO_BLOCK_MISS})"
            )));
        }
        return Err(SessionError::Fatal(BaiduError(format!(
            "Baidu create failed: errno {}",
            create.errno
        ))));
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

/// Resolve the set of slice indices to upload from precreate's `block_list`.
/// Baidu returns the indices it still needs; an empty list means "nothing
/// needed" (rapid hit / fully present). Indices ≥ `total_slices` are ignored
/// defensively. Result is sorted+deduped so the upload order is deterministic.
fn needed_indices(block_list: &[u32], total_slices: usize) -> Vec<usize> {
    if block_list.is_empty() {
        return Vec::new();
    }
    let mut idx: Vec<usize> = block_list
        .iter()
        .map(|&i| i as usize)
        .filter(|&i| i < total_slices)
        .collect();
    idx.sort_unstable();
    idx.dedup();
    idx
}

/// Upload the given slice indices with bounded concurrency. Reads each slice
/// from disk single-threaded (seek+read by index, which also enables
/// non-contiguous resume), then dispatches up to `SLICE_CONCURRENCY` POSTs in
/// parallel. The first `Expired`/`Fatal` aborts the remaining in-flight tasks.
#[allow(clippy::too_many_arguments)]
async fn upload_slices(
    client: &reqwest::Client,
    access_token: &str,
    local_path: &Path,
    remote_path: &str,
    uploadid: &str,
    needed: &[usize],
    total_size: i64,
    total_slices: usize,
    slice_size: usize,
    already_present: usize,
    on_progress: &mut impl FnMut(i64, i64),
) -> Result<(), SessionError> {
    use tokio::task::JoinSet;

    // Report the resume baseline immediately so the bar jumps to its true
    // starting point instead of 0 on a re-precreate.
    let report = |completed: usize, on_progress: &mut dyn FnMut(i64, i64)| {
        let done_slices = (already_present + completed) as i64;
        // Cap at total_size — the last slice is partial, so slice-count × size
        // would overshoot.
        let uploaded = (done_slices * slice_size as i64).min(total_size);
        on_progress(uploaded, total_size);
    };

    if needed.is_empty() {
        // Nothing to send (rapid hit / fully present) — surface 100%.
        report(0, on_progress);
        return Ok(());
    }
    report(0, on_progress);
    let _ = total_slices; // kept for signature symmetry / future per-slice UI

    let mut file = std::fs::File::open(local_path)
        .map_err(|e| SessionError::Fatal(BaiduError(format!("I/O error: {e}"))))?;
    let mut buf = vec![0u8; slice_size];
    let mut join_set: JoinSet<Result<(), SessionError>> = JoinSet::new();
    let mut iter = needed.iter().copied();
    let mut next = iter.next();
    let mut completed = 0usize;

    // Dispatch up to SLICE_CONCURRENCY tasks, then top up as each finishes.
    loop {
        // Fill the in-flight window.
        while join_set.len() < SLICE_CONCURRENCY {
            let Some(partseq) = next else { break };
            next = iter.next();

            // Read this slice's bytes from its byte offset. Single-threaded
            // here so disk reads stay sequential-ish and memory is bounded to
            // the in-flight window (≈ SLICE_CONCURRENCY × slice_size).
            let offset = partseq as u64 * slice_size as u64;
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| SessionError::Fatal(BaiduError(format!("I/O error: {e}"))))?;
            let mut read_total = 0usize;
            while read_total < slice_size {
                let n = file
                    .read(&mut buf[read_total..])
                    .map_err(|e| SessionError::Fatal(BaiduError(format!("I/O error: {e}"))))?;
                if n == 0 {
                    break;
                }
                read_total += n;
            }
            let slice_bytes = buf[..read_total].to_vec();

            let client = client.clone();
            let slice_url = format!(
                "{SUPERFILE_URL}?method=upload&access_token={access_token}\
                 &type=tmpfile&path={}&uploadid={}&partseq={}",
                urlencoding::encode(remote_path),
                uploadid,
                partseq,
            );
            join_set.spawn(async move {
                upload_one_slice(&client, &slice_url, slice_bytes, partseq).await
            });
        }

        // Nothing left to dispatch and nothing in flight → done.
        let Some(joined) = join_set.join_next().await else {
            break;
        };
        match joined {
            Ok(Ok(())) => {
                // One more slice landed; report cumulative progress.
                completed += 1;
                report(completed, on_progress);
            }
            Ok(Err(e)) => {
                join_set.abort_all();
                return Err(e);
            }
            Err(join_err) => {
                join_set.abort_all();
                return Err(SessionError::Fatal(BaiduError(format!(
                    "slice task join error: {join_err}"
                ))));
            }
        }
    }

    Ok(())
}

/// POST one slice, with transient-retry and status-aware error classification.
/// Transient transport failures and 5xx are retried in place; an HTTP 403
/// means the uploadid expired (→ `Expired`, caller re-precreates); any other
/// non-transient failure is `Fatal`. The multipart form is rebuilt per attempt
/// since it consumes its bytes; re-sending a slice is idempotent on Baidu's
/// side (keyed by uploadid+partseq).
async fn upload_one_slice(
    client: &reqwest::Client,
    slice_url: &str,
    slice_bytes: Vec<u8>,
    partseq: usize,
) -> Result<(), SessionError> {
    // `RequestError` carries the HTTP status and a redacted message (the slice
    // body, not the token-bearing URL). `retry_loop` logs it via Display, so
    // the access_token never reaches a log line. A 403 isn't transient, so it
    // propagates out of the retry as the final error; we then map it to
    // `Expired` (uploadid stale → re-precreate). On a non-success status,
    // `send_and_parse` reads Baidu's body so a slice errno/msg surfaces.
    let resp: SliceUploadResponse = retry_loop(
        &format!("slice {partseq}"),
        RETRY_BASE_DELAY,
        RequestError::is_transient,
        |e| e,
        || {
            let form = Form::new().part(
                "file",
                Part::bytes(slice_bytes.clone()).file_name(format!("part{partseq}")),
            );
            send_and_parse::<SliceUploadResponse>(
                client.post(slice_url).timeout(SLICE_TIMEOUT).multipart(form),
            )
        },
    )
    .await
    .map_err(|e| {
        if e.is_forbidden() {
            SessionError::Expired(format!("slice {partseq} got HTTP 403 (uploadid expired)"))
        } else {
            SessionError::Fatal(BaiduError::from(e))
        }
    })?;

    if let Some(errno) = resp.errno {
        if errno != 0 {
            return Err(SessionError::Fatal(BaiduError(format!(
                "superfile2 upload failed at partseq {partseq}: errno {errno} — {}",
                resp.error_msg.unwrap_or_default()
            ))));
        }
    }
    if resp.md5.is_none() {
        return Err(SessionError::Fatal(BaiduError(format!(
            "superfile2 returned no md5 for partseq {partseq}"
        ))));
    }
    Ok(())
}

/// Hash a file in fixed-size slices, returning the per-slice MD5s (hex)
/// in order and the total byte count. The per-slice MD5s form Baidu's
/// `block_list`; Baidu recomputes the overall MD5 from them at create
/// time, so we don't need to send a whole-file MD5 separately.
/// Hash a file in slices, invoking `on_progress(done, total)` after each
/// hashed block. Hashing a multi-GB file reads it whole and costs minutes, so
/// the worker drives a "Hashing… %" bar from this. `total` is stat'd up front
/// for the denominator. Pass `|_, _| {}` when progress isn't needed.
pub(super) fn hash_file_in_slices_with_progress(
    path: &Path,
    slice_size: usize,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(Vec<String>, i64), BaiduError> {
    let mut file = std::fs::File::open(path)?;
    let total_size = file.metadata()?.len();
    let mut buf = vec![0u8; slice_size];
    let mut hashes = Vec::new();
    let mut total: i64 = 0;

    loop {
        let mut read_total = 0usize;
        while read_total < slice_size {
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
        on_progress(total as u64, total_size);

        if read_total < slice_size {
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

    /// Small slice size for exercising multi-slice hashing on tiny fixtures.
    /// Production always uploads at `SLICE_SIZE` (32 MB).
    const SLICE_4MB: usize = 4 * 1024 * 1024;

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
    fn upload_slice_size_is_32mb_and_keeps_block_list_under_cap() {
        // SVIP 32 MB slices: even the 20 GB tier cap stays far under Baidu's
        // ~2048-slice block_list limit, and an 8 GB file is 256 slices (was 2065
        // at 4 MB, which died at partseq 2048).
        const GB: u64 = 1024 * 1024 * 1024;
        assert_eq!(SLICE_SIZE, 32 * 1024 * 1024);
        assert_eq!((8 * GB).div_ceil(SLICE_SIZE as u64), 256);
        assert!((20 * GB).div_ceil(SLICE_SIZE as u64) <= 2048);
    }

    #[test]
    fn needed_indices_empty_block_list_means_nothing_to_upload() {
        // Rapid-upload hit / fully-present file: Baidu returns no needed blocks.
        assert_eq!(needed_indices(&[], 10), Vec::<usize>::new());
    }

    #[test]
    fn needed_indices_fresh_precreate_returns_all() {
        // A fresh precreate lists every slice index.
        assert_eq!(needed_indices(&[0, 1, 2, 3], 4), vec![0, 1, 2, 3]);
    }

    #[test]
    fn needed_indices_resume_returns_only_remaining_subset() {
        // Re-precreate of a partially-uploaded file: only the missing slices.
        assert_eq!(needed_indices(&[1, 3], 5), vec![1, 3]);
    }

    #[test]
    fn needed_indices_sorts_dedups_and_drops_out_of_range() {
        // Defensive: unordered, duplicated, and out-of-range indices are
        // normalized to a sorted unique in-range set.
        assert_eq!(needed_indices(&[3, 1, 1, 9, 3], 5), vec![1, 3]);
    }

    #[test]
    fn hash_empty_file_returns_empty_block_md5() {
        let tmp = tempfile_with_contents(&[]);
        let (hashes, size) = hash_file_in_slices_with_progress(&tmp, SLICE_4MB, |_, _| {}).unwrap();
        assert_eq!(size, 0);
        // MD5 of zero bytes — Baidu accepts this placeholder for empty files.
        assert_eq!(hashes, vec!["d41d8cd98f00b204e9800998ecf8427e".to_string()]);
    }

    #[test]
    fn hash_small_file_single_block() {
        let tmp = tempfile_with_contents(b"hello world");
        let (hashes, size) = hash_file_in_slices_with_progress(&tmp, SLICE_4MB, |_, _| {}).unwrap();
        assert_eq!(size, 11);
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes[0], "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn hash_multi_block_file_produces_per_slice_md5s() {
        // Generate 10 MB of deterministic bytes (> 2 slices of 4 MB).
        let data: Vec<u8> = (0..10 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        let tmp = tempfile_with_contents(&data);
        let (hashes, size) = hash_file_in_slices_with_progress(&tmp, SLICE_4MB, |_, _| {}).unwrap();
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
