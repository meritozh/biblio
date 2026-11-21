mod admin;
mod app;
mod artwork;
mod catalog;
mod detail;
mod filters;
mod imports;
mod search;
mod selection;
mod settings;
mod theme;
mod ui;

use nsh_core::{element::Root, prelude::*};
pub const APP_ID: &str = "io.augite.biblio";

#[derive(Debug, Clone, PartialEq, Props)]
struct DesktopProps {
    directory: Prop<SharedString>,
}

#[component]
fn Desktop(props: DesktopProps) -> impl IntoElement {
    app::start(std::path::PathBuf::from(props.directory.get().as_ref()))
}

/// Builds the same desktop window for the platform runner and headless tests.
pub fn root(directory: &std::path::Path) -> Result<Root, &'static str> {
    let path = directory
        .to_str()
        .ok_or("The configuration path is not valid UTF-8")?
        .to_owned();
    Ok(Root::new::<Desktop>(move || DesktopProps {
        directory: path.clone().into(),
    })
    .app_id(APP_ID))
}
