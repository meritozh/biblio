use charset_normalizer_rs::entity::NormalizerSettings;
use encoding_rs::{BIG5, DecoderResult, GB18030, GBK, UTF_8, UTF_16BE, UTF_16LE};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Gbk,
    Gb18030,
    Big5,
}

impl Encoding {
    fn codec(self) -> &'static encoding_rs::Encoding {
        match self {
            Self::Utf8 => UTF_8,
            Self::Utf16Le => UTF_16LE,
            Self::Utf16Be => UTF_16BE,
            Self::Gbk => GBK,
            Self::Gb18030 => GB18030,
            Self::Big5 => BIG5,
        }
    }

    pub(super) fn bom(bytes: &[u8]) -> Option<(Self, usize)> {
        [
            (Self::Utf8, &b"\xef\xbb\xbf"[..]),
            (Self::Utf16Le, &b"\xff\xfe"[..]),
            (Self::Utf16Be, &b"\xfe\xff"[..]),
        ]
        .into_iter()
        .find_map(|(enc, bom)| bytes.starts_with(bom).then_some((enc, bom.len())))
    }

    pub(super) fn detect(bytes: &[u8], truncated: bool) -> (Self, bool) {
        if let Some((enc, _)) = Self::bom(bytes) {
            return (enc, false);
        }
        let pairs = bytes.chunks_exact(2);
        let count = pairs.len();
        if count >= 2 {
            let mut zeros = [0, 0];
            for pair in pairs {
                for lane in 0..2 {
                    zeros[lane] += usize::from(pair[lane] == 0 && pair[1 - lane] != 0);
                }
            }
            for (lane, enc) in [(0, Self::Utf16Be), (1, Self::Utf16Le)] {
                if zeros[lane] >= 2 && zeros[lane] * 5 >= count && zeros[lane] > zeros[1 - lane] * 4
                {
                    return (enc, false);
                }
            }
        }
        match str::from_utf8(bytes) {
            Ok(_) => return (Self::Utf8, false),
            Err(e) if truncated && e.error_len().is_none() => return (Self::Utf8, false),
            _ => {}
        }
        // Score only codecs the reader can actually use. Each gets a complete
        // prefix, even when the byte sample ended in a four-byte GB18030 unit.
        let best = [Self::Gb18030, Self::Big5]
            .into_iter()
            .filter_map(|enc| {
                let codec = enc.codec();
                let sample = if truncated {
                    enc.complete_prefix(bytes)?
                } else {
                    bytes
                };
                let settings = NormalizerSettings {
                    include_encodings: vec![codec.name().to_owned()],
                    enable_fallback: false,
                    ..Default::default()
                };
                let matches = charset_normalizer_rs::from_bytes(sample, Some(settings)).ok()?;
                Some((enc, matches.get_by_encoding(codec.name())?.clone()))
            })
            .min_by(|a, b| a.1.cmp(&b.1));
        best.map_or((Self::Utf8, true), |(enc, _)| (enc, false))
    }

    fn complete_prefix(self, bytes: &[u8]) -> Option<&[u8]> {
        let mut decoder = self.codec().new_decoder_without_bom_handling();
        let mut dst = vec![0; decoder.max_utf8_buffer_length_without_replacement(bytes.len())?];
        let (result, _, _) = decoder.decode_to_utf8_without_replacement(bytes, &mut dst, false);
        if result != DecoderResult::InputEmpty {
            return None;
        }
        match decoder
            .decode_to_utf8_without_replacement(&[], &mut dst, true)
            .0
        {
            DecoderResult::InputEmpty => Some(bytes),
            DecoderResult::Malformed(n, _) => Some(&bytes[..bytes.len() - n as usize]),
            DecoderResult::OutputFull => unreachable!("reserved a full sample buffer"),
        }
    }

    pub(super) fn stride(self) -> usize {
        if matches!(self, Self::Utf16Le | Self::Utf16Be) {
            2
        } else {
            1
        }
    }

    pub(super) fn ascii(self, src: &[u8]) -> Option<u8> {
        let byte = match (self, src) {
            (Self::Utf16Le, [b, 0, ..]) | (Self::Utf16Be, [0, b, ..]) => *b,
            (Self::Utf16Le | Self::Utf16Be, _) => return None,
            (_, [b, ..]) => *b,
            (_, []) => return None,
        };
        byte.is_ascii().then_some(byte)
    }

    // These encodings are stateless at a complete source unit. Decode exactly
    // one unit so the byte that produced a replacement or two Big5 scalars
    // stays available to search and layout. The reader supplies block overlap.
    pub(super) fn unit<'a>(self, bytes: &'a [u8], buf: &'a mut [u8; 8]) -> (usize, &'a str) {
        let lead = bytes[0];
        let width = match self {
            Self::Utf8 => match lead {
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => 1,
            },
            Self::Utf16Le | Self::Utf16Be => {
                let high_byte = if self == Self::Utf16Be {
                    Some(&lead)
                } else {
                    bytes.get(1)
                };
                if high_byte.is_some_and(|b| (0xd8..=0xdb).contains(b)) {
                    4
                } else {
                    2
                }
            }
            Self::Gbk | Self::Gb18030 | Self::Big5 => {
                if !(0x81..=0xfe).contains(&lead) {
                    1
                } else if self != Self::Big5 && bytes.get(1).is_some_and(u8::is_ascii_digit) {
                    4
                } else {
                    2
                }
            }
        };
        let input = &bytes[..width.min(bytes.len())];
        if self == Self::Utf8 {
            return match str::from_utf8(input) {
                Ok(text) => (input.len(), text),
                Err(e) => (e.error_len().unwrap_or(input.len()), "�"),
            };
        }
        let mut decoder = self.codec().new_decoder_without_bom_handling();
        let (result, read, written) = decoder.decode_to_utf8_without_replacement(input, buf, true);
        match result {
            DecoderResult::InputEmpty => (read, str::from_utf8(&buf[..written]).unwrap()),
            DecoderResult::Malformed(len, _) => (len as usize, "�"),
            DecoderResult::OutputFull => unreachable!("one source unit fits in eight UTF-8 bytes"),
        }
    }

    pub(super) fn preview(self, bytes: &[u8], at_eof: bool) -> String {
        let mut decoder = self.codec().new_decoder_without_bom_handling();
        let mut out = String::with_capacity(decoder.max_utf8_buffer_length(bytes.len()).unwrap());
        let (result, _, _) = decoder.decode_to_string(bytes, &mut out, at_eof);
        debug_assert_eq!(result, encoding_rs::CoderResult::InputEmpty);
        let mut breaks = 0;
        let mut previous_cr = false;
        let end = out
            .char_indices()
            .take(2049)
            .enumerate()
            .find_map(|(count, (i, c))| {
                if count == 2048 || (breaks >= 4 && !(previous_cr && c == '\n')) {
                    return Some(i);
                }
                if c == '\r' || (c == '\n' && !previous_cr) {
                    breaks += 1;
                }
                previous_cr = c == '\r';
                None
            });
        if let Some(end) = end {
            out.truncate(end);
        }
        out
    }
}
