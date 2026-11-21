//! Semantic colours. See the colour table in the root `DESIGN.md`.
use nsh_core::prelude::*;

#[derive(Clone, Copy)]
pub struct Palette(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct Colors {
    pub paper: u32,
    pub rail: u32,
    pub sheet: u32,
    pub ink: u32,
    pub secondary: u32,
    pub rule: u32,
    pub keyline: u32,
    pub wash: u32,
    pub action: u32,
    pub on_action: u32,
    pub tint: u32,
}

const PORCELAIN: [Colors; 2] = [
    Colors {
        paper: 0xfafaf8,
        rail: 0xf3f3f0,
        sheet: 0xffffff,
        ink: 0x1e2229,
        secondary: 0x5e6470,
        rule: 0xe4e4df,
        keyline: 0xc9cac4,
        wash: 0xeaeae6,
        action: 0x2f4a9e,
        on_action: 0xffffff,
        tint: 0xe9edf7,
    },
    Colors {
        paper: 0x16181c,
        rail: 0x1b1d22,
        sheet: 0x1f2227,
        ink: 0xecece8,
        secondary: 0xa3a8b2,
        rule: 0x2c3036,
        keyline: 0x41464e,
        wash: 0x262a30,
        action: 0x9db2f0,
        on_action: 0x11182e,
        tint: 0x232b3d,
    },
];
const GRAPHITE: [Colors; 2] = [
    Colors {
        paper: 0xf6f4f0,
        rail: 0xefece6,
        sheet: 0xfffdf9,
        ink: 0x2a2724,
        secondary: 0x67615a,
        rule: 0xe3ded5,
        keyline: 0xccc5b9,
        wash: 0xe8e4dc,
        action: 0x2a2724,
        on_action: 0xffffff,
        tint: 0xeae5dc,
    },
    Colors {
        paper: 0x1c1b19,
        rail: 0x21201d,
        sheet: 0x272522,
        ink: 0xeeeae3,
        secondary: 0xb3aca2,
        rule: 0x35322e,
        keyline: 0x4a4640,
        wash: 0x2d2a26,
        action: 0xeeeae3,
        on_action: 0x1c1b19,
        tint: 0x322f2a,
    },
];

fn colors() -> Colors {
    let graphite = use_context::<Palette>().is_some_and(|p| p.0.get());
    (if graphite { GRAPHITE } else { PORCELAIN })[usize::from(dark())]
}

pub fn ink() -> Rgba {
    rgb(colors().ink)
}
pub fn secondary() -> Rgba {
    rgb(colors().secondary)
}
pub fn paper() -> Rgba {
    rgb(colors().paper)
}
pub fn rail() -> Rgba {
    rgb(colors().rail)
}
pub fn sheet() -> Rgba {
    rgb(colors().sheet)
}
pub fn rule() -> Rgba {
    rgb(colors().rule)
}
pub fn keyline() -> Rgba {
    rgb(colors().keyline)
}
pub fn wash() -> Rgba {
    rgb(colors().wash)
}
pub fn accent() -> Rgba {
    rgb(colors().action)
}
pub fn on_accent() -> Rgba {
    rgb(colors().on_action)
}
pub fn tint() -> Rgba {
    rgb(colors().tint)
}
pub fn danger() -> Rgba {
    rgb(if dark() { 0xffb4a8 } else { 0x9a3b2f })
}
/// Fully transparent; lets quiet controls show the surface beneath them.
pub fn clear() -> Rgba {
    rgba(0x00000000)
}
