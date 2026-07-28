use std::collections::BTreeMap;

use read_fonts::{FontData, FontRead, FontRef, TopLevelTable};
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::types::Tag;
use write_fonts::{dump_table, FontBuilder};
use write_fonts::validate::Validate;
use write_fonts::FontWrite;

#[allow(non_upper_case_globals)]
pub const capacity: usize = 0xFFFF;

pub struct Font {
    pub tables: BTreeMap<Tag, Vec<u8>>,
}

#[derive(Clone, Copy)]
pub struct Metric {
    pub advance: u16,
    pub bearing: i16,
}

impl Font {
    pub fn new(data: &[u8]) -> Font {
        let reference = FontRef::new(data).expect("failed to parse font");
        let mut tables = BTreeMap::new();
        for record in reference.table_directory().table_records() {
            let tag = record.tag();
            if let Some(found) = reference.table_data(tag) {
                tables.insert(tag, found.as_bytes().to_vec());
            }
        }
        Font { tables }
    }

    pub fn data(&self) -> Vec<u8> {
        let mut builder = FontBuilder::new();
        for (tag, data) in &self.tables {
            builder.add_raw(*tag, data.clone());
        }
        builder.build()
    }

    pub fn contains(&self, tag: Tag) -> bool {
        self.tables.contains_key(&tag)
    }

    pub fn get(&self, tag: Tag) -> Option<&[u8]> {
        self.tables.get(&tag).map(|data| data.as_slice())
    }

    pub fn set(&mut self, tag: Tag, data: Vec<u8>) {
        self.tables.insert(tag, data);
    }

    pub fn put(&mut self, tag: Tag, table: &(impl FontWrite + Validate)) {
        self.set(tag, dump_table(table).expect("failed to serialize table"));
    }

    pub fn remove(&mut self, tag: Tag) {
        self.tables.remove(&tag);
    }

    pub fn read<'a, T: FontRead<'a> + TopLevelTable + read_fonts::ReadArgs<Args = ()>>(&'a self) -> Option<T> {
        let data = self.get(T::TAG)?;
        Some(T::read(FontData::new(data)).expect("failed to parse table"))
    }

    pub fn upem(&self) -> u16 {
        self.read::<read_fonts::tables::head::Head>().expect("missing head").units_per_em()
    }

    pub fn glyph_count(&self) -> usize {
        self.read::<read_fonts::tables::maxp::Maxp>().expect("missing maxp").num_glyphs() as usize
    }

    pub fn long_loca(&self) -> bool {
        self.read::<read_fonts::tables::head::Head>().expect("missing head").index_to_loc_format() == 1
    }

    pub fn glyphs(&self) -> Vec<Vec<u8>> {
        let loca = self.get(Tag::new(b"loca")).expect("missing loca");
        let glyf = self.get(Tag::new(b"glyf")).expect("missing glyf");
        let count = self.glyph_count();

        let offset = |index: usize| -> usize {
            if self.long_loca() {
                u32::from_be_bytes(loca[index * 4..index * 4 + 4].try_into().unwrap()) as usize
            } else {
                u16::from_be_bytes(loca[index * 2..index * 2 + 2].try_into().unwrap()) as usize * 2
            }
        };

        (0..count).map(|index| glyf[offset(index)..offset(index + 1)].to_vec()).collect()
    }

    pub fn set_glyphs(&mut self, glyphs: &[Vec<u8>]) {
        if glyphs.len() > capacity {
            panic!("{} glyphs given, more than the {} a font can hold", glyphs.len(), capacity);
        }

        let mut glyf = Vec::new();
        let mut offsets = Vec::with_capacity(glyphs.len() + 1);
        for glyph in glyphs {
            offsets.push(glyf.len());
            glyf.extend_from_slice(glyph);
            while glyf.len() % 4 != 0 {
                glyf.push(0);
            }
        }
        offsets.push(glyf.len());

        let mut loca = Vec::with_capacity(offsets.len() * 4);
        for offset in &offsets {
            loca.extend_from_slice(&(*offset as u32).to_be_bytes());
        }

        self.set(Tag::new(b"glyf"), glyf);
        self.set(Tag::new(b"loca"), loca);

        let mut head: write_fonts::tables::head::Head = self.read::<read_fonts::tables::head::Head>().expect("missing head").to_owned_table();
        head.index_to_loc_format = 1;
        self.put(Tag::new(b"head"), &head);

        let mut maxp: write_fonts::tables::maxp::Maxp = self.read::<read_fonts::tables::maxp::Maxp>().expect("missing maxp").to_owned_table();
        maxp.num_glyphs = glyphs.len() as u16;
        self.put(Tag::new(b"maxp"), &maxp);
    }

    pub fn metrics(&self, header: Tag, table: Tag) -> Vec<Metric> {
        let count = self.glyph_count();
        let data = self.get(table).expect("missing metrics table");
        let long = {
            let header = self.get(header).expect("missing metrics header");
            u16::from_be_bytes(header[34..36].try_into().unwrap()) as usize
        };

        let mut found = Vec::with_capacity(count);
        let mut advance = 0u16;
        for index in 0..count {
            let bearing;
            if index < long {
                advance = u16::from_be_bytes(data[index * 4..index * 4 + 2].try_into().unwrap());
                bearing = i16::from_be_bytes(data[index * 4 + 2..index * 4 + 4].try_into().unwrap());
            } else {
                let position = long * 4 + (index - long) * 2;
                bearing = i16::from_be_bytes(data[position..position + 2].try_into().unwrap());
            }
            found.push(Metric { advance, bearing });
        }
        found
    }

    pub fn set_metrics(&mut self, header: Tag, table: Tag, metrics: &[Metric]) {
        let mut last = metrics.len();
        if last > 1 {
            let advance = metrics[last - 1].advance;
            while metrics[last - 2].advance == advance {
                last -= 1;
                if last <= 1 {
                    last = 1;
                    break;
                }
            }
        }

        let mut data = Vec::with_capacity(last * 4 + (metrics.len() - last) * 2);
        for metric in &metrics[..last] {
            data.extend_from_slice(&metric.advance.to_be_bytes());
            data.extend_from_slice(&metric.bearing.to_be_bytes());
        }
        for metric in &metrics[last..] {
            data.extend_from_slice(&metric.bearing.to_be_bytes());
        }
        self.set(table, data);

        let mut found = self.get(header).expect("missing metrics header").to_vec();
        found[34..36].copy_from_slice(&(last as u16).to_be_bytes());
        self.set(header, found);
    }

    pub fn cmap(&self) -> BTreeMap<u32, u16> {
        let mut mapping = BTreeMap::new();
        let Some(cmap) = self.read::<read_fonts::tables::cmap::Cmap>() else {
            return mapping;
        };
        let Some((_, _, subtable)) = cmap.best_subtable() else {
            return mapping;
        };
        for (codepoint, glyph) in subtable.iter() {
            if glyph.to_u32() != 0 {
                mapping.entry(codepoint).or_insert(glyph.to_u32() as u16);
            }
        }
        mapping
    }
}

pub struct Segment {
    pub start: u16,
    pub end: u16,
    pub delta: u16,
    pub mapping: Vec<u16>,
}

impl Segment {
    pub fn new(start: u16, end: u16, delta: u16) -> Segment {
        Segment { start, end, delta, mapping: Vec::new() }
    }

    pub fn direct(&self) -> bool {
        self.mapping.is_empty()
    }

    pub fn count(&self) -> usize {
        self.end as usize - self.start as usize + 1
    }

    pub fn length(&self) -> usize {
        8 + self.mapping.len() * 2
    }

    pub fn absorbs(&self, other: &Segment) -> bool {
        let added = other.end as usize - self.end as usize;
        if self.direct() {
            self.count() + added < 4
        } else {
            added < 4
        }
    }

    pub fn absorb(&mut self, other: &Segment, mapping: &BTreeMap<u32, u16>) {
        if self.direct() {
            self.mapping = (self.start..=self.end).map(|code| code.wrapping_add(self.delta)).collect();
            self.delta = 0;
        }
        for code in self.end + 1..=other.end {
            self.mapping.push(mapping.get(&(code as u32)).copied().unwrap_or(0));
        }
        self.end = other.end;
    }

    pub fn trim(&mut self, end: u16) {
        self.end = end;
        if !self.direct() {
            self.mapping.truncate(self.count());
        }
    }
}

pub fn segments(mapping: &BTreeMap<u32, u16>) -> Vec<Segment> {
    let mut direct: Vec<Segment> = Vec::new();
    for (code, glyph) in mapping.iter().filter(|(code, _)| **code <= 0xFFFF).map(|(code, glyph)| (*code as u16, *glyph)) {
        match direct.last_mut() {
            Some(last) if (last.end as u32) + 1 == code as u32 && code.wrapping_add(last.delta) == glyph => last.end = code,
            _ => direct.push(Segment::new(code, code, glyph.wrapping_sub(code))),
        }
    }

    let mut packed: Vec<Segment> = Vec::new();
    for segment in direct {
        match packed.last_mut() {
            Some(last) if last.absorbs(&segment) => last.absorb(&segment, mapping),
            _ => packed.push(segment),
        }
    }
    packed
}

pub fn fit(segments: &mut Vec<Segment>, budget: usize) -> Option<u16> {
    let mut length = 24;
    let mut kept = 0;
    let mut dropped = None;

    for segment in segments.iter_mut() {
        if length + segment.length() <= budget {
            length += segment.length();
            kept += 1;
            continue;
        }

        let room = budget.saturating_sub(length + 8) / 2;
        if !segment.direct() && room > 0 {
            segment.trim(segment.start + room as u16 - 1);
            dropped = Some(segment.end + 1);
            kept += 1;
        } else {
            dropped = Some(segment.start);
        }
        break;
    }

    segments.truncate(kept);
    dropped
}

pub fn format4(segments: Vec<Segment>) -> Vec<u8> {
    let mut segments = segments;
    if segments.last().map(|segment| segment.end) != Some(0xFFFF) {
        segments.push(Segment::new(0xFFFF, 0xFFFF, 1));
    }

    let count = segments.len();
    let length = 16 + segments.iter().map(Segment::length).sum::<usize>();
    if length > 0xFFFF {
        panic!("a cmap format 4 subtable cannot state a length of {} bytes", length);
    }

    let mut data = Vec::with_capacity(length);
    data.extend_from_slice(&4u16.to_be_bytes());
    data.extend_from_slice(&(length as u16).to_be_bytes());
    data.extend_from_slice(&0u16.to_be_bytes());
    data.extend_from_slice(&((count * 2) as u16).to_be_bytes());
    let power = (count as f64).log2().floor() as u32;
    let search = 2u16.pow(power + 1);
    data.extend_from_slice(&search.to_be_bytes());
    data.extend_from_slice(&(power as u16).to_be_bytes());
    data.extend_from_slice(&((count * 2) as u16 - search).to_be_bytes());
    for segment in &segments {
        data.extend_from_slice(&segment.end.to_be_bytes());
    }
    data.extend_from_slice(&0u16.to_be_bytes());
    for segment in &segments {
        data.extend_from_slice(&segment.start.to_be_bytes());
    }
    for segment in &segments {
        data.extend_from_slice(&segment.delta.to_be_bytes());
    }
    let mut offset = 0;
    for (index, segment) in segments.iter().enumerate() {
        if segment.direct() {
            data.extend_from_slice(&0u16.to_be_bytes());
        } else {
            data.extend_from_slice(&((2 * (count - index) + 2 * offset) as u16).to_be_bytes());
            offset += segment.mapping.len();
        }
    }
    for segment in &segments {
        for glyph in &segment.mapping {
            data.extend_from_slice(&glyph.to_be_bytes());
        }
    }
    data
}

pub fn format12(mapping: &BTreeMap<u32, u16>) -> Vec<u8> {
    let mut groups: Vec<(u32, u32, u32)> = Vec::new();
    for (code, glyph) in mapping {
        match groups.last_mut() {
            Some((start, end, first)) if *end + 1 == *code && *first + (*code - *start) == *glyph as u32 => {
                *end = *code;
            }
            _ => groups.push((*code, *code, *glyph as u32)),
        }
    }

    let mut data = Vec::with_capacity(16 + groups.len() * 12);
    data.extend_from_slice(&12u16.to_be_bytes());
    data.extend_from_slice(&0u16.to_be_bytes());
    data.extend_from_slice(&((16 + groups.len() * 12) as u32).to_be_bytes());
    data.extend_from_slice(&0u32.to_be_bytes());
    data.extend_from_slice(&(groups.len() as u32).to_be_bytes());
    for (start, end, first) in &groups {
        data.extend_from_slice(&start.to_be_bytes());
        data.extend_from_slice(&end.to_be_bytes());
        data.extend_from_slice(&first.to_be_bytes());
    }
    data
}

pub fn charmap(mapping: &BTreeMap<u32, u16>) -> Vec<u8> {
    let mut segments = segments(mapping);
    let dropped = fit(&mut segments, 0xFFFF);
    if let Some(code) = dropped {
        eprintln!("cmap: the format 4 subtable holds the characters below U+{:04X} only; the rest are mapped by format 12 alone", code);
    }

    let plane = (!segments.is_empty()).then(|| format4(segments));
    let beyond = (mapping.keys().any(|code| *code > 0xFFFF) || dropped.is_some()).then(|| format12(mapping));

    let mut records: Vec<(u16, u16, usize)> = Vec::new();
    let mut subtables: Vec<&[u8]> = Vec::new();
    if let Some(data) = &plane {
        subtables.push(data);
        records.push((0, 3, 0));
        records.push((3, 1, 0));
    }
    if let Some(data) = &beyond {
        subtables.push(data);
        records.push((3, 10, subtables.len() - 1));
    }
    records.sort();

    let mut offsets = Vec::new();
    let mut position = 4 + records.len() * 8;
    for data in &subtables {
        offsets.push(position);
        position += data.len();
    }

    let mut table = Vec::with_capacity(position);
    table.extend_from_slice(&0u16.to_be_bytes());
    table.extend_from_slice(&(records.len() as u16).to_be_bytes());
    for (platform, encoding, index) in &records {
        table.extend_from_slice(&platform.to_be_bytes());
        table.extend_from_slice(&encoding.to_be_bytes());
        table.extend_from_slice(&(offsets[*index] as u32).to_be_bytes());
    }
    for data in &subtables {
        table.extend_from_slice(data);
    }
    table
}

pub struct Extent {
    pub minimum_x: f64,
    pub minimum_y: f64,
    pub maximum_x: f64,
    pub maximum_y: f64,
    pub any: bool,
}

impl skrifa::outline::OutlinePen for Extent {
    fn move_to(&mut self, x: f32, y: f32) {
        self.include(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.include(x, y);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.include(cx0, cy0);
        self.include(x, y);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.include(cx0, cy0);
        self.include(cx1, cy1);
        self.include(x, y);
    }

    fn close(&mut self) {}
}

impl Extent {
    pub fn new() -> Extent {
        Extent { minimum_x: f64::MAX, minimum_y: f64::MAX, maximum_x: f64::MIN, maximum_y: f64::MIN, any: false }
    }

    pub fn include(&mut self, x: f32, y: f32) {
        self.any = true;
        self.minimum_x = self.minimum_x.min(x as f64);
        self.minimum_y = self.minimum_y.min(y as f64);
        self.maximum_x = self.maximum_x.max(x as f64);
        self.maximum_y = self.maximum_y.max(y as f64);
    }
}

impl Default for Extent {
    fn default() -> Extent {
        Extent::new()
    }
}

pub struct Limits {
    pub points: u16,
    pub contours: u16,
    pub composite_points: u16,
    pub composite_contours: u16,
    pub elements: u16,
    pub depth: u16,
    pub instructions: u16,
}

impl Limits {
    pub fn new() -> Limits {
        Limits { points: 0, contours: 0, composite_points: 0, composite_contours: 0, elements: 0, depth: 0, instructions: 0 }
    }

    pub fn include(&mut self, glyf: &read_fonts::tables::glyf::Glyf, loca: &read_fonts::tables::loca::Loca, glyph: u32) {
        use read_fonts::tables::glyf::Glyph;

        let Ok(Some(parsed)) = loca.get_glyf(read_fonts::types::GlyphId::new(glyph), glyf) else {
            return;
        };

        let (points, contours, depth) = Font::count(glyf, loca, glyph);
        match parsed {
            Glyph::Simple(simple) => {
                self.points = self.points.max(points);
                self.contours = self.contours.max(contours);
                self.instructions = self.instructions.max(simple.instructions().len() as u16);
            }
            Glyph::Composite(composite) => {
                let (elements, instructions) = composite.count_and_instructions();
                self.composite_points = self.composite_points.max(points);
                self.composite_contours = self.composite_contours.max(contours);
                self.elements = self.elements.max(elements as u16);
                self.depth = self.depth.max(depth);
                self.instructions = self.instructions.max(instructions.map(|found| found.len()).unwrap_or(0) as u16);
            }
        }
    }

    pub fn apply(&self, font: &mut Font) {
        let mut maxp: write_fonts::tables::maxp::Maxp = font.read::<read_fonts::tables::maxp::Maxp>().expect("missing maxp").to_owned_table();
        maxp.max_points = Some(self.points);
        maxp.max_contours = Some(self.contours);
        maxp.max_composite_points = Some(self.composite_points);
        maxp.max_composite_contours = Some(self.composite_contours);
        maxp.max_size_of_instructions = Some(self.instructions);
        maxp.max_component_elements = Some(self.elements);
        maxp.max_component_depth = Some(self.depth);
        font.put(Tag::new(b"maxp"), &maxp);
    }
}

impl Default for Limits {
    fn default() -> Limits {
        Limits::new()
    }
}

impl Font {
    pub fn count(glyf: &read_fonts::tables::glyf::Glyf, loca: &read_fonts::tables::loca::Loca, glyph: u32) -> (u16, u16, u16) {
        use read_fonts::tables::glyf::Glyph;

        let Ok(Some(parsed)) = loca.get_glyf(read_fonts::types::GlyphId::new(glyph), glyf) else {
            return (0, 0, 0);
        };

        match parsed {
            Glyph::Simple(simple) => {
                let ends = simple.end_pts_of_contours();
                let points = ends.last().map(|end| end.get() as u32 + 1).unwrap_or(0);
                (points as u16, ends.len() as u16, 0)
            }
            Glyph::Composite(composite) => {
                let (mut points, mut contours, mut depth) = (0u16, 0u16, 0u16);
                for component in composite.components() {
                    let (inner, found, nested) = Font::count(glyf, loca, component.glyph.to_u32());
                    points += inner;
                    contours += found;
                    depth = depth.max(nested);
                }
                (points, contours, depth + 1)
            }
        }
    }

    pub fn resolve(glyf: &read_fonts::tables::glyf::Glyf, loca: &read_fonts::tables::loca::Loca, glyph: u32, affine: [f64; 6], extent: &mut Extent) {
        use read_fonts::tables::glyf::{Anchor, Glyph};

        let Ok(Some(parsed)) = loca.get_glyf(read_fonts::types::GlyphId::new(glyph), glyf) else {
            return;
        };

        match parsed {
            Glyph::Simple(simple) => {
                for point in simple.points() {
                    let (x, y) = (point.x as f64, point.y as f64);
                    extent.include((affine[0] * x + affine[2] * y + affine[4]) as f32, (affine[1] * x + affine[3] * y + affine[5]) as f32);
                }
            }
            Glyph::Composite(composite) => {
                for component in composite.components() {
                    let (dx, dy) = match component.anchor {
                        Anchor::Offset { x, y } => (x as f64, y as f64),
                        Anchor::Point { .. } => (0.0, 0.0),
                    };
                    let t = component.transform;
                    let local = [t.xx.to_f32() as f64, t.xy.to_f32() as f64, t.yx.to_f32() as f64, t.yy.to_f32() as f64, dx, dy];
                    let composed = [
                        affine[0] * local[0] + affine[2] * local[1],
                        affine[1] * local[0] + affine[3] * local[1],
                        affine[0] * local[2] + affine[2] * local[3],
                        affine[1] * local[2] + affine[3] * local[3],
                        affine[0] * local[4] + affine[2] * local[5] + affine[4],
                        affine[1] * local[4] + affine[3] * local[5] + affine[5],
                    ];
                    Font::resolve(glyf, loca, component.glyph.to_u32(), composed, extent);
                }
            }
        }
    }

    pub fn finalize(&mut self) {
        if !self.contains(Tag::new(b"glyf")) {
            return;
        }

        let data = self.data();
        let reference = FontRef::new(&data).expect("failed to parse font");
        use read_fonts::TableProvider;
        let glyf = reference.glyf().expect("missing glyf");
        let loca = reference.loca(None).expect("missing loca");

        let count = self.glyph_count();
        let mut glyphs = self.glyphs();
        let metrics = self.metrics(Tag::new(b"hhea"), Tag::new(b"hmtx"));

        let mut extents: Vec<Option<Extent>> = Vec::with_capacity(count);
        let mut font_extent = Extent::new();
        let mut limits = Limits::new();

        for index in 0..count {
            let mut extent = Extent::new();
            Font::resolve(&glyf, &loca, index as u32, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], &mut extent);
            limits.include(&glyf, &loca, index as u32);

            if extent.any && !glyphs[index].is_empty() {
                let round = |value: f64| -> i16 { value.round_ties_even() as i16 };
                glyphs[index][2..4].copy_from_slice(&round(extent.minimum_x).to_be_bytes());
                glyphs[index][4..6].copy_from_slice(&round(extent.minimum_y).to_be_bytes());
                glyphs[index][6..8].copy_from_slice(&round(extent.maximum_x).to_be_bytes());
                glyphs[index][8..10].copy_from_slice(&round(extent.maximum_y).to_be_bytes());
                font_extent.include(extent.minimum_x as f32, extent.minimum_y as f32);
                font_extent.include(extent.maximum_x as f32, extent.maximum_y as f32);
                extents.push(Some(extent));
            } else {
                extents.push(None);
            }
        }

        self.set_glyphs(&glyphs);
        limits.apply(self);

        let mut head = self.get(Tag::new(b"head")).expect("missing head").to_vec();
        let bounds = if font_extent.any {
            (font_extent.minimum_x as i16, font_extent.minimum_y as i16, font_extent.maximum_x as i16, font_extent.maximum_y as i16)
        } else {
            (0, 0, 0, 0)
        };
        head[36..38].copy_from_slice(&bounds.0.to_be_bytes());
        head[38..40].copy_from_slice(&bounds.1.to_be_bytes());
        head[40..42].copy_from_slice(&bounds.2.to_be_bytes());
        head[42..44].copy_from_slice(&bounds.3.to_be_bytes());
        self.set(Tag::new(b"head"), head);

        let mut widest = 0u16;
        let (mut left, mut right, mut reach) = (i32::MAX, i32::MAX, i32::MIN);
        for (index, metric) in metrics.iter().enumerate() {
            widest = widest.max(metric.advance);
            if let Some(extent) = &extents[index] {
                let width = extent.maximum_x as i32 - extent.minimum_x as i32;
                left = left.min(metric.bearing as i32);
                right = right.min(metric.advance as i32 - metric.bearing as i32 - width);
                reach = reach.max(metric.bearing as i32 + width);
            }
        }

        let mut hhea = self.get(Tag::new(b"hhea")).expect("missing hhea").to_vec();
        hhea[10..12].copy_from_slice(&widest.to_be_bytes());
        hhea[12..14].copy_from_slice(&(if left == i32::MAX { 0 } else { left } as i16).to_be_bytes());
        hhea[14..16].copy_from_slice(&(if right == i32::MAX { 0 } else { right } as i16).to_be_bytes());
        hhea[16..18].copy_from_slice(&(if reach == i32::MIN { 0 } else { reach } as i16).to_be_bytes());
        self.set(Tag::new(b"hhea"), hhea);

        if self.contains(Tag::new(b"vmtx")) && self.contains(Tag::new(b"vhea")) {
            let vertical = self.metrics(Tag::new(b"vhea"), Tag::new(b"vmtx"));
            let mut tallest = 0u16;
            let (mut top, mut bottom, mut drop) = (i32::MAX, i32::MAX, i32::MIN);
            for (index, metric) in vertical.iter().enumerate() {
                tallest = tallest.max(metric.advance);
                if let Some(extent) = &extents[index] {
                    let height = extent.maximum_y as i32 - extent.minimum_y as i32;
                    top = top.min(metric.bearing as i32);
                    bottom = bottom.min(metric.advance as i32 - metric.bearing as i32 - height);
                    drop = drop.max(metric.bearing as i32 + height);
                }
            }

            let mut vhea = self.get(Tag::new(b"vhea")).expect("missing vhea").to_vec();
            vhea[10..12].copy_from_slice(&tallest.to_be_bytes());
            vhea[12..14].copy_from_slice(&(if top == i32::MAX { 0 } else { top } as i16).to_be_bytes());
            vhea[14..16].copy_from_slice(&(if bottom == i32::MAX { 0 } else { bottom } as i16).to_be_bytes());
            vhea[16..18].copy_from_slice(&(if drop == i32::MIN { 0 } else { drop } as i16).to_be_bytes());
            self.set(Tag::new(b"vhea"), vhea);
        }
    }
}

pub struct Points {
    pub coordinates: Vec<kurbo::Point>,
    pub ends: Vec<usize>,
    pub composite: bool,
}

impl Points {
    pub fn of(glyph: Option<&read_fonts::tables::glyf::Glyph>, metric: &Metric, vertical: Option<&Metric>) -> Points {
        use read_fonts::tables::glyf::{Anchor, Glyph};

        let mut coordinates = Vec::new();
        let mut ends = Vec::new();
        let mut composite = false;
        let (mut minimum_x, mut maximum_y) = (0i32, 0i32);

        match glyph {
            Some(Glyph::Simple(simple)) => {
                minimum_x = simple.x_min() as i32;
                maximum_y = simple.y_max() as i32;
                for point in simple.points() {
                    coordinates.push(kurbo::Point::new(point.x as f64, point.y as f64));
                }
                ends = simple.end_pts_of_contours().iter().map(|end| end.get() as usize).collect();
            }
            Some(Glyph::Composite(found)) => {
                composite = true;
                minimum_x = found.x_min() as i32;
                maximum_y = found.y_max() as i32;
                for (index, component) in found.components().enumerate() {
                    let (x, y) = match component.anchor {
                        Anchor::Offset { x, y } => (x as f64, y as f64),
                        Anchor::Point { .. } => (0.0, 0.0),
                    };
                    coordinates.push(kurbo::Point::new(x, y));
                    ends.push(index);
                }
            }
            None => {}
        }

        let left = minimum_x as f64 - metric.bearing as f64;
        let right = left + metric.advance as f64;
        let (top, bottom) = match vertical {
            Some(found) => {
                let top = found.bearing as f64 + maximum_y as f64;
                (top, top - found.advance as f64)
            }
            None => (0.0, 0.0),
        };

        coordinates.push(kurbo::Point::new(left, 0.0));
        coordinates.push(kurbo::Point::new(right, 0.0));
        coordinates.push(kurbo::Point::new(0.0, top));
        coordinates.push(kurbo::Point::new(0.0, bottom));

        Points { coordinates, ends, composite }
    }
}

pub mod tags {
    use write_fonts::types::Tag;

    pub const HEAD: Tag = Tag::new(b"head");
    pub const HHEA: Tag = Tag::new(b"hhea");
    pub const VHEA: Tag = Tag::new(b"vhea");
    pub const HMTX: Tag = Tag::new(b"hmtx");
    pub const VMTX: Tag = Tag::new(b"vmtx");
    pub const MAXP: Tag = Tag::new(b"maxp");
    pub const GLYF: Tag = Tag::new(b"glyf");
    pub const LOCA: Tag = Tag::new(b"loca");
    pub const GVAR: Tag = Tag::new(b"gvar");
    pub const CVAR: Tag = Tag::new(b"cvar");
    pub const FVAR: Tag = Tag::new(b"fvar");
    pub const AVAR: Tag = Tag::new(b"avar");
    pub const HVAR: Tag = Tag::new(b"HVAR");
    pub const VVAR: Tag = Tag::new(b"VVAR");
    pub const MVAR: Tag = Tag::new(b"MVAR");
    pub const STAT: Tag = Tag::new(b"STAT");
    pub const GDEF: Tag = Tag::new(b"GDEF");
    pub const GSUB: Tag = Tag::new(b"GSUB");
    pub const GPOS: Tag = Tag::new(b"GPOS");
    pub const CMAP: Tag = Tag::new(b"cmap");
    pub const NAME: Tag = Tag::new(b"name");
    pub const POST: Tag = Tag::new(b"post");
    pub const OS2: Tag = Tag::new(b"OS/2");
    pub const GASP: Tag = Tag::new(b"gasp");
    pub const CFF: Tag = Tag::new(b"CFF ");
    pub const CVT: Tag = Tag::new(b"cvt ");
    pub const FPGM: Tag = Tag::new(b"fpgm");
    pub const PREP: Tag = Tag::new(b"prep");
    pub const HDMX: Tag = Tag::new(b"hdmx");
    pub const LTSH: Tag = Tag::new(b"LTSH");
    pub const VDMX: Tag = Tag::new(b"VDMX");
}
