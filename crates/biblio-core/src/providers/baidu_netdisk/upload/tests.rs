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
    assert_eq!(
        calls.load(Ordering::SeqCst),
        3,
        "should stop retrying on first success"
    );
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
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "permanent error must not be retried"
    );
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
    let path =
        std::env::temp_dir().join(format!("biblio-upload-test-{}-{}", std::process::id(), id));
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(contents).unwrap();
    f.sync_all().unwrap();
    path
}
