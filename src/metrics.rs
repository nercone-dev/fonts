use write_fonts::from_obj::ToOwnedTable;
use write_fonts::tables::gasp::{Gasp, GaspRange, GaspRangeBehavior};
use write_fonts::tables::os2::SelectionFlags;
use write_fonts::types::{Fixed, Tag};

use crate::font::{tags, Font};
use crate::models::{Family, Style};
use crate::prepare::Component;
use crate::ranges;

#[allow(non_upper_case_globals)]
pub const epsilon: f64 = 1e-6;

pub struct Metrics {
    pub upem: i32,
    pub ascender: i32,
    pub descender: i32,
    pub gap: i32,
    pub line_ascender: i32,
    pub line_descender: i32,
    pub line_gap: i32,
    pub window_ascent: i32,
    pub window_descent: i32,
    pub typo_metrics: bool,
    pub cap_height: i32,
    pub x_height: i32,
    pub italic_angle: f64,
    pub underline_position: i32,
    pub underline_thickness: i32,
}

impl Metrics {
    pub fn slanted(&self) -> bool {
        self.italic_angle.abs() > epsilon
    }

    pub fn caret(&self) -> (i32, i32) {
        if !self.slanted() {
            return (self.upem, 0);
        }
        (self.upem, (self.upem as f64 * (-self.italic_angle).to_radians().tan()).round_ties_even() as i32)
    }

    pub fn of(components: &[&Component], italic: bool) -> Metrics {
        let base = &components[0].font;
        let os2 = base.read::<read_fonts::tables::os2::Os2>().expect("missing OS/2");
        let hhea = base.read::<read_fonts::tables::hhea::Hhea>().expect("missing hhea");
        let post = base.read::<read_fonts::tables::post::Post>().expect("missing post");
        let upem = base.upem() as i32;

        let mut ascents = Vec::new();
        let mut descents = Vec::new();
        for component in components {
            let other = component.font.read::<read_fonts::tables::os2::Os2>().expect("missing OS/2");
            ascents.push((other.us_win_ascent() as i32).abs());
            descents.push((other.us_win_descent() as i32).abs());
        }

        Metrics {
            upem,
            ascender: (os2.s_typo_ascender() as i32).abs(),
            descender: -(os2.s_typo_descender() as i32).abs(),
            gap: (os2.s_typo_line_gap() as i32).abs(),
            line_ascender: (hhea.ascender().to_i16() as i32).abs(),
            line_descender: -(hhea.descender().to_i16() as i32).abs(),
            line_gap: (hhea.line_gap().to_i16() as i32).abs(),
            window_ascent: ascents.iter().copied().max().unwrap_or(0),
            window_descent: descents.iter().copied().max().unwrap_or(0),
            typo_metrics: os2.fs_selection().contains(SelectionFlags::USE_TYPO_METRICS),
            cap_height: match os2.s_cap_height() {
                Some(value) if value != 0 => value as i32,
                _ => (upem as f64 * 0.70).round_ties_even() as i32,
            },
            x_height: match os2.sx_height() {
                Some(value) if value != 0 => value as i32,
                _ => (upem as f64 * 0.52).round_ties_even() as i32,
            },
            italic_angle: if italic { -post.italic_angle().to_f64().abs() } else { 0.0 },
            underline_position: post.underline_position().to_i16() as i32,
            underline_thickness: post.underline_thickness().to_i16() as i32,
        }
    }

    pub fn apply(&self, font: &mut Font, family: &Family, style: &Style, revision: f64, advance: Option<u16>) {
        self.header(font, style, revision);
        self.horizontal(font);
        self.selection(font, family, style, advance);
        self.outline(font, family);
        self.smoothing(font);
    }

    pub fn header(&self, font: &mut Font, style: &Style, revision: f64) {
        let mut head: write_fonts::tables::head::Head = font.read::<read_fonts::tables::head::Head>().expect("missing head").to_owned_table();
        head.font_revision = Fixed::from_f64(revision);
        head.mac_style = write_fonts::tables::head::MacStyle::from_bits_truncate(
            (if style.bold() { 0x01 } else { 0 }) | (if style.italic() { 0x02 } else { 0 }),
        );
        head.lowest_rec_ppem = 8;
        head.font_direction_hint = 2;
        font.put(tags::HEAD, &head);
    }

    pub fn horizontal(&self, font: &mut Font) {
        let (rise, run) = self.caret();

        let mut hhea: write_fonts::tables::hhea::Hhea = font.read::<read_fonts::tables::hhea::Hhea>().expect("missing hhea").to_owned_table();
        hhea.ascender = (self.line_ascender as i16).into();
        hhea.descender = (self.line_descender as i16).into();
        hhea.line_gap = (self.line_gap as i16).into();
        hhea.caret_slope_rise = rise as i16;
        hhea.caret_slope_run = run as i16;
        hhea.caret_offset = 0;
        font.put(tags::HHEA, &hhea);

        if font.contains(tags::VHEA) {
            let mut vhea = font.get(tags::VHEA).expect("missing vhea").to_vec();
            vhea[8..10].copy_from_slice(&0i16.to_be_bytes());
            vhea[18..20].copy_from_slice(&0i16.to_be_bytes());
            vhea[20..22].copy_from_slice(&1i16.to_be_bytes());
            vhea[22..24].copy_from_slice(&0i16.to_be_bytes());
            font.set(tags::VHEA, vhea);
        }
    }

    pub fn selection(&self, font: &mut Font, family: &Family, style: &Style, advance: Option<u16>) {
        let mut os2: write_fonts::tables::os2::Os2 = font.read::<read_fonts::tables::os2::Os2>().expect("missing OS/2").to_owned_table();
        os2.us_weight_class = style.value() as u16;
        os2.us_width_class = 5;
        os2.fs_type = 0;

        os2.s_typo_ascender = self.ascender as i16;
        os2.s_typo_descender = self.descender as i16;
        os2.s_typo_line_gap = self.gap as i16;
        os2.us_win_ascent = self.window_ascent as u16;
        os2.us_win_descent = self.window_descent as u16;
        os2.s_cap_height = Some(self.cap_height as i16);
        os2.sx_height = Some(self.x_height as i16);

        let round = |value: f64| value.round_ties_even() as i16;
        os2.y_subscript_x_size = round(self.upem as f64 * 0.65);
        os2.y_subscript_y_size = round(self.upem as f64 * 0.60);
        os2.y_subscript_x_offset = 0;
        os2.y_subscript_y_offset = round(self.upem as f64 * 0.075);
        os2.y_superscript_x_size = round(self.upem as f64 * 0.65);
        os2.y_superscript_y_size = round(self.upem as f64 * 0.60);
        os2.y_superscript_x_offset = 0;
        os2.y_superscript_y_offset = round(self.upem as f64 * 0.35);
        os2.y_strikeout_size = round(self.upem as f64 * 0.05).max(1);
        os2.y_strikeout_position = round(self.x_height as f64 * 0.55);

        let mut selection = if self.typo_metrics { SelectionFlags::USE_TYPO_METRICS } else { SelectionFlags::empty() };
        if style.italic() {
            selection |= SelectionFlags::ITALIC;
        }
        if style.bold() {
            selection |= SelectionFlags::BOLD;
        }
        if !style.italic() && !style.bold() {
            selection |= SelectionFlags::REGULAR;
        }
        os2.fs_selection = selection;

        os2.panose_10[0] = 2;
        os2.panose_10[1] = if family.typeface == "Serif" { 2 } else { 11 };
        os2.panose_10[3] = if family.monospace { 9 } else { 3 };

        os2.ach_vend_id = Tag::new(b"NRCN");
        os2.us_default_char = Some(0);
        os2.us_break_char = Some(32);
        os2.us_max_context = Some(ranges::max_context(font));

        let codepoints = font.cmap().keys().copied().collect();
        let unicode = ranges::unicode_ranges(&codepoints);
        os2.ul_unicode_range_1 = unicode[0];
        os2.ul_unicode_range_2 = unicode[1];
        os2.ul_unicode_range_3 = unicode[2];
        os2.ul_unicode_range_4 = unicode[3];
        let codepages = ranges::codepage_ranges(&codepoints);
        os2.ul_code_page_range_1 = Some(codepages[0]);
        os2.ul_code_page_range_2 = Some(codepages[1]);

        os2.us_first_char_index = codepoints.iter().next().map(|code| (*code).min(0xFFFF) as u16).unwrap_or(0xFFFF);
        os2.us_last_char_index = codepoints.iter().next_back().map(|code| (*code).min(0xFFFF) as u16).unwrap_or(0xFFFF);

        os2.x_avg_char_width = match advance {
            Some(value) => value as i16,
            None => ranges::average_width(font),
        };

        font.put(tags::OS2, &os2);
    }

    pub fn outline(&self, font: &mut Font, family: &Family) {
        let mut post: write_fonts::tables::post::Post = font.read::<read_fonts::tables::post::Post>().expect("missing post").to_owned_table();
        post.version = write_fonts::types::Version16Dot16::VERSION_3_0;
        post.italic_angle = Fixed::from_f64(self.italic_angle);
        post.underline_position = (self.underline_position as i16).into();
        post.underline_thickness = (self.underline_thickness as i16).into();
        post.is_fixed_pitch = if family.monospace { 1 } else { 0 };
        post.glyph_name_index = None;
        post.string_data = None;
        font.put(tags::POST, &post);
    }

    pub fn smoothing(&self, font: &mut Font) {
        let table = Gasp::new(1, 1, vec![GaspRange::new(0xFFFF, GaspRangeBehavior::from_bits_truncate(0x000A))]);
        font.put(tags::GASP, &table);
    }
}
