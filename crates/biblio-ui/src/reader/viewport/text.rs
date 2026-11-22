//! Native text shared by visible passages and line-break measurement.
use super::Passage;
use crate::ui::*;
use biblio_core::text_reader::{book::Hit, store::Typography};
use nsh_core::prelude::*;

fn face(content: Text, ty: &Typography, heading: bool) -> Text {
    let size = if heading { ty.size * 1.22 } else { ty.size };
    let mut el = content
        .text_color(ink)
        .text_size(px(size))
        .line_height(px(size * ty.leading))
        .letter_spacing(px(ty.spacing));
    if heading {
        el = el.font_weight(FontWeight::SEMIBOLD);
    }
    if let Some(family) = &ty.family {
        el = el.font_family(family.clone());
    }
    el
}

fn highlighted(row: &Passage, s: &str, hit: Option<Hit>) -> Text {
    if let Some(hit) = hit {
        let a = row.offset(hit.start);
        let b = row.offset(hit.end);
        if a < b {
            return text(s[..a].to_owned())
                .child(text(s[a..b].to_owned()).bg(accent).text_color(on_accent))
                .child(text(s[b..].to_owned()));
        }
    }
    text(if s.is_empty() {
        " ".to_string()
    } else {
        s.to_owned()
    })
}

// NSH counts an inline view as one UTF-16 object-replacement character.
// The spacer has no source byte and only takes space on the first line.
impl Passage {
    pub(super) fn text(&self, s: &str, ty: &Typography, heading: bool, hit: Option<Hit>) -> Text {
        let body = highlighted(self, s, hit);
        let el = if self.is_refined_start() && !heading {
            let mut spacer = view().w(px(2. * ty.size)).h(px(1.)).accessible(false);
            if hit.is_some_and(|h| self.indent_hit(h)) {
                spacer = spacer.bg(accent).h(px(ty.size));
            }
            text("").label(s.to_owned()).child(spacer).child(body)
        } else {
            body
        };
        face(el, ty, heading)
    }
}
