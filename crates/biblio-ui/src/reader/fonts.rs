//! Active families as named by the platform's text stack. System default is
//! represented separately and remains selectable if enumeration fails.
use nsh_core::prelude::use_context;
use std::sync::{Arc, OnceLock};

#[derive(Clone)]
pub(super) struct FontFamilies(pub Arc<[String]>);

pub(super) fn installed() -> Result<Arc<[String]>, String> {
    if let Some(fonts) = use_context::<FontFamilies>() {
        return Ok(fonts.0);
    }
    static FAMILIES: OnceLock<Arc<[String]>> = OnceLock::new();
    if let Some(fonts) = FAMILIES.get() {
        return Ok(fonts.clone());
    }
    let mut names = platform_families()?;
    names.retain(|name| !name.trim().is_empty());
    names.sort_unstable();
    names.dedup();
    Ok(FAMILIES.get_or_init(|| names.into()).clone())
}

#[cfg(target_os = "macos")]
fn platform_families() -> Result<Vec<String>, String> {
    use objc2_core_foundation::{CFArray, CFRetained, CFString};
    // CoreText returns an owned array whose elements are documented CFStrings.
    let names = unsafe {
        CFRetained::cast_unchecked::<CFArray<CFString>>(
            objc2_core_text::CTFontManagerCopyAvailableFontFamilyNames(),
        )
    };
    Ok(names.iter().map(|name| name.to_string()).collect())
}

#[cfg(target_os = "linux")]
fn platform_families() -> Result<Vec<String>, String> {
    use pango::prelude::*;
    Ok(pangocairo::FontMap::default()
        .list_families()
        .iter()
        .map(|family| family.name().to_string())
        .collect())
}

#[cfg(target_os = "windows")]
fn platform_families() -> Result<Vec<String>, String> {
    use windows::Win32::Graphics::DirectWrite::*;
    fn enumerate() -> windows::core::Result<Vec<String>> {
        // All interfaces own their COM references. DirectWrite fills the
        // collection out parameter and GetString receives space for its NUL.
        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let mut collection = None;
            factory.GetSystemFontCollection(&mut collection, false)?;
            let collection = collection.ok_or_else(windows::core::Error::empty)?;
            let mut result = Vec::new();
            for i in 0..collection.GetFontFamilyCount() {
                let names = collection.GetFontFamily(i)?.GetFamilyNames()?;
                let len = names.GetStringLength(0)? as usize;
                let mut name = vec![0; len + 1];
                names.GetString(0, &mut name)?;
                result.push(String::from_utf16_lossy(&name[..len]));
            }
            Ok(result)
        }
    }
    enumerate().map_err(|e| e.to_string())
}
