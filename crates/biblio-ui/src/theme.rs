use nsh_core::prelude::*;

#[derive(Clone, Copy)]
pub struct Palette(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct Colors {
    pub paper: u32,
    pub sheet: u32,
    pub ink: u32,
    pub secondary: u32,
    pub rule: u32,
    pub action: u32,
    pub on_action: u32,
    pub cloth: u32,
    pub current: u32,
}

const PORCELAIN: [Colors; 2] = [
    Colors {
        paper: 0xf7f6f1,
        sheet: 0xfffefa,
        ink: 0x222c40,
        secondary: 0x586579,
        rule: 0xd0d4d6,
        action: 0x344fb2,
        on_action: 0xffffff,
        cloth: 0x23324f,
        current: 0x3c5177,
    },
    Colors {
        paper: 0x181e2a,
        sheet: 0x232c3b,
        ink: 0xf2f2ec,
        secondary: 0xb1bdcc,
        rule: 0x414d60,
        action: 0xacc1ff,
        on_action: 0x152346,
        cloth: 0x121a29,
        current: 0x304361,
    },
];
const GRAPHITE: [Colors; 2] = [
    Colors {
        paper: 0xf2eee7,
        sheet: 0xfffaf3,
        ink: 0x302d2a,
        secondary: 0x655f55,
        rule: 0xd5cdbf,
        action: 0x414e8e,
        on_action: 0xffffff,
        cloth: 0x35343b,
        current: 0x54525c,
    },
    Colors {
        paper: 0x211f1d,
        sheet: 0x2e2b28,
        ink: 0xf4eee4,
        secondary: 0xc5bbae,
        rule: 0x504a43,
        action: 0xbdc7fa,
        on_action: 0x212748,
        cloth: 0x19181c,
        current: 0x423e47,
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
pub fn sheet() -> Rgba {
    rgb(colors().sheet)
}
pub fn rule() -> Rgba {
    rgb(colors().rule)
}
pub fn accent() -> Rgba {
    rgb(colors().action)
}
pub fn on_accent() -> Rgba {
    rgb(colors().on_action)
}
pub fn cloth() -> Rgba {
    rgb(colors().cloth)
}
pub fn current() -> Rgba {
    rgb(colors().current)
}
pub fn rail_ink() -> Rgba {
    rgb(0xfaf8f0)
}
pub fn rail_muted() -> Rgba {
    rgb(0xc1ccdd)
}
pub fn gold() -> Rgba {
    rgb(0xf3b94f)
}
pub fn danger() -> Rgba {
    rgb(if dark() { 0xffbab0 } else { 0x993c32 })
}
pub fn stock(seed: usize) -> Rgba {
    let tints = if dark() {
        [0x2c3b52, 0x40363c, 0x32423f, 0x453f34]
    } else {
        [0xdce5f2, 0xf0dfdb, 0xdce8e0, 0xeee3cd]
    };
    rgb(tints[seed % tints.len()])
}
