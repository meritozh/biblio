use super::types::{BaiduError, FileManagerResponse};

const FILEMANAGER_URL: &str = "https://pan.baidu.com/rest/2.0/xpan/file";

/// Delete a file from Baidu Netdisk by path. The remote caller (biblio's
/// `file_delete` command) invokes this after removing the local row, so
/// failure here surfaces to the UI as "file still exists remotely" but
/// the local row is already gone.
pub async fn delete_file(access_token: &str, remote_path: &str) -> Result<(), BaiduError> {
    let filelist =
        serde_json::to_string(&[remote_path]).map_err(|e| BaiduError(format!("{e}")))?;

    let url = format!(
        "{FILEMANAGER_URL}?method=filemanager&opera=delete&access_token={access_token}"
    );

    let resp: FileManagerResponse = reqwest::Client::new()
        .post(&url)
        .form(&[("async", "0"), ("filelist", &filelist)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // errno 0 = deleted. errno -9 = file didn't exist (treat as success —
    // we're aiming for the post-state).
    if resp.errno != 0 && resp.errno != -9 {
        return Err(BaiduError(format!(
            "Baidu delete failed: errno {}",
            resp.errno
        )));
    }
    Ok(())
}
