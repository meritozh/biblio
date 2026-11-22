use super::*;
use std::io::{Seek, SeekFrom, Write};
use tempfile::NamedTempFile;

fn fixture(
    bytes: &[u8],
    encoding: Option<Encoding>,
) -> (NamedTempFile, TextBook, CancellationToken) {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(bytes).unwrap();
    let stop = CancellationToken::new();
    let book = TextBook::open(File::open(file.path()).unwrap(), encoding, &stop).unwrap();
    (file, book, stop)
}

fn raw(window: &Window) -> String {
    let mut text = String::new();
    for row in &window.rows {
        text.push_str(&row.text);
        text.push_str(row.ending.as_str());
    }
    text
}

#[test]
fn finds_literal_across_windows_and_reveals_each_hit() {
    let mut bytes = vec![b'x'; WINDOW as usize - 1];
    bytes.extend_from_slice("甲\r\n乙xx甲\r\n乙".as_bytes());
    let (_, book, stop) = fixture(&bytes, Some(Encoding::Utf8));
    let literal = "x甲\r\n乙";
    let first = book
        .find(literal, &TextPoint::default(), Direction::Next, &stop)
        .unwrap()
        .unwrap();
    assert_eq!(first.start.restart.restart, WINDOW - 2);
    let second = book
        .find(literal, &first.end, Direction::Next, &stop)
        .unwrap()
        .unwrap();
    assert!(second.start.restart.restart > first.end.restart.restart);
    assert_eq!(
        book.find(literal, &second.end, Direction::Next, &stop)
            .unwrap(),
        Some(first)
    );
    assert_eq!(
        book.find(literal, &second.start, Direction::Previous, &stop)
            .unwrap(),
        Some(first)
    );
    assert_eq!(
        book.find(literal, &first.start, Direction::Previous, &stop)
            .unwrap(),
        Some(second)
    );
    for hit in [first, second] {
        let view = book
            .window(ReadFrom::Around(hit.start), false, &stop)
            .unwrap();
        let row = view
            .rows
            .iter()
            .find(|r| r.offset_of(hit.start).is_some())
            .unwrap();
        assert!(row.text[row.offset_of(hit.start).unwrap()..].starts_with("x甲"));
    }
    assert_eq!(
        book.find("X甲", &second.end, Direction::Next, &stop)
            .unwrap(),
        None
    );
    assert_eq!(
        book.find("", &first.end, Direction::Previous, &stop)
            .unwrap(),
        None
    );
}

#[test]
fn projects_blank_runs_across_windows_without_changing_source() {
    let text = format!(
        "{} \t\r\n\t\r\n\n  indented\rfinal",
        "entry\n".repeat(MAX_ROWS - 1)
    );
    let (file, book, stop) = fixture(text.as_bytes(), None);
    let first = book.window(ReadFrom::Resume(None), true, &stop).unwrap();
    assert_eq!(first.rows.last().unwrap().text, "");
    let next = book
        .window(ReadFrom::After(first.after.unwrap()), true, &stop)
        .unwrap();
    assert_eq!(raw(&next), "  indented\rfinal");
    let previous = book
        .window(ReadFrom::Before(next.before.unwrap()), true, &stop)
        .unwrap();
    assert_eq!(raw(&previous), raw(&first));
    let full = book
        .window(ReadFrom::After(first.after.unwrap()), false, &stop)
        .unwrap();
    assert_eq!(raw(&full), "\t\r\n\n  indented\rfinal");
    let hit = book
        .find(
            " \t\r\n\t\r\n\n  ",
            &TextPoint::default(),
            Direction::Next,
            &stop,
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        hit.start.restart.restart as usize,
        text.find(" \t").unwrap()
    );
    assert_eq!(std::fs::read(file.path()).unwrap(), text.as_bytes());
}

#[test]
fn preserves_crlf_split_at_the_block_edge_and_final_cr() {
    let mut bytes = vec![b'z'; BLOCK - 1];
    bytes.extend_from_slice(b"\r\n\r\nlast\r");
    let (_, book, stop) = fixture(&bytes, None);
    let chunks = book.chunks(&stop).collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].ending, LineEnding::CrLf);
    assert_eq!(
        chunks[1].start,
        ReadingPosition::paragraph(BLOCK as u64 + 1)
    );
    assert_eq!(chunks[2].ending, LineEnding::Cr);
    let hit = book
        .find("\r\n\r\n", &TextPoint::default(), Direction::Next, &stop)
        .unwrap()
        .unwrap();
    assert_eq!(hit.end.restart.paragraph, BLOCK as u64 + 3);
}

#[test]
fn utf16_endianness_surrogates_and_sample_boundaries() {
    for enc in [Encoding::Utf16Le, Encoding::Utf16Be] {
        let le = enc == Encoding::Utf16Le;
        let text = format!("{}🦀\r\n尾", "z".repeat(BLOCK / 2 - 1));
        let mut bytes = if le { vec![255, 254] } else { vec![254, 255] };
        bytes.extend(
            text.encode_utf16()
                .flat_map(|u| if le { u.to_le_bytes() } else { u.to_be_bytes() }),
        );
        let (_, book, stop) = fixture(&bytes, None);
        assert_eq!(book.encoding(), enc);
        assert!(!book.sample(enc, &stop).unwrap().contains('�'));
        let view = book.window(ReadFrom::Resume(None), false, &stop).unwrap();
        assert_eq!(raw(&view), text);
        assert_eq!(view.rows[0].start, ReadingPosition::paragraph(2));
        let hit = book
            .find("🦀\r\n尾", &TextPoint::default(), Direction::Next, &stop)
            .unwrap()
            .unwrap();
        assert_eq!(hit.start.restart.restart, BLOCK as u64);
        assert_eq!(view.rows[0].offset_of(hit.start), Some(BLOCK / 2 - 1));
        assert_eq!(hit.end.restart.restart, bytes.len() as u64);
        let (_, no_bom, _) = fixture(&bytes[2..], None);
        assert_eq!(no_bom.encoding(), enc);
    }
}

#[test]
fn detects_utf8_with_a_sample_cut_inside_a_character() {
    let text = format!("{}界", "a".repeat(BLOCK - 1));
    let (_, book, stop) = fixture(text.as_bytes(), None);
    assert_eq!(book.encoding(), Encoding::Utf8);
    assert!(!book.encoding_uncertain());
    assert_eq!(
        raw(&book.window(ReadFrom::Resume(None), false, &stop).unwrap()),
        text
    );
}

#[test]
fn legacy_multibyte_units_keep_source_coordinates() {
    for (codec, enc, glyph) in [
        (encoding_rs::GBK, Encoding::Gbk, "中"),
        (encoding_rs::GB18030, Encoding::Gb18030, "🦀"),
        (encoding_rs::BIG5, Encoding::Big5, "書"),
    ] {
        let mut bytes = vec![b'x'; BLOCK - 1];
        let (encoded, _, failed) = codec.encode(glyph);
        assert!(!failed);
        bytes.extend_from_slice(&encoded);
        bytes.push(b'!');
        let (_, book, stop) = fixture(&bytes, Some(enc));
        let view = book.window(ReadFrom::Resume(None), false, &stop).unwrap();
        assert!(raw(&view).ends_with(&format!("{glyph}!")));
        let hit = book
            .find(glyph, &TextPoint::default(), Direction::Next, &stop)
            .unwrap()
            .unwrap();
        assert_eq!(hit.start.restart.restart as usize, BLOCK - 1);
        assert_eq!(hit.end.restart.restart as usize, BLOCK - 1 + encoded.len());
    }
}

#[test]
fn big5_points_distinguish_both_scalars_in_one_unit() {
    let (_, book, stop) = fixture(&[0x88, 0x62, 0x88, 0x62], Some(Encoding::Big5));
    let mark = "\u{0304}";
    let view = book.window(ReadFrom::Resume(None), false, &stop).unwrap();
    let row = &view.rows[0];
    assert_eq!(row.text, format!("Ê{mark}Ê{mark}"));
    let hit = book
        .find(mark, &TextPoint::default(), Direction::Next, &stop)
        .unwrap()
        .unwrap();
    assert_eq!(
        hit.start,
        TextPoint {
            restart: ReadingPosition::paragraph(0),
            utf8_skip: 2
        }
    );
    assert_eq!(row.offset_of(hit.start), Some(2));
    assert_eq!(row.point_at(2), Some(hit.start));
    let second = book
        .find(mark, &hit.end, Direction::Next, &stop)
        .unwrap()
        .unwrap();
    assert_eq!(second.start.restart.restart, 2);
    assert_eq!(
        book.find(mark, &second.start, Direction::Previous, &stop)
            .unwrap(),
        Some(hit)
    );
    assert_eq!(book.sample(Encoding::Big5, &stop).unwrap(), row.text);
}

#[test]
fn malformed_source_units_match_the_reference_decoder() {
    let mut bytes: Vec<u8> = (0..4096)
        .map(|i| ((i * 313 + i / 13) % 256) as u8)
        .collect();
    bytes.extend_from_slice(&[0x81, 0x30, 0x81, b'\n', 0xd8, 0x00, 0x09]);
    for (codec, enc) in [
        (encoding_rs::UTF_8, Encoding::Utf8),
        (encoding_rs::GBK, Encoding::Gbk),
        (encoding_rs::GB18030, Encoding::Gb18030),
        (encoding_rs::BIG5, Encoding::Big5),
        (encoding_rs::UTF_16BE, Encoding::Utf16Be),
        (encoding_rs::UTF_16LE, Encoding::Utf16Le),
    ] {
        let (_, book, stop) = fixture(&bytes, Some(enc));
        let view = book.window(ReadFrom::Resume(None), false, &stop).unwrap();
        let actual = raw(&view);
        let (expected, _) = codec.decode_without_bom_handling(&bytes);
        let mismatch = actual
            .char_indices()
            .zip(expected.chars())
            .find(|((_, a), b)| a != b);
        assert!(mismatch.is_none(), "{enc:?} first mismatch at {mismatch:?}");
        assert_eq!(actual.len(), expected.len(), "{enc:?}");
    }
}

#[test]
fn independent_readers_resume_inside_a_long_unbroken_line() {
    let mut file = NamedTempFile::new().unwrap();
    let len = 50 * 1024 * 1024;
    let anchor = 37 * 1024 * 1024;
    file.as_file().set_len(len).unwrap();
    file.seek(SeekFrom::Start(anchor)).unwrap();
    file.write_all(b"anchor").unwrap();
    let handle = File::open(file.path()).unwrap();
    let mut witness = handle.try_clone().unwrap();
    witness.seek(SeekFrom::Start(123)).unwrap();
    let stop = CancellationToken::new();
    let book = TextBook::open(handle, Some(Encoding::Utf8), &stop).unwrap();
    let at = ReadingPosition {
        paragraph: 0,
        restart: anchor,
    };
    let saved = book.checkpoint(at);
    std::thread::scope(|scope| {
        let search = scope.spawn(|| book.find("anchor", &at.into(), Direction::Next, &stop));
        let page = book
            .window(ReadFrom::Resume(Some(saved)), false, &stop)
            .unwrap();
        assert_eq!(page.rows[0].start, at);
        assert!(page.rows[0].text.starts_with("anchor"));
        assert!(
            page.rows
                .iter()
                .all(|r| r.continues_from_previous() && r.text.len() <= BLOCK)
        );
        assert_eq!(raw(&page).len(), WINDOW as usize);
        let before = book
            .window(ReadFrom::Before(page.before.unwrap()), false, &stop)
            .unwrap();
        assert_eq!(before.rows.last().unwrap().end, at);
        let again = book
            .window(ReadFrom::After(before.after.unwrap()), false, &stop)
            .unwrap();
        assert_eq!(raw(&again), raw(&page));
        assert_eq!(search.join().unwrap().unwrap().unwrap().start.restart, at);
    });
    assert_eq!(witness.stream_position().unwrap(), 123);
}

#[test]
fn stale_checkpoints_realign_and_cursors_are_book_local() {
    let bytes = format!("{}\r\nbook\r\nlast", "x".repeat(BLOCK * 5));
    let (mut file, old, stop) = fixture(bytes.as_bytes(), None);
    let cursor = old
        .window(ReadFrom::Resume(None), false, &stop)
        .unwrap()
        .after
        .unwrap();
    let saved = old.checkpoint(ReadingPosition {
        paragraph: 0,
        restart: (BLOCK * 5 + 3) as u64,
    });
    file.as_file().set_len(0).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    let rewritten: Vec<_> = "甲\r\n乙🦀"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect();
    file.write_all(&rewritten).unwrap();
    assert!(matches!(
        old.window(ReadFrom::After(cursor), false, &stop),
        Err(ReadError::SourceChanged)
    ));
    let book = TextBook::open(
        File::open(file.path()).unwrap(),
        Some(Encoding::Utf16Be),
        &stop,
    )
    .unwrap();
    assert!(matches!(
        book.window(ReadFrom::Before(cursor), false, &stop),
        Err(ReadError::InvalidCursor)
    ));
    let view = book
        .window(ReadFrom::Resume(Some(saved)), false, &stop)
        .unwrap();
    assert_eq!(view.rows[0].start, ReadingPosition::paragraph(6));
    assert_eq!(raw(&view), "乙🦀");
}

#[test]
fn empty_and_cancelled_reads() {
    let (_, book, stop) = fixture(b"", None);
    let window = book.window(ReadFrom::Resume(None), true, &stop).unwrap();
    assert!(window.rows.is_empty() && window.before.is_none() && window.after.is_none());
    assert_eq!(
        book.find("x", &TextPoint::default(), Direction::Next, &stop)
            .unwrap(),
        None
    );
    stop.cancel();
    assert!(matches!(
        book.window(ReadFrom::Resume(None), false, &stop),
        Err(ReadError::Cancelled)
    ));
    assert!(matches!(
        book.sample(Encoding::Gbk, &stop),
        Err(ReadError::Cancelled)
    ));
    assert!(matches!(
        book.find("x", &TextPoint::default(), Direction::Next, &stop),
        Err(ReadError::Cancelled)
    ));
}

#[test]
fn long_whitespace_is_projected_in_bounded_requests_and_indentation_survives() {
    let spaces = " ".repeat(WINDOW as usize * 3);
    let text = format!("title\n{spaces}whole line\r\n{spaces}\n\t\nend");
    let (_, book, stop) = fixture(text.as_bytes(), None);
    let mut page = book.window(ReadFrom::Resume(None), true, &stop).unwrap();
    assert_eq!(raw(&page), "title\n");
    let mut display = String::new();
    let mut blank_rows = 0;
    let mut requests = 0;
    while let Some(after) = page.after {
        page = book.window(ReadFrom::After(after), true, &stop).unwrap();
        requests += 1;
        assert!(requests < 40);
        for row in &page.rows {
            blank_rows += usize::from(row.text.is_empty());
            display.push_str(&row.text);
        }
    }
    assert_eq!(blank_rows, 1);
    assert_eq!(display, format!("{spaces}whole lineend"));
}

#[test]
fn automatically_decodes_a_legacy_chinese_sample() {
    let text = "这是一段中文小说，用来判断书籍的编码。第一章 春天的故事。读者们又见面了，今天会发生什么呢？".repeat(20);
    let (bytes, _, failed) = encoding_rs::GB18030.encode(&text);
    assert!(!failed);
    let (_, book, stop) = fixture(&bytes, None);
    assert!(matches!(book.encoding(), Encoding::Gbk | Encoding::Gb18030));
    assert_eq!(
        raw(&book.window(ReadFrom::Resume(None), false, &stop).unwrap()),
        text
    );
}

#[test]
fn a_hidden_source_match_maps_to_its_visible_blank_row() {
    let (_, book, stop) = fixture(b"a\n \r\n\t\n \t\r\nb", None);
    let hit = book
        .find("\t\r\n", &TextPoint::default(), Direction::Next, &stop)
        .unwrap()
        .unwrap();
    let view = book.window(ReadFrom::Resume(None), true, &stop).unwrap();
    assert_eq!(view.rows.len(), 3);
    assert_eq!(view.rows[1].offset_of(hit.start), Some(0));
    let anchored = book
        .window(ReadFrom::Around(hit.start), true, &stop)
        .unwrap();
    assert_eq!(anchored.rows[0].offset_of(hit.start), Some(0));
    let before = book
        .window(ReadFrom::Before(anchored.before.unwrap()), true, &stop)
        .unwrap();
    assert_eq!(raw(&before), "a\n");
}

#[test]
fn uncertain_text_offers_a_candidate_preview_without_changing_the_book() {
    let (_, book, stop) = fixture(&[0xff, 0xff, 0xff], None);
    assert_eq!(book.encoding(), Encoding::Utf8);
    assert!(book.encoding_uncertain());
    assert_eq!(book.sample(Encoding::Utf16Be, &stop).unwrap(), "\u{ffff}�");
    assert_eq!(
        raw(&book.window(ReadFrom::Resume(None), false, &stop).unwrap()),
        "���"
    );
}

#[test]
fn recognizes_big5_when_the_file_head_ends_in_a_partial_character() {
    let text = "第一章 春天的故事\r\n這是繁體中文小說，書中的讀者來自臺灣。\r\n";
    let (encoded, _, errors) = encoding_rs::BIG5.encode(text);
    assert!(!errors);
    let mut bytes = encoded.repeat((BLOCK - 1) / encoded.len());
    bytes.resize(BLOCK - 1, b' ');
    bytes.extend_from_slice(&[0xae, 0xd1]); // 書
    let (_, book, stop) = fixture(&bytes, None);
    assert_eq!(book.encoding(), Encoding::Big5);
    let hit = book
        .find(" 書", &TextPoint::default(), Direction::Next, &stop)
        .unwrap()
        .unwrap();
    assert_eq!(hit.end.restart.restart as usize, bytes.len());
}
