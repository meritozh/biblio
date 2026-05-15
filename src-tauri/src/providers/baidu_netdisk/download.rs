use std::path::Path;

use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use super::types::{BaiduError, FilemetasResponse};

const FILEMETAS_URL: &str = "https://pan.baidu.com/rest/2.0/xpan/multimedia";

/// Look up the short-lived download link for a single file by its `fs_id`.
/// Baidu requires a fresh dlink per download attempt — the URL it returns
/// embeds an expiring signature, so callers should consume it immediately
/// instead of caching across retries.
pub async fn get_download_dlink(
    access_token: &str,
    fs_id: &str,
) -> Result<String, BaiduError> {
    // The fsids parameter is a JSON-encoded array of integers. fs_id values
    // are stored as strings in our DB (Baidu returns them as 64-bit numbers
    // and serde_json's Value preserves precision through string round-trip),
    // so we pass them through verbatim wrapped in `[…]`.
    let fsids = format!("[{}]", fs_id);
    let url = format!(
        "{FILEMETAS_URL}?method=filemetas&access_token={access_token}\
         &fsids={fsids}&dlink=1&thumb=0&extra=0"
    );

    let resp: FilemetasResponse = reqwest::Client::new()
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if resp.errno != 0 {
        return Err(BaiduError(format!(
            "Baidu filemetas failed: errno {}",
            resp.errno
        )));
    }

    let dlink = resp
        .list
        .into_iter()
        .find_map(|e| e.dlink)
        .ok_or_else(|| BaiduError("Baidu filemetas returned no dlink".into()))?;
    Ok(dlink)
}

/// Stream the file at `dlink` to `dest`. Writes to `<dest>.tmp` first, then
/// renames atomically on success so a partial download can never look like
/// a finished cache entry.
///
/// Baidu's dlink endpoint requires the literal `User-Agent: pan.baidu.com`;
/// other UAs get a 403 even with a valid access_token. The dlink itself
/// already encodes the user, so the access_token must still be appended as
/// a query parameter.
pub async fn download_to(
    access_token: &str,
    dlink: &str,
    dest: &Path,
) -> Result<u64, BaiduError> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let url = if dlink.contains('?') {
        format!("{dlink}&access_token={access_token}")
    } else {
        format!("{dlink}?access_token={access_token}")
    };

    let resp = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "pan.baidu.com")
        .send()
        .await?
        .error_for_status()?;

    // Buffered fetch keeps the dependency surface small (no
    // `futures_util` / streaming feature). For multi-GB files this would
    // pin one full copy in memory; if that becomes a problem, swap to
    // `bytes_stream` and add `tokio-util` / `futures_util`.
    let bytes = resp.bytes().await?;

    let tmp = dest.with_extension(format!(
        "{}.tmp",
        dest.extension().and_then(|s| s.to_str()).unwrap_or("part"),
    ));
    let mut file = File::create(&tmp).await?;
    file.write_all(&bytes).await?;
    file.flush().await?;
    file.sync_all().await?;
    drop(file);

    tokio::fs::rename(&tmp, dest).await?;
    Ok(bytes.len() as u64)
}
