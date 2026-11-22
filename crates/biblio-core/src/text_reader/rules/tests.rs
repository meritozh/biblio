use super::*;
use crate::text_reader::book::{Encoding, ReadError, ReadFrom, TextBook};
use std::io::Write;
use tokio_util::sync::CancellationToken;

fn book(bytes: &[u8], encoding: Encoding) -> TextBook {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(bytes).unwrap();
    TextBook::open(file, Some(encoding), &CancellationToken::new()).unwrap()
}

fn draft(mode: PatternMode, chapter: &str, volume: Option<&str>) -> RuleDraft {
    RuleDraft {
        name: "My rule".into(),
        mode,
        chapter: chapter.into(),
        volume: volume.map(str::to_string),
    }
}

fn preset(id: &str) -> ChapterRule {
    let rule = builtin_rules().into_iter().find(|r| r.id == id).unwrap();
    ChapterRule::compile(&rule.draft).unwrap()
}

#[test]
fn surveys_all_presets_and_prefers_explicit_headings() {
    let text = "第1卷 起点\n第一章\n第十二节 山雨\n第一回\n第一话 月落\nChapter 12\nSection 2\n\
                1\n二\n３\n肆\n5、归来\n6. 下一站\n【序幕】\n★终章★";
    let book = book(text.as_bytes(), Encoding::Utf8);
    let stop = CancellationToken::new();
    let custom = SavedRule {
        id: "custom".into(),
        draft: draft(PatternMode::Regex, ".+", None),
    };
    let result = survey(&book, &[custom], &stop).unwrap();
    assert_eq!(result.detected.as_deref(), Some("common"));
    let expected = [
        ("auto", 2, "第一章"),
        ("common", 2, "第一章"),
        ("classic", 2, "第一回"),
        ("english", 2, "Chapter 12"),
        ("digits", 4, "1"),
        ("number-title", 2, "5、归来"),
        ("symbols", 2, "【序幕】"),
        ("custom", 15, "第1卷 起点"),
    ];
    assert_eq!(result.cards.len(), expected.len());
    for (card, (id, count, example)) in result.cards.iter().zip(expected) {
        let stats = card.stats.as_ref().unwrap();
        assert_eq!((&*card.id, stats.chapters), (id, count));
        assert_eq!(
            stats
                .example
                .as_ref()
                .unwrap()
                .resolve(&book, &stop)
                .unwrap(),
            example
        );
    }
}

#[test]
fn detection_uses_counts_within_groups_and_ignores_volume_only_rules() {
    let stop = CancellationToken::new();
    for (text, expected) in [
        ("第一章\nEpisode 1\npart 2\n３\n４\n５", Some("english")),
        ("第一卷\n1\n2、远方", Some("digits")),
        ("第一卷\n1\n2、远方\n三 归来", Some("number-title")),
        ("第一卷\nSome ordinary prose.", None),
    ] {
        let book = book(text.as_bytes(), Encoding::Utf8);
        let result = survey(&book, &builtin_rules(), &stop).unwrap();
        assert_eq!(result.detected.as_deref(), expected);
        assert_eq!(result.cards.len(), 7);
        if expected.is_none() {
            assert_eq!(
                result.cards[0].stats.as_ref().unwrap(),
                &RuleStats::default()
            );
            let common = result.cards[1].stats.as_ref().unwrap();
            assert_eq!(common.chapters, 0);
            assert_eq!(
                common
                    .example
                    .as_ref()
                    .unwrap()
                    .resolve(&book, &stop)
                    .unwrap(),
                "第十二章 山雨"
            );
        }
    }
}

#[test]
fn preset_probes_recognize_complete_headings() {
    for (id, positives, negatives) in [
        (
            "common",
            &["第一章", "　　第〇节 正文"][..],
            &["读到第一章", "第一回"][..],
        ),
        (
            "classic",
            &["第一回", "第兩百話 相逢"],
            &["写在第一回", "第一章"],
        ),
        (
            "english",
            &[
                "Chapter 12",
                "section 1",
                " PART 3 — Tea",
                "Episode 2: Home",
            ],
            &["see Chapter 12", "Chapter 12th", "Partition 2"],
        ),
        (
            "digits",
            &[" 01 ", "壹佰貳拾", "两千零一"],
            &["前十二", "三人", "12.5"],
        ),
        (
            "number-title",
            &["12. A title", "一、名", "１２：名", "2\tA"],
            &["1.", "第十二章", "2人同行"],
        ),
        (
            "symbols",
            &["【序章】", "☆ 远方 ☆", "[Prologue]"],
            &["【序章", "★远方☆", "【 】", "提到【序章】"],
        ),
    ] {
        let rule = preset(id);
        for line in positives {
            assert_eq!(
                rule.probe(line).unwrap(),
                Some(Heading::Chapter),
                "{id}: {line}"
            );
        }
        for line in negatives {
            assert_eq!(rule.probe(line).unwrap(), None, "{id}: {line}");
        }
    }
}

#[test]
fn template_and_regex_probe_agree_with_outline() {
    let stop = CancellationToken::new();
    let lines = [
        "  第[壹拾]章.\t清晨",
        " 第[2]章.  尾声",
        " 第[〇]章.  ",
        " 第[两]章X  清晨",
        "正文 第[3]章. 清晨",
    ];
    let text = lines.join("\r\n");
    let book = book(text.as_bytes(), Encoding::Utf8);
    let drafts = [
        draft(
            PatternMode::Template,
            " 第[{序号}]章. {标题}",
            Some(" 第[{序号}]章. 尾声"),
        ),
        draft(
            PatternMode::Regex,
            r"^[ \t]+第\[(?:壹拾|2)\]章\.(?=[ \t]+\S)",
            Some(r"(?<=章\.)[ \t]+尾声$"),
        ),
    ];
    for input in drafts {
        let rule = ChapterRule::compile(&input).unwrap();
        let expected = [
            Some(Heading::Chapter),
            Some(Heading::Volume),
            None,
            None,
            None,
        ];
        for (line, heading) in lines.into_iter().zip(expected) {
            assert_eq!(rule.probe(line).unwrap(), heading);
            assert_eq!(rule.probe(&format!("{line}\r\n")).unwrap(), heading);
        }
        let contents = outline(&book, &rule, &stop).unwrap();
        assert_eq!(contents.chapters, 1);
        assert_eq!(contents.entries.len(), 2);
        assert_eq!(contents.entries[0].kind, ContentKind::Chapter);
        assert_eq!(contents.entries[1].kind, ContentKind::Volume);
        for (entry, line) in contents.entries.iter().zip(lines) {
            assert_eq!(entry.title.resolve(&book, &stop).unwrap(), line);
        }
        assert_eq!(
            rule.probe("line one\nline two"),
            Err(RuleError::MultilineSample)
        );
    }
}

#[test]
fn validates_the_offending_field_and_enforces_compile_limits() {
    for (mode, source) in [
        (PatternMode::Template, "第{作者}章"),
        (PatternMode::Template, "未闭合{标题"),
        (PatternMode::Regex, "["),
        (PatternMode::Regex, r"\p{L}{50000}"),
        (PatternMode::Template, "  "),
        (PatternMode::Regex, "a\r\nb"),
    ] {
        for field in [Heading::Chapter, Heading::Volume] {
            let input = match field {
                Heading::Chapter => draft(mode, source, None),
                Heading::Volume => draft(mode, "chapter", Some(source)),
            };
            assert!(
                matches!(ChapterRule::compile(&input), Err(RuleError::Expression { field: got, .. }) if got == field),
                "{input:?}"
            );
        }
    }
    assert!(ChapterRule::compile(&draft(PatternMode::Regex, "", None)).is_err());
    assert!(ChapterRule::compile(&draft(PatternMode::Regex, "chapter", Some(""))).is_ok());
}

#[test]
fn outline_preserves_front_matter_volumes_and_counts() {
    let text = "\r\n\t\r\n题　记 👩‍💻\r\n第一卷 卷名\r卷首注\n第一章 春\r\n \ta e\u{301} 🦀 \u{a0}\n\
                第二节 夏\n第两卷 秋\n尾声";
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let book = book(&bytes, Encoding::Utf16Le);
    let stop = CancellationToken::new();
    let contents = outline(&book, &preset("common"), &stop).unwrap();
    let expected = [
        (ContentKind::FrontMatter, "扉页", 5),
        (ContentKind::Volume, "第一卷 卷名", 3),
        (ContentKind::Chapter, "第一章 春", 4),
        (ContentKind::Chapter, "第二节 夏", 0),
        (ContentKind::Volume, "第两卷 秋", 2),
    ];
    assert_eq!(contents.chapters, 2);
    assert_eq!(contents.entries.len(), expected.len());
    for (entry, (kind, title, words)) in contents.entries.iter().zip(expected) {
        let prefix = if kind == ContentKind::FrontMatter {
            ""
        } else {
            text.split_once(title).unwrap().0
        };
        let at = (2 + prefix.encode_utf16().count() * 2) as u64;
        assert_eq!(
            (
                entry.kind,
                entry.anchor.paragraph,
                entry.anchor.restart,
                entry.words
            ),
            (kind, at, at, words)
        );
        assert_eq!(entry.title.resolve(&book, &stop).unwrap(), title);
    }
}

#[test]
fn unmatched_book_remains_readable() {
    let stop = CancellationToken::new();
    for text in ["", "\n\n", "序言\n\n正文"] {
        let book = book(text.as_bytes(), Encoding::Utf8);
        let contents = outline(&book, &preset("common"), &stop).unwrap();
        assert_eq!(contents.chapters, 0);
        assert!(contents.entries.is_empty());
        let view = book.window(ReadFrom::Resume(None), false, &stop).unwrap();
        let source: String = view
            .rows
            .iter()
            .flat_map(|r| [r.text.as_str(), r.ending.as_str()])
            .collect();
        assert_eq!(source, text);
    }
    let book = book(b" \n\t\n1", Encoding::Utf8);
    let contents = outline(&book, &preset("digits"), &stop).unwrap();
    assert_eq!(contents.entries.len(), 1);
    assert_eq!(contents.entries[0].kind, ContentKind::Chapter);
}

#[test]
fn runtime_budget_errors_stay_on_the_failing_rule() {
    let stop = CancellationToken::new();
    let bad = SavedRule {
        id: "costly".into(),
        draft: draft(PatternMode::Regex, r"^(a|aa)+\1$", None),
    };
    let rule = ChapterRule::compile(&bad.draft).unwrap();
    let line = format!("{}!", "a".repeat(48));
    let error = RuleError::MatchBudget {
        field: Heading::Chapter,
    };
    assert_eq!(rule.probe(&line), Err(error.clone()));
    let book = book(format!("第一章\n{line}\n第二章").as_bytes(), Encoding::Utf8);
    let previous = outline(&book, &preset("common"), &stop).unwrap();
    assert!(matches!(outline(&book, &rule, &stop), Err(ScanError::Rule(e)) if e == error));
    let result = survey(&book, &[bad], &stop).unwrap();
    assert_eq!(result.cards.last().unwrap().stats, Err(error));
    assert_eq!(result.detected.as_deref(), Some("common"));
    assert_eq!(
        result.cards[0].stats.as_ref().unwrap().chapters,
        previous.chapters
    );
    assert_eq!(previous.chapters, 2);
}

#[test]
fn matches_chunked_lines_once_and_resolves_long_titles_on_demand() {
    let stop = CancellationToken::new();
    let title = format!("  start{}end", "名🦀".repeat(32_000));
    let body = " x ".repeat(80_000);
    let book = book(
        format!("{body}\r\n{title}\r\n{body}").as_bytes(),
        Encoding::Utf8,
    );
    let custom = SavedRule {
        id: "long".into(),
        draft: draft(PatternMode::Regex, "^  start.*end$", None),
    };
    let rule = ChapterRule::compile(&custom.draft).unwrap();
    let result = survey(&book, &[custom], &stop).unwrap();
    let stats = result.cards.last().unwrap().stats.as_ref().unwrap();
    assert_eq!(stats.chapters, 1);
    let sample = stats.example.as_ref().unwrap();
    assert!(matches!(sample, Title::Source(_)));
    assert_eq!(sample.resolve(&book, &stop).unwrap(), title);
    let contents = outline(&book, &rule, &stop).unwrap();
    assert_eq!(contents.entries.len(), 2);
    assert_eq!(contents.entries[0].words, 80_000);
    let chapter = &contents.entries[1];
    assert_eq!(chapter.words, 80_000);
    assert_eq!(chapter.anchor.restart, (body.len() + 2) as u64);
    assert_eq!(chapter.title, *sample);
    assert_eq!(chapter.title.resolve(&book, &stop).unwrap(), title);
    stop.cancel();
    assert!(matches!(
        sample.resolve(&book, &stop),
        Err(ReadError::Cancelled)
    ));
    assert!(matches!(
        survey(&book, &[], &stop),
        Err(ScanError::Read(ReadError::Cancelled))
    ));
    assert!(matches!(
        outline(&book, &rule, &stop),
        Err(ScanError::Read(ReadError::Cancelled))
    ));
}

#[test]
fn scans_original_blank_lines_across_collapsed_windows() {
    let stop = CancellationToken::new();
    let text = format!("Opening\n{}Last", "\t\n".repeat(8192));
    let book = book(text.as_bytes(), Encoding::Utf8);
    let mut view = book.window(ReadFrom::Resume(None), true, &stop).unwrap();
    let mut crossed_empty_window = false;
    while let Some(next) = view.after {
        view = book.window(ReadFrom::After(next), true, &stop).unwrap();
        crossed_empty_window |= view.rows.is_empty() && view.after.is_some();
    }
    assert!(crossed_empty_window);
    let custom = SavedRule {
        id: "tabs".into(),
        draft: draft(PatternMode::Regex, r"^\t$", None),
    };
    let rule = ChapterRule::compile(&custom.draft).unwrap();
    let result = survey(&book, &[custom], &stop).unwrap();
    assert_eq!(
        result
            .cards
            .last()
            .unwrap()
            .stats
            .as_ref()
            .unwrap()
            .chapters,
        8192
    );
    let contents = outline(&book, &rule, &stop).unwrap();
    assert_eq!(contents.chapters, 8192);
    assert_eq!(contents.entries[0].words, 7);
    assert_eq!(contents.entries[1].anchor.restart, 8);
    assert_eq!(contents.entries.last().unwrap().words, 4);
}
