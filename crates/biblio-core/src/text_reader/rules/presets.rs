use super::{NUMBER, PatternMode, RuleDraft, SavedRule};

pub const AUTO_ID: &str = "auto";

pub(super) struct Preset {
    pub id: &'static str,
    name: &'static str,
    chapter: &'static str,
    volume: Option<&'static str>,
    pub example: &'static str,
    pub explicit: bool,
}

const VOLUME: &str = r"^\s*(?:第\s*{序号}\s*卷|卷\s*{序号}).*$";

// Order within each group breaks a tie in chapter counts.
pub(super) const PRESETS: &[Preset] = &[
    Preset {
        id: "common",
        name: "常规中文",
        chapter: r"^\s*第\s*{序号}\s*[章节節].*$",
        volume: Some(VOLUME),
        example: "第十二章 山雨",
        explicit: true,
    },
    Preset {
        id: "classic",
        name: "回／话",
        chapter: r"^\s*第\s*{序号}\s*[回话話].*$",
        volume: Some(VOLUME),
        example: "第一回 风起",
        explicit: true,
    },
    Preset {
        id: "english",
        name: "英文章节",
        chapter: r"(?i)^\s*(?:chapter|section|part|episode)\s+{序号}(?:[\s:：.．—–-]+.*)?$",
        volume: Some(r"(?i)^\s*(?:volume|book)\s+{序号}(?:[\s:：.．—–-]+.*)?$"),
        example: "Chapter 12: The Crossing",
        explicit: true,
    },
    Preset {
        id: "digits",
        name: "纯数字",
        chapter: r"^\s*{序号}\s*$",
        volume: None,
        example: "十二",
        explicit: false,
    },
    Preset {
        id: "number-title",
        name: "数字加标题",
        chapter: r"^\s*{序号}(?:\s*[.．、:：)）—–-]\s*|\s+)\S.*$",
        volume: None,
        example: "12、归来",
        explicit: false,
    },
    Preset {
        id: "symbols",
        name: "特殊符号",
        chapter: r"^\s*(?:【.*\S.*】|《.*\S.*》|「.*\S.*」|『.*\S.*』|\[.*\S.*\]|★.*\S.*★|☆.*\S.*☆|◆.*\S.*◆)\s*$",
        volume: None,
        example: "【山雨】",
        explicit: false,
    },
];

/// Editable expressions for the six fixed presets. Automatic detection has no expression.
pub fn builtin_rules() -> Vec<SavedRule> {
    PRESETS
        .iter()
        .map(|preset| SavedRule {
            id: preset.id.into(),
            draft: RuleDraft {
                name: preset.name.into(),
                mode: PatternMode::Regex,
                chapter: preset.chapter.replace("{序号}", NUMBER),
                volume: preset
                    .volume
                    .map(|pattern| pattern.replace("{序号}", NUMBER)),
            },
        })
        .collect()
}
