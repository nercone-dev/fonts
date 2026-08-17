use std::collections::BTreeMap;

use read_fonts::tables::hhea::Hhea;
use read_fonts::tables::os2::Os2;

use crate::font::{Extent, Font};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub width: f64,
    pub ascent: f64,
    pub descent: f64,
    pub cap: f64,
}

impl Cell {
    pub fn of(font: &Font, width: u16) -> Cell {
        let upem = font.upem() as f64;
        let hhea = font.read::<Hhea>().expect("missing hhea");
        let gap = (hhea.line_gap().to_i16() as f64).max(0.0);
        let cap = font.read::<Os2>().and_then(|os2| os2.s_cap_height()).unwrap_or(0) as f64;
        Cell {
            width: width as f64 / upem,
            ascent: ((hhea.ascender().to_i16() as f64).abs() + (gap / 2.0).floor()) / upem,
            descent: (-(hhea.descender().to_i16() as f64).abs() - (gap - (gap / 2.0).floor())) / upem,
            cap: cap.max(0.0) / upem,
        }
    }

    pub fn height(&self) -> f64 {
        self.ascent - self.descent
    }

    pub fn middle(&self) -> f64 {
        (self.ascent + self.descent) / 2.0
    }

    pub fn icon(&self) -> f64 {
        if self.cap <= 0.0 {
            return self.height();
        }
        (self.cap * 2.0 + self.height()) / 3.0
    }

    pub fn ratio(&self, other: &Cell) -> f64 {
        (self.width / other.width).min(self.icon() / other.icon())
    }

    pub fn shift(&self, other: &Cell) -> f64 {
        self.middle() - other.middle()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stretch {
    Aspect,
    Fill,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub align: Align,
    pub stretch: Stretch,
    pub overlap: f64,
    pub ratio: Option<f64>,
    pub full: bool,
}

#[allow(non_upper_case_globals)]
impl Placement {
    pub const rise: f64 = 0.01;

    pub fn new(align: Align, stretch: Stretch, overlap: f64, ratio: Option<f64>, full: bool) -> Placement {
        Placement { align, stretch, overlap, ratio, full }
    }

    pub fn icon(align: Align) -> Placement {
        Placement::new(align, Stretch::Aspect, 0.0, None, false)
    }

    pub fn aspect(align: Align, overlap: f64, ratio: Option<f64>) -> Placement {
        Placement::new(align, Stretch::Aspect, overlap, ratio, true)
    }

    pub fn fill(align: Align, overlap: f64, ratio: Option<f64>) -> Placement {
        Placement::new(align, Stretch::Fill, overlap, ratio, true)
    }

    pub fn reach(&self, cell: &Cell, upem: f64) -> (f64, f64) {
        let height = if self.full { cell.height() } else { cell.icon() };
        (cell.width * upem * (1.0 + self.overlap), height * upem * (1.0 + self.overlap.min(Placement::rise)))
    }

    pub fn scale(&self, cell: &Cell, upem: f64, bounds: &Extent) -> (f64, f64) {
        let width = bounds.maximum_x - bounds.minimum_x;
        let height = bounds.maximum_y - bounds.minimum_y;
        if width <= 0.0 || height <= 0.0 {
            return (1.0, 1.0);
        }

        let (reach_x, reach_y) = self.reach(cell, upem);
        let mut horizontal = reach_x / width;
        let mut vertical = reach_y / height;

        if self.stretch == Stretch::Aspect {
            horizontal = horizontal.min(vertical);
            vertical = horizontal;
        }

        if let Some(limit) = self.ratio {
            let found = width * horizontal / (height * vertical);
            if found > limit {
                horizontal *= limit / found;
            }
        }

        (horizontal, vertical)
    }

    pub fn offset(&self, cell: &Cell, upem: f64, bounds: &Extent, scale: (f64, f64)) -> (f64, f64) {
        let (horizontal, vertical) = scale;
        let cell_width = cell.width * upem;
        let width = (bounds.maximum_x - bounds.minimum_x) * horizontal;
        let left = -bounds.minimum_x * horizontal;

        let mut x = match self.align {
            Align::Left => left,
            Align::Middle => left + (cell_width - width) / 2.0,
            Align::Right => left + cell_width - width,
        };

        if self.overlap == 0.0 {
            x = x.max(left);
        } else {
            let overlap = cell_width * self.overlap;
            match self.align {
                Align::Left => x -= overlap,
                Align::Middle => {}
                Align::Right => x = cell_width + overlap - bounds.maximum_x * horizontal,
            }
        }

        let middle = (bounds.minimum_y + bounds.maximum_y) / 2.0 * vertical;
        (x, cell.middle() * upem - middle)
    }
}

pub struct Symbols;

#[allow(non_upper_case_globals)]
impl Symbols {
    pub const groups: [&'static [(u32, u32)]; 46] = [
        &[(0xEA61, 0xEA61), (0xEB13, 0xEB13)],
        &[(0xEAB4, 0xEAB7)],
        &[(0xEA7D, 0xEA7D), (0xEA99, 0xEAA1), (0xEBCB, 0xEBCB)],
        &[(0xEAA2, 0xEAA2), (0xEB9A, 0xEB9A), (0xEC08, 0xEC09)],
        &[(0xEAD4, 0xEAD6)],
        &[(0xEB43, 0xEB43), (0xEC0B, 0xEC0C)],
        &[(0xEB6E, 0xEB71)],
        &[(0xEB89, 0xEB8B), (0xEC07, 0xEC07)],
        &[(0xEBD5, 0xEBD7)],
        &[(0xF005, 0xF006), (0xF089, 0xF089)],
        &[(0xF026, 0xF028)],
        &[(0xF02B, 0xF02C)],
        &[(0xF031, 0xF035)],
        &[(0xF044, 0xF046)],
        &[(0xF048, 0xF052)],
        &[(0xF060, 0xF063)],
        &[(0xF053, 0xF054), (0xF077, 0xF078)],
        &[(0xF07D, 0xF07E)],
        &[(0xF0A4, 0xF0A7)],
        &[(0xF0D7, 0xF0DA), (0xF0DC, 0xF0DE)],
        &[(0xF100, 0xF107)],
        &[(0xF130, 0xF131)],
        &[(0xF141, 0xF142)],
        &[(0xF153, 0xF15A)],
        &[(0xF175, 0xF178)],
        &[(0xF182, 0xF183)],
        &[(0xF221, 0xF22D)],
        &[(0xF255, 0xF25B)],
        &[(0xF416, 0xF416), (0xF424, 0xF424), (0xF431, 0xF434), (0xF43E, 0xF43E), (0xF443, 0xF443), (0xF45C, 0xF45C), (0xF46C, 0xF46C)],
        &[
            (0xF438, 0xF438), (0xF444, 0xF445), (0xF44A, 0xF44B), (0xF460, 0xF460), (0xF467, 0xF467), (0xF470, 0xF470), (0xF47B, 0xF47E),
            (0xF48B, 0xF48B), (0xF4A2, 0xF4A2), (0xF4C3, 0xF4C3), (0xF51D, 0xF51D),
        ],
        &[(0xF476, 0xF476), (0xF478, 0xF478), (0xF49A, 0xF49A)],
        &[(0xF4EF, 0xF4F2)],
        &[(0xF461, 0xF461), (0xF47A, 0xF47A), (0xF493, 0xF493), (0xF533, 0xF533)],
        &[(0xE339, 0xE339), (0xE33E, 0xE33E), (0xE341, 0xE341)],
        &[(0xE33F, 0xE340), (0xE344, 0xE344), (0xE347, 0xE349), (0xE352, 0xE353), (0xE37F, 0xE380)],
        &[(0xE34E, 0xE350)],
        &[(0xE354, 0xE35B), (0xE3A9, 0xE3A9)],
        &[(0xE381, 0xE38C)],
        &[(0xE38D, 0xE3A8)],
        &[(0xE3AF, 0xE3BB)],
        &[(0xE368, 0xE369)],
        &[(0xE34C, 0xE34D), (0xE36B, 0xE36B), (0xE3C1, 0xE3C2)],
        &[(0xE345, 0xE345), (0xE351, 0xE351), (0xE36A, 0xE36A), (0xE36C, 0xE375), (0xE382, 0xE382)],
        &[(0xE300, 0xE33D), (0xE35E, 0xE367), (0xE376, 0xE37B), (0xE37D, 0xE37E), (0xE3AA, 0xE3AE)],
        &[(0xEE00, 0xEE05)],
        &[(0xEE06, 0xEE0B)],
    ];

    pub fn placement(codepoint: u32) -> Placement {
        match codepoint {
            0xE0B0 => Placement::fill(Align::Left, 0.06, Some(0.70)),
            0xE0B1 => Placement::fill(Align::Left, 0.0, Some(0.70)),
            0xE0B2 => Placement::fill(Align::Right, 0.06, Some(0.70)),
            0xE0B3 => Placement::fill(Align::Right, 0.0, Some(0.70)),
            0xE0B4 => Placement::fill(Align::Left, 0.06, Some(0.59)),
            0xE0B5 => Placement::fill(Align::Left, 0.0, Some(0.50)),
            0xE0B6 => Placement::fill(Align::Right, 0.06, Some(0.59)),
            0xE0B7 => Placement::fill(Align::Right, 0.0, Some(0.50)),
            0xE0B8 => Placement::fill(Align::Left, 0.05, None),
            0xE0B9 => Placement::fill(Align::Left, 0.0, None),
            0xE0BA => Placement::fill(Align::Right, 0.05, None),
            0xE0BB => Placement::fill(Align::Right, 0.0, None),
            0xE0BC => Placement::fill(Align::Left, 0.05, None),
            0xE0BD => Placement::fill(Align::Left, 0.0, None),
            0xE0BE => Placement::fill(Align::Right, 0.05, None),
            0xE0BF => Placement::fill(Align::Right, 0.0, None),
            0xE0C0 => Placement::fill(Align::Left, 0.05, None),
            0xE0C1 => Placement::fill(Align::Left, 0.0, None),
            0xE0C2 => Placement::fill(Align::Right, 0.05, None),
            0xE0C3 => Placement::fill(Align::Right, 0.0, None),
            0xE0C4 => Placement::fill(Align::Left, -0.03, Some(0.86)),
            0xE0C5 => Placement::fill(Align::Right, -0.03, Some(0.86)),
            0xE0C6 => Placement::fill(Align::Left, -0.03, Some(0.78)),
            0xE0C7 => Placement::fill(Align::Right, -0.03, Some(0.78)),
            0xE0C8 => Placement::fill(Align::Left, 0.05, None),
            0xE0CA => Placement::fill(Align::Right, 0.05, None),
            0xE0CC => Placement::fill(Align::Left, 0.02, Some(0.85)),
            0xE0CD => Placement::fill(Align::Left, 0.0, Some(0.865)),
            0xE0CE | 0xE0D0 | 0xE0D1 => Placement::aspect(Align::Left, 0.0, None),
            0xE0D2 => Placement::fill(Align::Left, 0.02, Some(0.70)),
            0xE0D4 => Placement::fill(Align::Right, 0.02, Some(0.70)),
            0xE0D6 => Placement::fill(Align::Left, 0.05, Some(0.70)),
            0xE0D7 => Placement::fill(Align::Right, 0.05, Some(0.70)),
            0xEE00 | 0xEE03 => Placement::fill(Align::Right, 0.05, None),
            0xEE01 | 0xEE04 => Placement::fill(Align::Middle, 0.10, None),
            0xEE02 | 0xEE05 => Placement::fill(Align::Left, 0.05, None),
            0xE0A0..=0xE0A3 | 0xE0B0..=0xE0D7 => Placement::aspect(Align::Middle, 0.0, None),
            0xEE06..=0xEE0B => Placement::aspect(Align::Middle, -0.03, None),
            _ => Placement::icon(Align::Middle),
        }
    }

    pub fn placements(cmap: &BTreeMap<u32, u16>) -> BTreeMap<u16, Placement> {
        cmap.iter().map(|(code, glyph)| (*glyph, Symbols::placement(*code))).collect()
    }

    pub fn grouped(cmap: &BTreeMap<u32, u16>) -> Vec<Vec<u16>> {
        Symbols::groups
            .iter()
            .map(|group| {
                group
                    .iter()
                    .flat_map(|(start, end)| (*start..=*end).filter_map(|code| cmap.get(&code).copied()))
                    .collect::<Vec<u16>>()
            })
            .filter(|group| !group.is_empty())
            .collect()
    }
}
