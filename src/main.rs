#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
use biblio_ui::{APP_ID, root};
#[cfg(target_os = "linux")]
use nsh_platform_linux as platform;
#[cfg(target_os = "macos")]
use nsh_platform_macos as platform;
#[cfg(target_os = "windows")]
use nsh_platform_windows as platform;

fn main() -> Result<std::process::ExitCode, Box<dyn std::error::Error>> {
    let dir = std::env::var_os("BIBLIO_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::config_dir().map(|base| base.join(APP_ID)))
        .ok_or("Cannot locate the application configuration directory")?;
    let root = root(&dir)?;
    let status = platform::run_app(
        platform::AppSource::Rust(root),
        platform::AppOptions {
            app_id: APP_ID.into(),
            title: "Biblio".into(),
            size: (1200, 800),
            data_dir: Some(dir.join("desktop")),
            ..Default::default()
        },
    );
    Ok(std::process::Termination::report(status))
}
