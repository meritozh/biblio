mod admin;
mod app;
mod catalog;
mod detail;
mod filters;
mod imports;
mod paged;
mod reader;
mod search;
mod selection;
mod settings;
mod ui;

use nsh_core::{element::Root, prelude::*};
pub use reader::writes::ReaderWrites;
pub const APP_ID: &str = "io.augite.biblio";

#[derive(Debug, Clone, PartialEq, Props)]
struct DesktopProps {
    directory: Prop<SharedString>,
    workspace: Option<Renderer<SharedString>>,
}

#[component]
fn Desktop(props: DesktopProps) -> impl IntoElement {
    props
        .workspace
        .expect("desktop workspace")
        .render("library", props.directory.get())
        .w_full()
        .h_full()
}

/// Builds the same desktop window for the platform runner and headless tests.
pub fn root(directory: &std::path::Path) -> Result<Root, &'static str> {
    Ok(root_with_reader_writes(directory)?.0)
}

/// Keep the writer until the native event loop has returned.
pub fn root_with_reader_writes(
    directory: &std::path::Path,
) -> Result<(Root, ReaderWrites), &'static str> {
    desktop_root(directory, app::start)
}

fn desktop_root(
    directory: &std::path::Path,
    build: impl Fn(std::path::PathBuf, ReaderWrites) -> AnyElement + Send + Sync + 'static,
) -> Result<(Root, ReaderWrites), &'static str> {
    let build = std::sync::Arc::new(build);
    let path = directory
        .to_str()
        .ok_or("The configuration path is not valid UTF-8")?
        .to_owned();
    let writes = ReaderWrites::new();
    let window_writes = writes.clone();
    let root = Root::new::<Desktop>(move || {
        let writes = window_writes.clone();
        let build = build.clone();
        DesktopProps {
            directory: path.clone().into(),
            workspace: Some(Renderer::new(move |path: &SharedString| {
                build(path.as_str().into(), writes.clone())
            })),
        }
    })
    .app_id(APP_ID);
    Ok((root, writes))
}
