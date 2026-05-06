use super::types::BaiduError;

const AUTHORIZE_URL_TEMPLATE: &str =
    "https://openapi.baidu.com/oauth/2.0/authorize?response_type=token&redirect_uri=oob&scope=basic,netdisk&client_id=";

pub fn build_authorize_url(app_key: &str) -> Result<String, BaiduError> {
    let key = app_key.trim();
    if key.is_empty() {
        return Err(BaiduError("AppKey is required".into()));
    }
    Ok(format!("{AUTHORIZE_URL_TEMPLATE}{key}"))
}
