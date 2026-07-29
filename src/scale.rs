use kurbo::Vec2;
use read_fonts::tables::glyf::CurvePoint;
use read_fonts::{FontRef, TableProvider};
use write_fonts::from_obj::{FromTableRef, ToOwnedTable};
use write_fonts::tables::base::{Axis as BaseAxis, Base, BaseCoord, MinMax};
use write_fonts::tables::gdef::CaretValue;
use write_fonts::tables::glyf::{Anchor, Bbox, Glyph};
use write_fonts::tables::gpos::{
    AnchorTable, CursivePosFormat1, ExtensionSubtable, MarkArray, MarkBasePosFormat1, MarkLigPosFormat1, MarkMarkPosFormat1, PairPos,
    PositionLookup, SinglePos, ValueRecord,
};
use write_fonts::tables::gvar::{GlyphDelta, GlyphDeltas, GlyphVariations, Gvar, Tent};
use write_fonts::tables::variations::{ItemVariationData, ItemVariationStore};
use write_fonts::types::{F2Dot14, FWord, GlyphId, UfWord};

use crate::design::iup_delta;
use crate::font::{tags, Font, Metric, Points};

pub struct Scaler {
    pub factor: f64,
}

impl Scaler {
    pub fn new(factor: f64) -> Scaler {
        Scaler { factor }
    }

    pub fn round(&self, value: f64) -> i32 {
        (value * self.factor + 0.5).floor() as i32
    }

    pub fn apply(&self, font: &mut Font) {
        self.head(font);
        self.metrics(font);
        self.headers(font);
        self.profile(font);
        self.post(font);
        self.baselines(font);
        self.outlines(font);
        self.variations(font);
        self.controls(font);
        self.positioning(font);
        self.definitions(font);
    }

    pub fn head(&self, font: &mut Font) {
        let head: Option<write_fonts::tables::head::Head> = font.read::<read_fonts::tables::head::Head>().map(|found| found.to_owned_table());
        let Some(mut head) = head else { return };
        head.units_per_em = self.round(head.units_per_em as f64) as u16;
        head.x_min = self.round(head.x_min as f64) as i16;
        head.y_min = self.round(head.y_min as f64) as i16;
        head.x_max = self.round(head.x_max as f64) as i16;
        head.y_max = self.round(head.y_max as f64) as i16;
        font.put(tags::HEAD, &head);
    }

    pub fn metrics(&self, font: &mut Font) {
        for (header, table) in [(tags::HHEA, tags::HMTX), (tags::VHEA, tags::VMTX)] {
            if !font.contains(header) || !font.contains(table) {
                continue;
            }
            let scaled: Vec<Metric> = font
                .metrics(header, table)
                .iter()
                .map(|metric| Metric {
                    advance: self.round(metric.advance as f64) as u16,
                    bearing: self.round(metric.bearing as f64) as i16,
                })
                .collect();
            font.set_metrics(header, table, &scaled);
        }
    }

    pub fn headers(&self, font: &mut Font) {
        let hhea: Option<write_fonts::tables::hhea::Hhea> = font.read::<read_fonts::tables::hhea::Hhea>().map(|found| found.to_owned_table());
        if let Some(mut hhea) = hhea {
            hhea.ascender = FWord::new(self.round(hhea.ascender.to_i16() as f64) as i16);
            hhea.descender = FWord::new(self.round(hhea.descender.to_i16() as f64) as i16);
            hhea.line_gap = FWord::new(self.round(hhea.line_gap.to_i16() as f64) as i16);
            hhea.advance_width_max = UfWord::new(self.round(hhea.advance_width_max.to_u16() as f64) as u16);
            hhea.min_left_side_bearing = FWord::new(self.round(hhea.min_left_side_bearing.to_i16() as f64) as i16);
            hhea.min_right_side_bearing = FWord::new(self.round(hhea.min_right_side_bearing.to_i16() as f64) as i16);
            hhea.x_max_extent = FWord::new(self.round(hhea.x_max_extent.to_i16() as f64) as i16);
            hhea.caret_offset = self.round(hhea.caret_offset as f64) as i16;
            font.put(tags::HHEA, &hhea);
        }

        let vhea: Option<write_fonts::tables::vhea::Vhea> = font.read::<read_fonts::tables::vhea::Vhea>().map(|found| found.to_owned_table());
        if let Some(mut vhea) = vhea {
            vhea.ascender = FWord::new(self.round(vhea.ascender.to_i16() as f64) as i16);
            vhea.descender = FWord::new(self.round(vhea.descender.to_i16() as f64) as i16);
            vhea.line_gap = FWord::new(self.round(vhea.line_gap.to_i16() as f64) as i16);
            vhea.advance_height_max = UfWord::new(self.round(vhea.advance_height_max.to_u16() as f64) as u16);
            vhea.min_top_side_bearing = FWord::new(self.round(vhea.min_top_side_bearing.to_i16() as f64) as i16);
            vhea.min_bottom_side_bearing = FWord::new(self.round(vhea.min_bottom_side_bearing.to_i16() as f64) as i16);
            vhea.y_max_extent = FWord::new(self.round(vhea.y_max_extent.to_i16() as f64) as i16);
            vhea.caret_offset = self.round(vhea.caret_offset as f64) as i16;
            font.put(tags::VHEA, &vhea);
        }
    }

    pub fn profile(&self, font: &mut Font) {
        let os2: Option<write_fonts::tables::os2::Os2> = font.read::<read_fonts::tables::os2::Os2>().map(|found| found.to_owned_table());
        let Some(mut os2) = os2 else { return };
        os2.x_avg_char_width = self.round(os2.x_avg_char_width as f64) as i16;
        os2.y_subscript_x_size = self.round(os2.y_subscript_x_size as f64) as i16;
        os2.y_subscript_y_size = self.round(os2.y_subscript_y_size as f64) as i16;
        os2.y_subscript_x_offset = self.round(os2.y_subscript_x_offset as f64) as i16;
        os2.y_subscript_y_offset = self.round(os2.y_subscript_y_offset as f64) as i16;
        os2.y_superscript_x_size = self.round(os2.y_superscript_x_size as f64) as i16;
        os2.y_superscript_y_size = self.round(os2.y_superscript_y_size as f64) as i16;
        os2.y_superscript_x_offset = self.round(os2.y_superscript_x_offset as f64) as i16;
        os2.y_superscript_y_offset = self.round(os2.y_superscript_y_offset as f64) as i16;
        os2.y_strikeout_size = self.round(os2.y_strikeout_size as f64) as i16;
        os2.y_strikeout_position = self.round(os2.y_strikeout_position as f64) as i16;
        os2.s_typo_ascender = self.round(os2.s_typo_ascender as f64) as i16;
        os2.s_typo_descender = self.round(os2.s_typo_descender as f64) as i16;
        os2.s_typo_line_gap = self.round(os2.s_typo_line_gap as f64) as i16;
        os2.us_win_ascent = self.round(os2.us_win_ascent as f64) as u16;
        os2.us_win_descent = self.round(os2.us_win_descent as f64) as u16;
        os2.sx_height = os2.sx_height.map(|value| self.round(value as f64) as i16);
        os2.s_cap_height = os2.s_cap_height.map(|value| self.round(value as f64) as i16);
        font.put(tags::OS2, &os2);
    }

    pub fn post(&self, font: &mut Font) {
        let post: Option<write_fonts::tables::post::Post> = font.read::<read_fonts::tables::post::Post>().map(|found| found.to_owned_table());
        let Some(mut post) = post else { return };
        post.underline_position = FWord::new(self.round(post.underline_position.to_i16() as f64) as i16);
        post.underline_thickness = FWord::new(self.round(post.underline_thickness.to_i16() as f64) as i16);
        font.put(tags::POST, &post);
    }

    pub fn baselines(&self, font: &mut Font) {
        let base: Option<Base> = font.read::<read_fonts::tables::base::Base>().map(|found| found.to_owned_table());
        let Some(mut base) = base else { return };
        for axis in [base.horiz_axis.as_mut(), base.vert_axis.as_mut()].into_iter().flatten() {
            self.direction(axis);
        }
        font.put(tags::BASE, &base);
    }

    pub fn direction(&self, axis: &mut BaseAxis) {
        for record in axis.base_script_list.base_script_records.iter_mut() {
            let script = &mut record.base_script;
            if let Some(values) = script.base_values.as_mut() {
                for coordinate in values.base_coords.iter_mut() {
                    self.baseline(coordinate);
                }
            }
            if let Some(extremes) = script.default_min_max.as_mut() {
                self.extremes(extremes);
            }
            for entry in script.base_lang_sys_records.iter_mut() {
                self.extremes(&mut entry.min_max);
            }
        }
    }

    pub fn extremes(&self, extremes: &mut MinMax) {
        for coordinate in [extremes.min_coord.as_mut(), extremes.max_coord.as_mut()].into_iter().flatten() {
            self.baseline(coordinate);
        }
        for record in extremes.feat_min_max_records.iter_mut() {
            for coordinate in [record.min_coord.as_mut(), record.max_coord.as_mut()].into_iter().flatten() {
                self.baseline(coordinate);
            }
        }
    }

    pub fn baseline(&self, coordinate: &mut BaseCoord) {
        match coordinate {
            BaseCoord::Format1(found) => found.coordinate = self.round(found.coordinate as f64) as i16,
            BaseCoord::Format2(found) => found.coordinate = self.round(found.coordinate as f64) as i16,
            BaseCoord::Format3(found) => found.coordinate = self.round(found.coordinate as f64) as i16,
        }
    }

    pub fn bounds(&self, bbox: Bbox) -> Bbox {
        Bbox {
            x_min: self.round(bbox.x_min as f64) as i16,
            y_min: self.round(bbox.y_min as f64) as i16,
            x_max: self.round(bbox.x_max as f64) as i16,
            y_max: self.round(bbox.y_max as f64) as i16,
        }
    }

    pub fn outlines(&self, font: &mut Font) {
        if !font.contains(tags::GLYF) {
            return;
        }

        let count = font.glyph_count();
        let mut glyphs = Vec::with_capacity(count);
        {
            let data = font.data();
            let reference = FontRef::new(&data).expect("failed to parse font");
            let glyf = reference.glyf().expect("missing glyf");
            let loca = reference.loca(None).expect("missing loca");

            for index in 0..count {
                let identifier = GlyphId::new(index as u32);
                let Some(parsed) = loca.get_glyf(identifier, &glyf).expect("failed to parse glyph") else {
                    glyphs.push(Vec::new());
                    continue;
                };
                let mut glyph = Glyph::from_table_ref(&parsed);
                match &mut glyph {
                    Glyph::Simple(simple) => {
                        for contour in simple.contours.iter_mut() {
                            let moved: Vec<CurvePoint> = contour
                                .iter()
                                .map(|point| CurvePoint::new(self.round(point.x as f64) as i16, self.round(point.y as f64) as i16, point.on_curve))
                                .collect();
                            *contour = moved.into();
                        }
                        simple.bbox = self.bounds(simple.bbox);
                    }
                    Glyph::Composite(composite) => {
                        for component in composite.components_mut() {
                            if let Anchor::Offset { x, y } = &mut component.anchor {
                                *x = self.round(*x as f64) as i16;
                                *y = self.round(*y as f64) as i16;
                            }
                        }
                        composite.bbox = self.bounds(composite.bbox);
                    }
                    Glyph::Empty => {}
                }
                glyphs.push(if matches!(glyph, Glyph::Empty) { Vec::new() } else { write_fonts::dump_table(&glyph).expect("failed to serialize glyph") });
            }
        }

        font.set_glyphs(&glyphs);
    }

    pub fn variations(&self, font: &mut Font) {
        if !font.contains(tags::GVAR) {
            return;
        }

        let data = font.data();
        let reference = FontRef::new(&data).expect("failed to parse font");
        let gvar = reference.gvar().expect("missing gvar");
        let axis_count = match reference.fvar() {
            Ok(fvar) => fvar.axis_count(),
            Err(_) => gvar.axis_count(),
        };

        let glyf = reference.glyf().expect("missing glyf");
        let loca = reference.loca(None).expect("missing loca");
        let horizontal = font.metrics(tags::HHEA, tags::HMTX);
        let vertical = if font.contains(tags::VMTX) { Some(font.metrics(tags::VHEA, tags::VMTX)) } else { None };

        let count = font.glyph_count();
        let mut rebuilt = Vec::with_capacity(count);
        for index in 0..count {
            let identifier = GlyphId::new(index as u32);
            let variations = match gvar.glyph_variation_data(identifier) {
                Ok(Some(found)) => found,
                _ => {
                    rebuilt.push(GlyphVariations::new(identifier, Vec::new()));
                    continue;
                }
            };

            let glyph = loca.get_glyf(identifier, &glyf).expect("failed to parse glyph");
            let geometry = Points::of(glyph.as_ref(), &horizontal[index], vertical.as_ref().map(|found| &found[index]));
            let total = geometry.coordinates.len();

            let mut tuples = Vec::new();
            for tuple in variations.tuples() {
                let tents: Vec<Tent> = {
                    let peaks: Vec<F2Dot14> = tuple.peak().values().iter().map(|value| value.get()).collect();
                    match (tuple.intermediate_start(), tuple.intermediate_end()) {
                        (Some(start), Some(end)) => peaks
                            .iter()
                            .zip(start.values().iter().zip(end.values()))
                            .map(|(peak, (low, high))| Tent::new(*peak, Some((low.get(), high.get()))))
                            .collect(),
                        _ => peaks.iter().map(|peak| Tent::new(*peak, None)).collect(),
                    }
                };

                let mut deltas: Vec<Option<Vec2>> = vec![None; total];
                if tuple.has_deltas_for_all_points() {
                    for (position, delta) in tuple.deltas().enumerate() {
                        if position < total {
                            deltas[position] = Some(Vec2::new(self.round(delta.x_delta as f64) as f64, self.round(delta.y_delta as f64) as f64));
                        }
                    }
                } else {
                    for delta in tuple.deltas() {
                        let position = delta.position as usize;
                        if position < total {
                            deltas[position] = Some(Vec2::new(self.round(delta.x_delta as f64) as f64, self.round(delta.y_delta as f64) as f64));
                        }
                    }
                }

                let dense = iup_delta(&deltas, &geometry.coordinates, &geometry.ends);
                let flattened: Vec<GlyphDelta> = deltas
                    .iter()
                    .zip(&dense)
                    .map(|(delta, inferred)| match delta {
                        Some(value) => GlyphDelta::required(value.x as i16, value.y as i16),
                        None => GlyphDelta::optional(inferred.x.round() as i16, inferred.y.round() as i16),
                    })
                    .collect();
                tuples.push(GlyphDeltas::new(tents, flattened));
            }

            rebuilt.push(GlyphVariations::new(identifier, tuples));
        }

        let table = Gvar::new(rebuilt, axis_count).expect("failed to build gvar");
        font.put(tags::GVAR, &table);
    }

    pub fn controls(&self, font: &mut Font) {
        let scaled: Option<Vec<u8>> = font.get(tags::CVT).map(|data| {
            data.chunks_exact(2)
                .flat_map(|pair| (self.round(i16::from_be_bytes([pair[0], pair[1]]) as f64) as i16).to_be_bytes())
                .collect()
        });
        if let Some(scaled) = scaled {
            font.set(tags::CVT, scaled);
        }

        if self.factor != 1.0 {
            font.remove(tags::CVAR);
        }
    }

    pub fn positioning(&self, font: &mut Font) {
        let gpos: Option<write_fonts::tables::gpos::Gpos> = font.read::<read_fonts::tables::gpos::Gpos>().map(|found| found.to_owned_table());
        let Some(mut gpos) = gpos else { return };
        for lookup in gpos.lookup_list.lookups.iter_mut() {
            self.lookup(lookup);
        }
        font.put(tags::GPOS, &gpos);
    }

    pub fn lookup(&self, lookup: &mut PositionLookup) {
        match lookup {
            PositionLookup::Single(found) => found.subtables.iter_mut().for_each(|subtable| self.single(subtable)),
            PositionLookup::Pair(found) => found.subtables.iter_mut().for_each(|subtable| self.pair(subtable)),
            PositionLookup::Cursive(found) => found.subtables.iter_mut().for_each(|subtable| self.cursive(subtable)),
            PositionLookup::MarkToBase(found) => found.subtables.iter_mut().for_each(|subtable| self.base(subtable)),
            PositionLookup::MarkToLig(found) => found.subtables.iter_mut().for_each(|subtable| self.ligature(subtable)),
            PositionLookup::MarkToMark(found) => found.subtables.iter_mut().for_each(|subtable| self.mark(subtable)),
            PositionLookup::Contextual(_) | PositionLookup::ChainContextual(_) => {}
            PositionLookup::Extension(found) => found.subtables.iter_mut().for_each(|subtable| self.extension(subtable)),
        }
    }

    pub fn extension(&self, subtable: &mut ExtensionSubtable) {
        match subtable {
            ExtensionSubtable::Single(found) => self.single(&mut found.extension),
            ExtensionSubtable::Pair(found) => self.pair(&mut found.extension),
            ExtensionSubtable::Cursive(found) => self.cursive(&mut found.extension),
            ExtensionSubtable::MarkToBase(found) => self.base(&mut found.extension),
            ExtensionSubtable::MarkToLig(found) => self.ligature(&mut found.extension),
            ExtensionSubtable::MarkToMark(found) => self.mark(&mut found.extension),
            ExtensionSubtable::Contextual(_) | ExtensionSubtable::ChainContextual(_) => {}
        }
    }

    pub fn single(&self, subtable: &mut SinglePos) {
        match subtable {
            SinglePos::Format1(found) => self.value(&mut found.value_record),
            SinglePos::Format2(found) => found.value_records.iter_mut().for_each(|record| self.value(record)),
        }
    }

    pub fn pair(&self, subtable: &mut PairPos) {
        match subtable {
            PairPos::Format1(found) => {
                for set in found.pair_sets.iter_mut() {
                    for record in set.pair_value_records.iter_mut() {
                        self.value(&mut record.value_record1);
                        self.value(&mut record.value_record2);
                    }
                }
            }
            PairPos::Format2(found) => {
                for first in found.class1_records.iter_mut() {
                    for second in first.class2_records.iter_mut() {
                        self.value(&mut second.value_record1);
                        self.value(&mut second.value_record2);
                    }
                }
            }
        }
    }

    pub fn cursive(&self, subtable: &mut CursivePosFormat1) {
        for record in subtable.entry_exit_record.iter_mut() {
            if let Some(found) = record.entry_anchor.as_mut() {
                self.anchor(found);
            }
            if let Some(found) = record.exit_anchor.as_mut() {
                self.anchor(found);
            }
        }
    }

    pub fn base(&self, subtable: &mut MarkBasePosFormat1) {
        self.marks(&mut subtable.mark_array);
        for record in subtable.base_array.base_records.iter_mut() {
            for anchor in record.base_anchors.iter_mut() {
                if let Some(found) = anchor.as_mut() {
                    self.anchor(found);
                }
            }
        }
    }

    pub fn ligature(&self, subtable: &mut MarkLigPosFormat1) {
        self.marks(&mut subtable.mark_array);
        for attach in subtable.ligature_array.ligature_attaches.iter_mut() {
            for record in attach.component_records.iter_mut() {
                for anchor in record.ligature_anchors.iter_mut() {
                    if let Some(found) = anchor.as_mut() {
                        self.anchor(found);
                    }
                }
            }
        }
    }

    pub fn mark(&self, subtable: &mut MarkMarkPosFormat1) {
        self.marks(&mut subtable.mark1_array);
        for record in subtable.mark2_array.mark2_records.iter_mut() {
            for anchor in record.mark2_anchors.iter_mut() {
                if let Some(found) = anchor.as_mut() {
                    self.anchor(found);
                }
            }
        }
    }

    pub fn marks(&self, array: &mut MarkArray) {
        for record in array.mark_records.iter_mut() {
            self.anchor(&mut record.mark_anchor);
        }
    }

    pub fn value(&self, record: &mut ValueRecord) {
        record.x_placement = record.x_placement.map(|value| self.round(value as f64) as i16);
        record.y_placement = record.y_placement.map(|value| self.round(value as f64) as i16);
        record.x_advance = record.x_advance.map(|value| self.round(value as f64) as i16);
        record.y_advance = record.y_advance.map(|value| self.round(value as f64) as i16);
    }

    pub fn anchor(&self, anchor: &mut AnchorTable) {
        match anchor {
            AnchorTable::Format1(found) => {
                found.x_coordinate = self.round(found.x_coordinate as f64) as i16;
                found.y_coordinate = self.round(found.y_coordinate as f64) as i16;
            }
            AnchorTable::Format2(found) => {
                found.x_coordinate = self.round(found.x_coordinate as f64) as i16;
                found.y_coordinate = self.round(found.y_coordinate as f64) as i16;
            }
            AnchorTable::Format3(found) => {
                found.x_coordinate = self.round(found.x_coordinate as f64) as i16;
                found.y_coordinate = self.round(found.y_coordinate as f64) as i16;
            }
        }
    }

    pub fn definitions(&self, font: &mut Font) {
        let gdef: Option<write_fonts::tables::gdef::Gdef> = font.read::<read_fonts::tables::gdef::Gdef>().map(|found| found.to_owned_table());
        let Some(mut gdef) = gdef else { return };

        if let Some(list) = gdef.lig_caret_list.as_mut() {
            for glyph in list.lig_glyphs.iter_mut() {
                for value in glyph.caret_values.iter_mut() {
                    self.caret(value);
                }
            }
        }

        if let Some(store) = gdef.item_var_store.as_mut() {
            self.store(store);
        }

        font.put(tags::GDEF, &gdef);
    }

    pub fn caret(&self, value: &mut CaretValue) {
        match value {
            CaretValue::Format1(found) => found.coordinate = self.round(found.coordinate as f64) as i16,
            CaretValue::Format2(_) => {}
            CaretValue::Format3(found) => found.coordinate = self.round(found.coordinate as f64) as i16,
        }
    }

    pub fn store(&self, store: &mut ItemVariationStore) {
        for data in store.item_variation_data.iter_mut() {
            if let Some(found) = data.as_mut() {
                self.deltas(found);
            }
        }
    }

    pub fn deltas(&self, data: &mut ItemVariationData) {
        let regions = data.region_indexes.len();
        let long = data.word_delta_count & 0x8000 != 0;
        let words = (data.word_delta_count & 0x7FFF) as usize;
        let row = if long { words * 4 + (regions - words) * 2 } else { words * 2 + (regions - words) };

        let mut rows: Vec<Vec<i64>> = Vec::with_capacity(data.item_count as usize);
        for item in 0..data.item_count as usize {
            let bytes = &data.delta_sets[item * row..(item + 1) * row];
            let mut values = Vec::with_capacity(regions);
            let mut position = 0;
            for column in 0..regions {
                let value = match (column < words, long) {
                    (true, true) => {
                        position += 4;
                        i32::from_be_bytes(bytes[position - 4..position].try_into().unwrap()) as i64
                    }
                    (true, false) | (false, true) => {
                        position += 2;
                        i16::from_be_bytes(bytes[position - 2..position].try_into().unwrap()) as i64
                    }
                    (false, false) => {
                        position += 1;
                        bytes[position - 1] as i8 as i64
                    }
                };
                values.push(self.round(value as f64) as i64);
            }
            rows.push(values);
        }

        let huge = rows.iter().flatten().any(|value| *value > i16::MAX as i64 || *value < i16::MIN as i64);
        let narrow = |value: i64| if huge { i16::MIN as i64 <= value && value <= i16::MAX as i64 } else { i8::MIN as i64 <= value && value <= i8::MAX as i64 };

        let mut count = 0;
        for values in &rows {
            for (column, value) in values.iter().enumerate() {
                if !narrow(*value) {
                    count = count.max(column + 1);
                }
            }
        }

        let mut encoded = Vec::with_capacity(rows.len() * (count * 2 + regions));
        for values in &rows {
            for (column, value) in values.iter().enumerate() {
                match (column < count, huge) {
                    (true, true) => encoded.extend((*value as i32).to_be_bytes()),
                    (true, false) | (false, true) => encoded.extend((*value as i16).to_be_bytes()),
                    (false, false) => encoded.push((*value as i8) as u8),
                }
            }
        }

        data.word_delta_count = count as u16 | if huge { 0x8000 } else { 0 };
        data.delta_sets = encoded;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_noto_sans_jp() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/build/sources/noto/NotoSansJP.ttf");
        let data = std::fs::read(path).expect("missing test font");
        let mut font = Font::new(&data);

        Scaler::new(2048.0 / 1000.0).apply(&mut font);
        assert_eq!(font.upem(), 2048);

        let output = "/private/tmp/claude-501/-Volumes-Developments-nercone-dev-fonts/d08e5eec-1bbb-4368-8fb4-36df636f3bff/scratchpad/scale-test/rust.ttf";
        std::fs::create_dir_all(std::path::Path::new(output).parent().unwrap()).expect("failed to create output directory");
        std::fs::write(output, font.data()).expect("failed to write output font");
    }
}
