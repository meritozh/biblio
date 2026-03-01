use regex::Regex;
use std::sync::LazyLock;

static VALID_TAG_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\p{L}\p{N}\s\-_]+$").expect("Invalid regex pattern"));

static VALID_METADATA_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\p{L}\p{N}_]+$").expect("Invalid regex pattern"));

static VALID_HEX_COLOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^#[0-9A-Fa-f]{6}$").expect("Invalid regex pattern"));

fn contains_control_chars(s: &str) -> bool {
    s.chars().any(|c| c.is_control())
}

fn contains_bidirectional_override(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}'))
}

fn normalize_unicode(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn validate_tag_name(name: &str) -> Result<String, String> {
    let normalized = normalize_unicode(name);

    if normalized.is_empty() {
        return Err("Tag name cannot be empty".to_string());
    }

    if normalized.len() > 50 {
        return Err("Tag name must be 50 characters or less".to_string());
    }

    if contains_control_chars(name) {
        return Err("Tag name contains invalid control characters".to_string());
    }

    if contains_bidirectional_override(name) {
        return Err("Tag name contains invalid bidirectional override characters".to_string());
    }

    if !VALID_TAG_NAME.is_match(&normalized) {
        return Err(
            "Tag name can only contain letters, numbers, spaces, hyphens, and underscores"
                .to_string(),
        );
    }

    Ok(normalized)
}

pub fn validate_metadata_key(key: &str) -> Result<String, String> {
    let normalized = normalize_unicode(key);

    if normalized.is_empty() {
        return Err("Metadata key cannot be empty".to_string());
    }

    if normalized.len() > 100 {
        return Err("Metadata key must be 100 characters or less".to_string());
    }

    if contains_control_chars(key) {
        return Err("Metadata key contains invalid control characters".to_string());
    }

    if contains_bidirectional_override(key) {
        return Err("Metadata key contains invalid bidirectional override characters".to_string());
    }

    if !VALID_METADATA_KEY.is_match(&normalized) {
        return Err("Metadata key can only contain letters, numbers, and underscores".to_string());
    }

    Ok(normalized)
}

pub fn validate_metadata_value(value: &str) -> Result<String, String> {
    if value.len() > 10000 {
        return Err("Metadata value must be 10,000 characters or less".to_string());
    }

    if contains_control_chars(value) {
        return Err("Metadata value contains invalid control characters".to_string());
    }

    if contains_bidirectional_override(value) {
        return Err(
            "Metadata value contains invalid bidirectional override characters".to_string(),
        );
    }

    Ok(value.trim().to_string())
}

pub fn validate_color(color: &str) -> Result<String, String> {
    if !VALID_HEX_COLOR.is_match(color) {
        return Err("Color must be a valid hex color (#RRGGBB)".to_string());
    }
    Ok(color.to_string())
}

pub fn validate_display_name(name: &str) -> Result<String, String> {
    let normalized = normalize_unicode(name);

    if normalized.is_empty() {
        return Err("Display name cannot be empty".to_string());
    }

    if normalized.len() > 255 {
        return Err("Display name must be 255 characters or less".to_string());
    }

    if contains_control_chars(name) {
        return Err("Display name contains invalid control characters".to_string());
    }

    if contains_bidirectional_override(name) {
        return Err("Display name contains invalid bidirectional override characters".to_string());
    }

    Ok(normalized)
}

pub fn validate_category_name(name: &str) -> Result<String, String> {
    let normalized = normalize_unicode(name);

    if normalized.is_empty() {
        return Err("Category name cannot be empty".to_string());
    }

    if normalized.len() > 50 {
        return Err("Category name must be 50 characters or less".to_string());
    }

    if contains_control_chars(name) {
        return Err("Category name contains invalid control characters".to_string());
    }

    if contains_bidirectional_override(name) {
        return Err("Category name contains invalid bidirectional override characters".to_string());
    }

    Ok(normalized)
}
