use std::collections::HashMap;

use kurbo::{Point, Vec2};
use read_fonts::{FontRef, TableProvider};
use write_fonts::from_obj::{FromTableRef, ToOwnedTable};
use write_fonts::tables::avar::{Avar, AxisValueMap, SegmentMaps};
use write_fonts::tables::gdef::{CaretValue, Gdef};
use write_fonts::tables::glyf::{Anchor, Bbox, Glyph};
use write_fonts::tables::gpos::{
    AnchorTable, CursivePosFormat1, ExtensionSubtable, Gpos, MarkBasePosFormat1, MarkLigPosFormat1,
    MarkMarkPosFormat1, PairPos, PositionLookup, SinglePos, ValueRecord,
};
use write_fonts::tables::gsub::Gsub;
use write_fonts::tables::gvar::iup::iup_delta_optimize;
use write_fonts::tables::gvar::{GlyphDelta, GlyphDeltas, GlyphVariations, Gvar, Tent};
use write_fonts::tables::hvar::Hvar;
use write_fonts::tables::layout::{
    Condition, ConditionFormat1, ConditionSet, DeviceOrVariationIndex, FeatureList,
    FeatureTableSubstitution, FeatureVariationRecord, FeatureVariations,
};
use write_fonts::tables::variations::ivs_builder::VariationStoreBuilder;
use write_fonts::tables::variations::{
    ItemVariationData, ItemVariationStore, RegionAxisCoordinates, VariationRegion,
    VariationRegionList,
};
use write_fonts::types::{F2Dot14, GlyphId, Tag};

use crate::design::{iup_delta, support_scalar, tolerance, Axis, Mapping, Space};
use crate::font::{tags, Font, Metric, Points};

pub fn otround(value: f64) -> i32 {
    (value + 0.5).floor() as i32
}

pub fn quantize(value: f64) -> f64 {
    otround(value * 16384.0) as f64 / 16384.0
}

pub fn piecewise(value: f64, mapping: &[(f64, f64)]) -> f64 {
    if mapping.is_empty() {
        return value;
    }
    if let Some((_, mapped)) = mapping.iter().find(|(from, _)| *from == value) {
        return *mapped;
    }

    let (first, lowest) = mapping[0];
    if value < first {
        return value + lowest - first;
    }
    let (last, highest) = mapping[mapping.len() - 1];
    if value > last {
        return value + highest - last;
    }

    let (left, lower) = *mapping.iter().filter(|(from, _)| *from < value).last().unwrap();
    let (right, upper) = *mapping.iter().find(|(from, _)| *from > value).unwrap();
    lower + (upper - lower) * (value - left) / (right - left)
}

pub type Region = Vec<(Tag, (f64, f64, f64))>;

pub struct Pin {
    pub value: f64,
}

impl Pin {
    pub fn new(value: f64) -> Pin {
        Pin { value }
    }

    pub fn axes(font: &Font) -> Vec<Tag> {
        let Some(fvar) = font.read::<read_fonts::tables::fvar::Fvar>() else {
            return Vec::new();
        };
        fvar.axes().expect("failed to parse fvar axes").iter().map(|entry| entry.axis_tag()).collect()
    }

    pub fn coordinate(&self, font: &Font) -> f64 {
        let Some(fvar) = font.read::<read_fonts::tables::fvar::Fvar>() else {
            return 0.0;
        };
        let mappings = Space::mappings(font);

        for (index, entry) in fvar.axes().expect("failed to parse fvar axes").iter().enumerate() {
            if entry.axis_tag() != Axis::tag() {
                continue;
            }
            let axis = Axis::new(entry.min_value().to_f64(), entry.default_value().to_f64(), entry.max_value().to_f64());
            let mut coordinate = axis.normalize(self.value);
            if let Some(pairs) = mappings.get(index) {
                if !pairs.is_empty() {
                    coordinate = piecewise(coordinate, pairs);
                }
            }
            return quantize(coordinate);
        }

        0.0
    }

    pub fn solve(tent: (f64, f64, f64), pin: f64) -> Option<f64> {
        let (lower, peak, upper) = tent;
        if !(lower <= peak && peak <= upper) || (lower < 0.0 && upper > 0.0) {
            return None;
        }
        let location = HashMap::from([(Axis::tag(), pin)]);
        let scalar = support_scalar(&location, &[(Axis::tag(), tent)]);
        if scalar == 0.0 {
            return None;
        }
        Some(scalar)
    }

    pub fn apply(&self, font: &mut Font) {
        let axes = Pin::axes(font);
        if !axes.contains(&Axis::tag()) {
            return;
        }
        let pin = self.coordinate(font);

        if font.contains(tags::GVAR) {
            self.outlines(font, pin, &axes);
        }
        self.store(font, pin, &axes);
        self.declare(font);
    }

    pub fn outlines(&self, font: &mut Font, pin: f64, axes: &[Tag]) {
        let remaining: Vec<Tag> = axes.iter().copied().filter(|tag| *tag != Axis::tag()).collect();

        let data = font.data();
        let reference = FontRef::new(&data).expect("failed to parse font");
        let glyf = reference.glyf().expect("missing glyf");
        let loca = reference.loca(None).expect("missing loca");
        let gvar = reference.gvar().expect("missing gvar");

        let horizontal = font.metrics(tags::HHEA, tags::HMTX);
        let vertical = if font.contains(tags::VMTX) { Some(font.metrics(tags::VHEA, tags::VMTX)) } else { None };
        let count = font.glyph_count();

        let mut instanced: Vec<Instanced> = Vec::with_capacity(count);
        for index in 0..count {
            let identifier = GlyphId::new(index as u32);
            let glyph = loca.get_glyf(identifier, &glyf).expect("failed to parse glyph");
            let points = Points::of(glyph.as_ref(), &horizontal[index], vertical.as_ref().map(|found| &found[index]));
            let total = points.coordinates.len();

            let mut components = Vec::new();
            if let Some(read_fonts::tables::glyf::Glyph::Composite(found)) = glyph.as_ref() {
                for component in found.components() {
                    let transform = component.transform;
                    components.push((
                        component.glyph.to_u32() as usize,
                        [transform.xx.to_f32() as f64, transform.xy.to_f32() as f64, transform.yx.to_f32() as f64, transform.yy.to_f32() as f64],
                        matches!(component.anchor, read_fonts::tables::glyf::Anchor::Offset { .. }),
                    ));
                }
            }

            let mut coordinates = points.coordinates.clone();
            let mut merged: Vec<(Region, Vec<Vec2>)> = Vec::new();

            if let Ok(Some(variations)) = gvar.glyph_variation_data(identifier) {
                for tuple in variations.tuples() {
                    let peaks: Vec<f64> = tuple.peak().values().iter().map(|value| value.get().to_f32() as f64).collect();
                    let (starts, ends): (Vec<f64>, Vec<f64>) = match (tuple.intermediate_start(), tuple.intermediate_end()) {
                        (Some(start), Some(end)) => (
                            start.values().iter().map(|value| value.get().to_f32() as f64).collect(),
                            end.values().iter().map(|value| value.get().to_f32() as f64).collect(),
                        ),
                        _ => (peaks.iter().map(|peak| peak.min(0.0)).collect(), peaks.iter().map(|peak| peak.max(0.0)).collect()),
                    };

                    let mut region: Region = Vec::new();
                    let mut scalar = 1.0;
                    let mut dropped = false;
                    for (position, tag) in axes.iter().enumerate() {
                        let triple = (starts[position], peaks[position], ends[position]);
                        if triple == (0.0, 0.0, 0.0) {
                            continue;
                        }
                        if *tag == Axis::tag() {
                            if triple.1 == 0.0 {
                                continue;
                            }
                            match Pin::solve(triple, pin) {
                                None => {
                                    dropped = true;
                                    break;
                                }
                                Some(found) => scalar = found,
                            }
                            continue;
                        }
                        region.push((*tag, triple));
                    }
                    if dropped {
                        continue;
                    }

                    let mut sparse: Vec<Option<Vec2>> = vec![None; total];
                    if tuple.has_deltas_for_all_points() {
                        for (position, delta) in tuple.deltas().enumerate() {
                            if position < total {
                                sparse[position] = Some(Vec2::new(delta.x_delta as f64, delta.y_delta as f64));
                            }
                        }
                    } else {
                        for delta in tuple.deltas() {
                            let position = delta.position as usize;
                            if position < total {
                                sparse[position] = Some(Vec2::new(delta.x_delta as f64, delta.y_delta as f64));
                            }
                        }
                    }

                    let dense: Vec<Vec2> = iup_delta(&sparse, &points.coordinates, &points.ends)
                        .into_iter()
                        .map(|delta| delta * scalar)
                        .collect();

                    match merged.iter_mut().find(|(found, _)| *found == region) {
                        Some((_, deltas)) => {
                            for (value, delta) in deltas.iter_mut().zip(&dense) {
                                *value += *delta;
                            }
                        }
                        None => merged.push((region, dense)),
                    }
                }
            }

            let mut tuples = Vec::new();
            for (region, deltas) in merged {
                if region.is_empty() {
                    for (value, delta) in coordinates.iter_mut().zip(&deltas) {
                        *value += *delta;
                    }
                } else {
                    tuples.push((region, deltas));
                }
            }

            instanced.push(Instanced { coordinates, ends: points.ends, tuples, components });
        }

        let mut cache: Vec<Option<Vec<Point>>> = vec![None; count];
        let boxes: Vec<(i32, i32, i32, i32)> = (0..count).map(|index| Instanced::bounds(index, &instanced, &mut cache)).collect();

        let mut glyphs = Vec::with_capacity(count);
        let mut widths = Vec::with_capacity(count);
        let mut heights = Vec::with_capacity(count);
        for index in 0..count {
            let entry = &instanced[index];
            let total = entry.coordinates.len();
            let (left, right) = (entry.coordinates[total - 4].x, entry.coordinates[total - 3].x);
            let (top, bottom) = (entry.coordinates[total - 2].y, entry.coordinates[total - 1].y);
            let (minimum_x, _, _, maximum_y) = boxes[index];

            widths.push(Metric {
                advance: otround(right - left).max(0) as u16,
                bearing: otround(minimum_x as f64 - left) as i16,
            });
            heights.push(Metric {
                advance: otround(top - bottom).max(0) as u16,
                bearing: otround(top - maximum_y as f64) as i16,
            });

            let identifier = GlyphId::new(index as u32);
            let Some(parsed) = loca.get_glyf(identifier, &glyf).expect("failed to parse glyph") else {
                glyphs.push(Vec::new());
                continue;
            };
            let bbox = Bbox {
                x_min: boxes[index].0 as i16,
                y_min: boxes[index].1 as i16,
                x_max: boxes[index].2 as i16,
                y_max: boxes[index].3 as i16,
            };
            let mut glyph = Glyph::from_table_ref(&parsed);
            match &mut glyph {
                Glyph::Simple(simple) => {
                    let mut position = 0;
                    for contour in simple.contours.iter_mut() {
                        let moved: Vec<read_fonts::tables::glyf::CurvePoint> = contour
                            .iter()
                            .map(|point| {
                                let value = read_fonts::tables::glyf::CurvePoint::new(
                                    otround(entry.coordinates[position].x) as i16,
                                    otround(entry.coordinates[position].y) as i16,
                                    point.on_curve,
                                );
                                position += 1;
                                value
                            })
                            .collect();
                        *contour = moved.into();
                    }
                    simple.bbox = bbox;
                }
                Glyph::Composite(composite) => {
                    for (position, component) in composite.components_mut().iter_mut().enumerate() {
                        if let Anchor::Offset { x, y } = &mut component.anchor {
                            *x = otround(entry.coordinates[position].x) as i16;
                            *y = otround(entry.coordinates[position].y) as i16;
                        }
                    }
                    composite.bbox = bbox;
                }
                Glyph::Empty => {}
            }
            glyphs.push(if matches!(glyph, Glyph::Empty) { Vec::new() } else { write_fonts::dump_table(&glyph).expect("failed to serialize glyph") });
        }

        let mut rebuilt = Vec::with_capacity(count);
        for (index, entry) in instanced.iter().enumerate() {
            let identifier = GlyphId::new(index as u32);
            let origins: Vec<kurbo13::Point> = entry
                .coordinates
                .iter()
                .map(|point| kurbo13::Point::new(otround(point.x) as f64, otround(point.y) as f64))
                .collect();

            let mut tuples = Vec::new();
            for (region, deltas) in &entry.tuples {
                let rounded: Vec<kurbo13::Vec2> = deltas
                    .iter()
                    .map(|delta| kurbo13::Vec2::new(otround(delta.x) as f64, otround(delta.y) as f64))
                    .collect();
                if rounded.iter().all(|delta| delta.x == 0.0 && delta.y == 0.0) {
                    continue;
                }
                let optimized = iup_delta_optimize(rounded, origins.clone(), tolerance, &entry.ends).expect("failed to optimize deltas");
                let tents: Vec<Tent> = remaining
                    .iter()
                    .map(|tag| match region.iter().find(|(found, _)| found == tag) {
                        Some((_, (lower, peak, upper))) => {
                            if *lower == peak.min(0.0) && *upper == peak.max(0.0) {
                                Tent::new(F2Dot14::from_f32(*peak as f32), None)
                            } else {
                                Tent::new(
                                    F2Dot14::from_f32(*peak as f32),
                                    Some((F2Dot14::from_f32(*lower as f32), F2Dot14::from_f32(*upper as f32))),
                                )
                            }
                        }
                        None => Tent::new(F2Dot14::from_f32(0.0), None),
                    })
                    .collect();
                tuples.push(GlyphDeltas::new(tents, optimized));
            }
            rebuilt.push(GlyphVariations::new(identifier, tuples));
        }
        let table = Gvar::new(rebuilt, remaining.len() as u16).expect("failed to build gvar");

        font.set_glyphs(&glyphs);
        font.set_metrics(tags::HHEA, tags::HMTX, &widths);
        if vertical.is_some() {
            font.set_metrics(tags::VHEA, tags::VMTX, &heights);
        }
        font.put(tags::GVAR, &table);
    }

    pub fn store(&self, font: &mut Font, pin: f64, axes: &[Tag]) {
        let Some(gdef) = font.read::<read_fonts::tables::gdef::Gdef>() else {
            return;
        };
        let mut owned: Gdef = gdef.to_owned_table();
        let mut deltas: HashMap<u32, f64> = HashMap::new();

        if let Some(Ok(store)) = gdef.item_var_store() {
            let remaining: Vec<Tag> = axes.iter().copied().filter(|tag| *tag != Axis::tag()).collect();
            let supports: Vec<Region> = Mapping::regions(store.offset_data().as_bytes())
                .iter()
                .map(|region| {
                    axes.iter()
                        .copied()
                        .zip(region.iter().copied())
                        .filter(|(_, (_, peak, _))| *peak != 0.0)
                        .collect()
                })
                .collect();

            let mut regionlist: Vec<Region> = Vec::new();
            let mut rebuilt: Vec<Option<ItemVariationData>> = Vec::new();

            for (outer, data) in store.item_variation_data().iter().enumerate() {
                let Some(Ok(data)) = data else {
                    rebuilt.push(None);
                    continue;
                };
                let items = data.item_count() as usize;
                let rows: Vec<Vec<f64>> = (0..items)
                    .map(|inner| data.delta_set(inner as u16).map(|value| value as f64).collect())
                    .collect();

                let mut columns: Vec<(Region, Vec<f64>)> = Vec::new();
                let mut folded = vec![0.0; items];
                for (column, entry) in data.region_indexes().iter().enumerate() {
                    let mut region = supports[entry.get() as usize].clone();
                    let mut scalar = 1.0;
                    if let Some(position) = region.iter().position(|(tag, _)| *tag == Axis::tag()) {
                        match Pin::solve(region[position].1, pin) {
                            None => continue,
                            Some(found) => {
                                scalar = found;
                                region.remove(position);
                            }
                        }
                    }
                    if region.is_empty() {
                        for (value, row) in folded.iter_mut().zip(&rows) {
                            *value += row[column] * scalar;
                        }
                    } else {
                        if !columns.iter().any(|(found, _)| *found == region) {
                            columns.push((region.clone(), vec![0.0; items]));
                        }
                        let values = &mut columns.iter_mut().find(|(found, _)| *found == region).unwrap().1;
                        for (value, row) in values.iter_mut().zip(&rows) {
                            *value += row[column] * scalar;
                        }
                    }
                }

                for (inner, value) in folded.iter().enumerate() {
                    deltas.insert(((outer as u32) << 16) | inner as u32, *value);
                }

                let indexes: Vec<u16> = columns
                    .iter()
                    .map(|(region, _)| match regionlist.iter().position(|found| found == region) {
                        Some(found) => found as u16,
                        None => {
                            regionlist.push(region.clone());
                            (regionlist.len() - 1) as u16
                        }
                    })
                    .collect();
                let mut encoded = Vec::new();
                for item in 0..items {
                    for (_, values) in &columns {
                        encoded.extend((otround(values[item]) as i16).to_be_bytes());
                    }
                }
                rebuilt.push(Some(ItemVariationData::new(items as u16, columns.len() as u16, indexes, encoded)));
            }

            let list = VariationRegionList::new(
                remaining.len() as u16,
                regionlist
                    .iter()
                    .map(|region| {
                        VariationRegion::new(
                            remaining
                                .iter()
                                .map(|tag| match region.iter().find(|(found, _)| found == tag) {
                                    Some((_, (lower, peak, upper))) => RegionAxisCoordinates::new(
                                        F2Dot14::from_f32(*lower as f32),
                                        F2Dot14::from_f32(*peak as f32),
                                        F2Dot14::from_f32(*upper as f32),
                                    ),
                                    None => RegionAxisCoordinates::new(F2Dot14::from_f32(0.0), F2Dot14::from_f32(0.0), F2Dot14::from_f32(0.0)),
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            );
            owned.item_var_store = Some(ItemVariationStore::new(list, rebuilt)).into();
        }

        let fold = Fold { deltas, factor: 1.0 };
        fold.carets(&mut owned);
        font.put(tags::GDEF, &owned);

        if let Some(gpos) = font.read::<read_fonts::tables::gpos::Gpos>() {
            let mut owned: Gpos = gpos.to_owned_table();
            fold.gpos(&mut owned);
            font.put(tags::GPOS, &owned);
        }
    }

    pub fn declare(&self, font: &mut Font) {
        let Some(fvar) = font.read::<read_fonts::tables::fvar::Fvar>() else {
            return;
        };
        let axes: Vec<Tag> = fvar.axes().expect("failed to parse fvar axes").iter().map(|entry| entry.axis_tag()).collect();
        let mappings = Space::mappings(font);

        let mut owned: write_fonts::tables::fvar::Fvar = fvar.to_owned_table();
        {
            let arrays = &mut owned.axis_instance_arrays;
            arrays.axes.retain(|axis| axis.axis_tag != Axis::tag());
            arrays.instances.clear();
        }
        font.put(tags::FVAR, &owned);

        if font.contains(tags::AVAR) {
            let remaining: Vec<Vec<(f64, f64)>> = axes
                .iter()
                .enumerate()
                .filter(|(_, tag)| **tag != Axis::tag())
                .map(|(index, _)| mappings.get(index).cloned().unwrap_or_default())
                .collect();
            if remaining.is_empty() {
                font.remove(tags::AVAR);
            } else {
                let table = Avar::new(
                    remaining
                        .iter()
                        .map(|pairs| {
                            SegmentMaps::new(
                                pairs
                                    .iter()
                                    .map(|(plain, mapped)| AxisValueMap::new(F2Dot14::from_f32(*plain as f32), F2Dot14::from_f32(*mapped as f32)))
                                    .collect(),
                            )
                        })
                        .collect(),
                );
                font.put(tags::AVAR, &table);
            }
        }

        for tag in [tags::HVAR, tags::VVAR, tags::MVAR] {
            font.remove(tag);
        }
    }
}

pub fn scalar(value: f64, tent: (f64, f64, f64)) -> f64 {
    let (lower, peak, upper) = tent;
    if peak == 0.0 {
        return 1.0;
    }
    if lower > peak || peak > upper {
        return 1.0;
    }
    if lower < 0.0 && upper > 0.0 {
        return 1.0;
    }
    if value == peak {
        return 1.0;
    }
    if value <= lower || upper <= value {
        return 0.0;
    }
    if value < peak {
        return (value - lower) / (peak - lower);
    }
    (value - upper) / (peak - upper)
}

pub struct Limits {
    pub minimum: f64,
    pub default: f64,
    pub maximum: f64,
    pub negative: f64,
    pub positive: f64,
}

impl Limits {
    pub fn reversed(&self) -> Limits {
        Limits {
            minimum: -self.maximum,
            default: -self.default,
            maximum: -self.minimum,
            negative: self.positive,
            positive: self.negative,
        }
    }

    pub fn renormalize(&self, value: f64) -> f64 {
        if value == self.default {
            return 0.0;
        }
        if self.default < 0.0 {
            return -self.reversed().renormalize(-value);
        }
        if value > self.default {
            return (value - self.default) / (self.maximum - self.default);
        }
        if self.minimum >= 0.0 {
            return (value - self.default) / (self.default - self.minimum);
        }
        let total = self.negative * -self.minimum + self.positive * self.default;
        let distance = if value >= 0.0 {
            (self.default - value) * self.positive
        } else {
            -value * self.negative + self.positive * self.default
        };
        -distance / total
    }

    pub fn solve(&self, tent: (f64, f64, f64)) -> Vec<(f64, Option<(f64, f64, f64)>)> {
        let (lower, peak, upper) = tent;

        if self.default > peak {
            return self
                .reversed()
                .solve((-upper, -peak, -lower))
                .into_iter()
                .map(|(found, piece)| (found, piece.map(|(low, middle, high)| (-high, -middle, -low))))
                .collect();
        }

        if self.maximum <= lower && self.maximum < peak {
            return Vec::new();
        }

        if self.maximum < peak {
            let multiplier = scalar(self.maximum, tent);
            return self
                .solve((lower, self.maximum, self.maximum))
                .into_iter()
                .map(|(found, piece)| (found * multiplier, piece))
                .collect();
        }

        let gain = scalar(self.default, tent);
        let mut out = vec![(gain, None)];
        let outer = scalar(self.maximum, tent);

        if gain >= outer {
            let crossing = peak + (1.0 - gain) * (upper - peak);
            out.push((1.0 - gain, Some((lower.max(self.default), peak, crossing))));
            if upper >= self.maximum {
                out.push((outer - gain, Some((crossing, self.maximum, self.maximum))));
            } else {
                let upper = if upper == self.default { upper + 1.0 / 16384.0 } else { upper };
                out.push((0.0 - gain, Some((crossing, upper, self.maximum))));
                out.push((0.0 - gain, Some((upper, self.maximum, self.maximum))));
            }
        } else {
            out.push((1.0 - gain, Some((self.default.max(lower), peak, self.maximum))));
            if peak < self.maximum {
                out.push((outer - gain, Some((peak, self.maximum, self.maximum))));
            }
        }

        if lower <= self.minimum {
            out.push((scalar(self.minimum, tent) - gain, Some((self.minimum, self.minimum, self.default))));
        } else {
            let lower = if lower == self.default { lower - 1.0 / 16384.0 } else { lower };
            out.push((0.0 - gain, Some((self.minimum, lower, self.default))));
            out.push((0.0 - gain, Some((self.minimum, self.minimum, lower))));
        }

        out
    }

    pub fn rebase(&self, tent: (f64, f64, f64)) -> Vec<(f64, Option<(f64, f64, f64)>)> {
        self.solve(tent)
            .into_iter()
            .filter(|(found, _)| *found != 0.0)
            .map(|(found, piece)| {
                (found, piece.map(|(low, middle, high)| (self.renormalize(low), self.renormalize(middle), self.renormalize(high))))
            })
            .collect()
    }

    pub fn limit(&self, tent: (f64, f64, f64)) -> Vec<(f64, Option<(f64, f64, f64)>)> {
        let (lower, peak, upper) = tent;
        if peak == 0.0 {
            return vec![(1.0, None)];
        }
        if !(lower <= peak && peak <= upper) || (lower < 0.0 && upper > 0.0) {
            return Vec::new();
        }
        self.rebase(tent)
    }
}

pub struct Rebase {
    pub minimum: f64,
    pub default: f64,
    pub maximum: f64,
    pub factor: f64,
}

impl Rebase {
    pub fn new(minimum: f64, default: f64, maximum: f64, factor: f64) -> Rebase {
        Rebase { minimum, default, maximum, factor }
    }

    pub fn clamped(&self, font: &Font) -> (f64, f64, f64) {
        let fvar = font.read::<read_fonts::tables::fvar::Fvar>().expect("missing fvar");
        for entry in fvar.axes().expect("failed to parse fvar axes") {
            if entry.axis_tag() != Axis::tag() {
                continue;
            }
            let (low, high) = (entry.min_value().to_f64(), entry.max_value().to_f64());
            let minimum = self.minimum.max(low).min(high);
            let maximum = self.maximum.max(low).min(high);
            let default = self.default.min(maximum).max(minimum);
            return (minimum, default, maximum);
        }
        (self.minimum, self.default, self.maximum)
    }

    pub fn limits(&self, font: &Font, mapped: bool) -> Limits {
        let fvar = font.read::<read_fonts::tables::fvar::Fvar>().expect("missing fvar");
        let mappings = Space::mappings(font);
        let (minimum, default, maximum) = self.clamped(font);

        for (index, entry) in fvar.axes().expect("failed to parse fvar axes").iter().enumerate() {
            if entry.axis_tag() != Axis::tag() {
                continue;
            }
            let axis = Axis::new(entry.min_value().to_f64(), entry.default_value().to_f64(), entry.max_value().to_f64());
            let pairs = if mapped { mappings.get(index).cloned().unwrap_or_default() } else { Vec::new() };
            let normalized = |value: f64| {
                let coordinate = axis.normalize(value);
                if pairs.is_empty() {
                    quantize(coordinate)
                } else {
                    quantize(piecewise(coordinate, &pairs))
                }
            };
            return Limits {
                minimum: normalized(minimum),
                default: normalized(default),
                maximum: normalized(maximum),
                negative: axis.default - axis.minimum,
                positive: axis.maximum - axis.default,
            };
        }

        Limits { minimum: -1.0, default: 0.0, maximum: 1.0, negative: 1.0, positive: 1.0 }
    }

    pub fn apply(&self, font: &mut Font) {
        let axes = Pin::axes(font);
        assert!(
            axes.len() == 1 && axes[0] == Axis::tag(),
            "Rebase requires a single-axis wght font; found {:?}",
            axes
        );
        let limits = self.limits(font, true);

        if font.contains(tags::GLYF) {
            self.outlines(font, &limits);
        }
        self.store(font, &limits);
        self.features(font, &limits);
        self.declare(font);
    }

    pub fn outlines(&self, font: &mut Font, limits: &Limits) {
        let data = font.data();
        let reference = FontRef::new(&data).expect("failed to parse font");
        let glyf = reference.glyf().expect("missing glyf");
        let loca = reference.loca(None).expect("missing loca");
        let gvar = reference.gvar().ok();

        let horizontal = font.metrics(tags::HHEA, tags::HMTX);
        let vertical = if font.contains(tags::VMTX) { Some(font.metrics(tags::VHEA, tags::VMTX)) } else { None };
        let count = font.glyph_count();

        let mut instanced: Vec<Instanced> = Vec::with_capacity(count);
        for index in 0..count {
            let identifier = GlyphId::new(index as u32);
            let glyph = loca.get_glyf(identifier, &glyf).expect("failed to parse glyph");
            let points = Points::of(glyph.as_ref(), &horizontal[index], vertical.as_ref().map(|found| &found[index]));
            let total = points.coordinates.len();

            let mut components = Vec::new();
            if let Some(read_fonts::tables::glyf::Glyph::Composite(found)) = glyph.as_ref() {
                for component in found.components() {
                    let transform = component.transform;
                    components.push((
                        component.glyph.to_u32() as usize,
                        [transform.xx.to_f32() as f64, transform.xy.to_f32() as f64, transform.yx.to_f32() as f64, transform.yy.to_f32() as f64],
                        matches!(component.anchor, read_fonts::tables::glyf::Anchor::Offset { .. }),
                    ));
                }
            }

            let mut coordinates = points.coordinates.clone();
            let mut merged: Vec<(Region, Vec<Vec2>)> = Vec::new();

            if let Some(Ok(Some(variations))) = gvar.as_ref().map(|found| found.glyph_variation_data(identifier)) {
                for tuple in variations.tuples() {
                    let peak = tuple.peak().values().first().map(|value| value.get().to_f32() as f64).unwrap_or(0.0);
                    let (start, end) = match (tuple.intermediate_start(), tuple.intermediate_end()) {
                        (Some(start), Some(end)) => (
                            start.values().first().map(|value| value.get().to_f32() as f64).unwrap_or(0.0),
                            end.values().first().map(|value| value.get().to_f32() as f64).unwrap_or(0.0),
                        ),
                        _ => (peak.min(0.0), peak.max(0.0)),
                    };
                    let tent = (start, peak, end);
                    let pieces = if tent == (0.0, 0.0, 0.0) { vec![(1.0, None)] } else { limits.limit(tent) };
                    if pieces.is_empty() {
                        continue;
                    }

                    let mut sparse: Vec<Option<Vec2>> = vec![None; total];
                    if tuple.has_deltas_for_all_points() {
                        for (position, delta) in tuple.deltas().enumerate() {
                            if position < total {
                                sparse[position] = Some(Vec2::new(delta.x_delta as f64, delta.y_delta as f64));
                            }
                        }
                    } else {
                        for delta in tuple.deltas() {
                            let position = delta.position as usize;
                            if position < total {
                                sparse[position] = Some(Vec2::new(delta.x_delta as f64, delta.y_delta as f64));
                            }
                        }
                    }

                    let dense = iup_delta(&sparse, &points.coordinates, &points.ends);

                    for (multiplier, piece) in pieces {
                        let region: Region = piece.map(|tent| vec![(Axis::tag(), tent)]).unwrap_or_default();
                        let scaled: Vec<Vec2> = dense.iter().map(|delta| *delta * multiplier).collect();
                        match merged.iter_mut().find(|(found, _)| *found == region) {
                            Some((_, deltas)) => {
                                for (value, delta) in deltas.iter_mut().zip(&scaled) {
                                    *value += *delta;
                                }
                            }
                            None => merged.push((region, scaled)),
                        }
                    }
                }
            }

            let mut tuples = Vec::new();
            for (region, deltas) in merged {
                if region.is_empty() {
                    for (value, delta) in coordinates.iter_mut().zip(&deltas) {
                        *value += *delta;
                    }
                } else {
                    tuples.push((region, deltas));
                }
            }

            instanced.push(Instanced { coordinates, ends: points.ends, tuples, components });
        }

        let mut cache: Vec<Option<Vec<Point>>> = vec![None; count];
        let boxes: Vec<(i32, i32, i32, i32)> = (0..count).map(|index| Instanced::bounds(index, &instanced, &mut cache)).collect();

        let scaled: Vec<Instanced> = instanced
            .iter()
            .map(|entry| Instanced {
                coordinates: entry
                    .coordinates
                    .iter()
                    .map(|point| Point::new(otround(self.factor * point.x) as f64, otround(self.factor * point.y) as f64))
                    .collect(),
                ends: entry.ends.clone(),
                tuples: Vec::new(),
                components: entry.components.clone(),
            })
            .collect();
        let mut rescaled: Vec<Option<Vec<Point>>> = vec![None; count];
        let finals: Vec<(i32, i32, i32, i32)> = (0..count).map(|index| Instanced::bounds(index, &scaled, &mut rescaled)).collect();

        let mut glyphs = Vec::with_capacity(count);
        let mut widths = Vec::with_capacity(count);
        let mut heights = Vec::with_capacity(count);
        for index in 0..count {
            let entry = &instanced[index];
            let total = entry.coordinates.len();
            let (left, right) = (entry.coordinates[total - 4].x, entry.coordinates[total - 3].x);
            let (top, bottom) = (entry.coordinates[total - 2].y, entry.coordinates[total - 1].y);
            let (minimum_x, _, _, maximum_y) = boxes[index];

            widths.push(Metric {
                advance: otround(self.factor * otround(right - left).max(0) as f64).max(0) as u16,
                bearing: otround(self.factor * otround(minimum_x as f64 - left) as f64) as i16,
            });
            heights.push(Metric {
                advance: otround(self.factor * otround(top - bottom).max(0) as f64).max(0) as u16,
                bearing: otround(self.factor * otround(top - maximum_y as f64) as f64) as i16,
            });

            let identifier = GlyphId::new(index as u32);
            let Some(parsed) = loca.get_glyf(identifier, &glyf).expect("failed to parse glyph") else {
                glyphs.push(Vec::new());
                continue;
            };
            let bbox = Bbox {
                x_min: finals[index].0 as i16,
                y_min: finals[index].1 as i16,
                x_max: finals[index].2 as i16,
                y_max: finals[index].3 as i16,
            };
            let mut glyph = Glyph::from_table_ref(&parsed);
            match &mut glyph {
                Glyph::Simple(simple) => {
                    let mut position = 0;
                    for contour in simple.contours.iter_mut() {
                        let moved: Vec<read_fonts::tables::glyf::CurvePoint> = contour
                            .iter()
                            .map(|point| {
                                let value = read_fonts::tables::glyf::CurvePoint::new(
                                    scaled[index].coordinates[position].x as i16,
                                    scaled[index].coordinates[position].y as i16,
                                    point.on_curve,
                                );
                                position += 1;
                                value
                            })
                            .collect();
                        *contour = moved.into();
                    }
                    simple.bbox = bbox;
                }
                Glyph::Composite(composite) => {
                    for (position, component) in composite.components_mut().iter_mut().enumerate() {
                        if let Anchor::Offset { x, y } = &mut component.anchor {
                            *x = scaled[index].coordinates[position].x as i16;
                            *y = scaled[index].coordinates[position].y as i16;
                        }
                    }
                    composite.bbox = bbox;
                }
                Glyph::Empty => {}
            }
            glyphs.push(if matches!(glyph, Glyph::Empty) { Vec::new() } else { write_fonts::dump_table(&glyph).expect("failed to serialize glyph") });
        }

        let mut rebuilt = Vec::with_capacity(count);
        for (index, entry) in instanced.iter().enumerate() {
            let identifier = GlyphId::new(index as u32);
            let mut tuples = Vec::new();
            for (region, deltas) in &entry.tuples {
                let finals: Vec<Vec2> = deltas
                    .iter()
                    .map(|delta| {
                        Vec2::new(
                            otround(self.factor * otround(delta.x) as f64) as f64,
                            otround(self.factor * otround(delta.y) as f64) as f64,
                        )
                    })
                    .collect();
                if finals.iter().all(|delta| delta.x == 0.0 && delta.y == 0.0) {
                    continue;
                }

                let flattened: Vec<GlyphDelta> = finals
                    .iter()
                    .map(|value| GlyphDelta::required(value.x as i16, value.y as i16))
                    .collect();

                let (lower, peak, upper) = (quantize(region[0].1 .0), quantize(region[0].1 .1), quantize(region[0].1 .2));
                let tent = if lower == peak.min(0.0) && upper == peak.max(0.0) {
                    Tent::new(F2Dot14::from_f32(peak as f32), None)
                } else {
                    Tent::new(F2Dot14::from_f32(peak as f32), Some((F2Dot14::from_f32(lower as f32), F2Dot14::from_f32(upper as f32))))
                };
                tuples.push(GlyphDeltas::new(vec![tent], flattened));
            }
            rebuilt.push(GlyphVariations::new(identifier, tuples));
        }

        font.set_glyphs(&glyphs);
        font.set_metrics(tags::HHEA, tags::HMTX, &widths);
        if vertical.is_some() {
            font.set_metrics(tags::VHEA, tags::VMTX, &heights);
        }
        if gvar.is_some() {
            let table = Gvar::new(rebuilt, 1).expect("failed to build gvar");
            font.put(tags::GVAR, &table);
        }
    }

    pub fn store(&self, font: &mut Font, limits: &Limits) {
        let Some(gdef) = font.read::<read_fonts::tables::gdef::Gdef>() else {
            return;
        };
        let mut owned: Gdef = gdef.to_owned_table();
        let mut deltas: HashMap<u32, f64> = HashMap::new();

        if let Some(Ok(store)) = gdef.item_var_store() {
            let supports: Vec<Option<(f64, f64, f64)>> = Mapping::regions(store.offset_data().as_bytes())
                .iter()
                .map(|region| {
                    let triple = region.first().copied().unwrap_or((0.0, 0.0, 0.0));
                    if triple.1 != 0.0 { Some(triple) } else { None }
                })
                .collect();

            let mut regionlist: Vec<(f64, f64, f64)> = Vec::new();
            let mut rebuilt: Vec<Option<ItemVariationData>> = Vec::new();

            for (outer, data) in store.item_variation_data().iter().enumerate() {
                let Some(Ok(data)) = data else {
                    rebuilt.push(None);
                    continue;
                };
                let items = data.item_count() as usize;
                let rows: Vec<Vec<f64>> = (0..items)
                    .map(|inner| data.delta_set(inner as u16).map(|value| value as f64).collect())
                    .collect();

                let mut columns: Vec<((f64, f64, f64), Vec<f64>)> = Vec::new();
                let mut folded = vec![0.0; items];
                for (column, entry) in data.region_indexes().iter().enumerate() {
                    let pieces = match supports[entry.get() as usize] {
                        None => vec![(1.0, None)],
                        Some(tent) => limits.limit(tent),
                    };
                    for (multiplier, piece) in pieces {
                        match piece {
                            None => {
                                for (value, row) in folded.iter_mut().zip(&rows) {
                                    *value += row[column] * multiplier;
                                }
                            }
                            Some(tent) => {
                                if !columns.iter().any(|(found, _)| *found == tent) {
                                    columns.push((tent, vec![0.0; items]));
                                }
                                let values = &mut columns.iter_mut().find(|(found, _)| *found == tent).unwrap().1;
                                for (value, row) in values.iter_mut().zip(&rows) {
                                    *value += row[column] * multiplier;
                                }
                            }
                        }
                    }
                }

                for (inner, value) in folded.iter().enumerate() {
                    deltas.insert(((outer as u32) << 16) | inner as u32, *value);
                }

                let indexes: Vec<u16> = columns
                    .iter()
                    .map(|(tent, _)| match regionlist.iter().position(|found| found == tent) {
                        Some(found) => found as u16,
                        None => {
                            regionlist.push(*tent);
                            (regionlist.len() - 1) as u16
                        }
                    })
                    .collect();
                let mut encoded = Vec::new();
                for item in 0..items {
                    for (_, values) in &columns {
                        encoded.extend((otround(self.factor * otround(values[item]) as f64) as i16).to_be_bytes());
                    }
                }
                rebuilt.push(Some(ItemVariationData::new(items as u16, columns.len() as u16, indexes, encoded)));
            }

            let list = VariationRegionList::new(
                1,
                regionlist
                    .iter()
                    .map(|(lower, peak, upper)| {
                        VariationRegion::new(vec![RegionAxisCoordinates::new(
                            F2Dot14::from_f32(quantize(*lower) as f32),
                            F2Dot14::from_f32(quantize(*peak) as f32),
                            F2Dot14::from_f32(quantize(*upper) as f32),
                        )])
                    })
                    .collect(),
            );
            owned.item_var_store = Some(ItemVariationStore::new(list, rebuilt)).into();
        }

        let fold = Fold { deltas, factor: self.factor };
        fold.carets(&mut owned);
        font.put(tags::GDEF, &owned);

        if let Some(gpos) = font.read::<read_fonts::tables::gpos::Gpos>() {
            let mut owned: Gpos = gpos.to_owned_table();
            fold.gpos(&mut owned);
            font.put(tags::GPOS, &owned);
        }
    }

    pub fn features(&self, font: &mut Font, limits: &Limits) {
        if let Some(gsub) = font.read::<read_fonts::tables::gsub::Gsub>() {
            let mut owned: Gsub = gsub.to_owned_table();
            if owned.feature_variations.is_some() {
                let current = (*owned.feature_variations).clone().map(|found| *found);
                owned.feature_variations = self.variations(limits, current, &mut owned.feature_list).into();
                font.put(tags::GSUB, &owned);
            }
        }
        if let Some(gpos) = font.read::<read_fonts::tables::gpos::Gpos>() {
            let mut owned: Gpos = gpos.to_owned_table();
            if owned.feature_variations.is_some() {
                let current = (*owned.feature_variations).clone().map(|found| *found);
                owned.feature_variations = self.variations(limits, current, &mut owned.feature_list).into();
                font.put(tags::GPOS, &owned);
            }
        }
    }

    pub fn variations(&self, limits: &Limits, current: Option<FeatureVariations>, features: &mut FeatureList) -> Option<FeatureVariations> {
        let table = current?;

        let mut kept: Vec<FeatureVariationRecord> = Vec::new();
        let mut seen: std::collections::HashSet<Vec<(u16, i32, i32)>> = std::collections::HashSet::new();
        let mut defaults: Option<FeatureTableSubstitution> = None;
        let mut applied = false;
        let mut universal = false;

        for record in table.feature_variation_records {
            let mut record = record;
            let mut applies = true;
            let mut keep = false;
            let mut dropped = false;
            let mut conditions: Vec<Condition> = Vec::new();

            let existing: Vec<Condition> = record
                .condition_set
                .as_ref()
                .map(|set| set.conditions.iter().map(|condition| (**condition).clone()).collect())
                .unwrap_or_default();
            for condition in existing {
                match condition {
                    Condition::Format1AxisRange(found) => {
                        let minimum = found.filter_range_min_value.to_f32() as f64;
                        let maximum = found.filter_range_max_value.to_f32() as f64;
                        if !(minimum <= limits.default && limits.default <= maximum) {
                            applies = false;
                        }
                        if limits.minimum > maximum || limits.maximum < minimum {
                            dropped = true;
                            break;
                        }
                        if minimum > maximum || minimum > limits.maximum || maximum < limits.minimum {
                            dropped = true;
                            break;
                        }
                        let low = limits.renormalize(minimum.clamp(limits.minimum, limits.maximum));
                        let high = limits.renormalize(maximum.clamp(limits.minimum, limits.maximum));
                        keep = true;
                        if low != -1.0 || high != 1.0 {
                            conditions.push(Condition::Format1AxisRange(ConditionFormat1::new(
                                found.axis_index,
                                F2Dot14::from_f32(quantize(low) as f32),
                                F2Dot14::from_f32(quantize(high) as f32),
                            )));
                        }
                    }
                    other => {
                        applies = false;
                        conditions.push(other);
                    }
                }
            }

            let retained = !dropped && keep;
            universal = retained && conditions.is_empty();
            if retained {
                let unique = {
                    let mut key: Vec<(u16, i32, i32)> = Vec::new();
                    let mut comparable = true;
                    for condition in &conditions {
                        match condition {
                            Condition::Format1AxisRange(found) => key.push((
                                found.axis_index,
                                found.filter_range_min_value.to_bits() as i32,
                                found.filter_range_max_value.to_bits() as i32,
                            )),
                            _ => {
                                comparable = false;
                                break;
                            }
                        }
                    }
                    key.sort_unstable();
                    key.dedup();
                    !comparable || seen.insert(key)
                };
                record.condition_set = if conditions.is_empty() { None } else { Some(ConditionSet::new(conditions)) }.into();
                if unique {
                    kept.push(record.clone());
                }
            }

            if applies && !applied {
                let substitution = record.feature_table_substitution.as_ref().expect("record has no substitution").clone();
                let mut restored = substitution.clone();
                for (default, entry) in restored.substitutions.iter_mut().zip(&substitution.substitutions) {
                    let index = entry.feature_index as usize;
                    default.alternate_feature.set((*features.feature_records[index].feature).clone());
                    features.feature_records[index].feature.set((*entry.alternate_feature).clone());
                }
                defaults = Some(restored);
                applied = true;
            }

            if universal {
                break;
            }
        }

        if applied && !kept.is_empty() && !universal {
            kept.push(FeatureVariationRecord {
                condition_set: Some(ConditionSet::new(Vec::new())).into(),
                feature_table_substitution: defaults.into(),
            });
        }

        if kept.is_empty() {
            None
        } else {
            Some(FeatureVariations::new(kept))
        }
    }

    pub fn declare(&self, font: &mut Font) {
        let Some(fvar) = font.read::<read_fonts::tables::fvar::Fvar>() else {
            return;
        };
        let (minimum, default, maximum) = self.clamped(font);
        let mappings = Space::mappings(font);
        let range = self.limits(font, false);

        let mut owned: write_fonts::tables::fvar::Fvar = fvar.to_owned_table();
        {
            let arrays = &mut owned.axis_instance_arrays;
            for entry in arrays.axes.iter_mut() {
                if entry.axis_tag == Axis::tag() {
                    entry.min_value = write_fonts::types::Fixed::from_f64(minimum);
                    entry.default_value = write_fonts::types::Fixed::from_f64(default);
                    entry.max_value = write_fonts::types::Fixed::from_f64(maximum);
                }
            }
            arrays.instances.clear();
        }
        font.put(tags::FVAR, &owned);

        if font.contains(tags::AVAR) {
            let pairs = mappings.first().cloned().unwrap_or_default();
            if !pairs.is_empty() {
                let mapped = Limits {
                    minimum: quantize(piecewise(range.minimum, &pairs)),
                    default: quantize(piecewise(range.default, &pairs)),
                    maximum: quantize(piecewise(range.maximum, &pairs)),
                    negative: range.negative,
                    positive: range.positive,
                };
                let mut segments: std::collections::BTreeMap<i32, f64> = std::collections::BTreeMap::new();
                for (from, to) in &pairs {
                    if *from < range.minimum || *from > range.maximum {
                        continue;
                    }
                    let from = quantize(range.renormalize(*from));
                    let to = quantize(mapped.renormalize(*to));
                    segments.insert(otround(from * 16384.0), to);
                }
                for anchor in [-1.0f64, 0.0, 1.0] {
                    segments.insert(otround(anchor * 16384.0), anchor);
                }
                let table = Avar::new(vec![SegmentMaps::new(
                    segments
                        .iter()
                        .map(|(from, to)| AxisValueMap::new(F2Dot14::from_f32(*from as f32 / 16384.0), F2Dot14::from_f32(*to as f32)))
                        .collect(),
                )]);
                font.put(tags::AVAR, &table);
            }
        }

        if self.factor != 1.0 {
            let mut head: write_fonts::tables::head::Head = font
                .read::<read_fonts::tables::head::Head>()
                .expect("missing head")
                .to_owned_table();
            head.units_per_em = otround(head.units_per_em as f64 * self.factor) as u16;
            font.put(tags::HEAD, &head);
        }

        for tag in [tags::HVAR, tags::VVAR, tags::MVAR] {
            font.remove(tag);
        }
    }
}

pub struct Instanced {
    pub coordinates: Vec<Point>,
    pub ends: Vec<usize>,
    pub tuples: Vec<(Region, Vec<Vec2>)>,
    pub components: Vec<(usize, [f64; 4], bool)>,
}

impl Instanced {
    pub fn flatten(index: usize, glyphs: &[Instanced], cache: &mut Vec<Option<Vec<Point>>>) -> Vec<Point> {
        if let Some(found) = &cache[index] {
            return found.clone();
        }

        let glyph = &glyphs[index];
        let mut flattened = Vec::new();
        if glyph.components.is_empty() {
            let total = glyph.coordinates.len();
            for point in &glyph.coordinates[..total - 4] {
                flattened.push(Point::new(otround(point.x) as f64, otround(point.y) as f64));
            }
        } else {
            for (position, (base, transform, offset)) in glyph.components.iter().enumerate() {
                let points = Instanced::flatten(*base, glyphs, cache);
                let (x, y) = if *offset {
                    (glyph.coordinates[position].x, glyph.coordinates[position].y)
                } else {
                    (0.0, 0.0)
                };
                for point in points {
                    flattened.push(Point::new(
                        transform[0] * point.x + transform[2] * point.y + x,
                        transform[1] * point.x + transform[3] * point.y + y,
                    ));
                }
            }
        }

        cache[index] = Some(flattened.clone());
        flattened
    }

    pub fn bounds(index: usize, glyphs: &[Instanced], cache: &mut Vec<Option<Vec<Point>>>) -> (i32, i32, i32, i32) {
        let flattened = Instanced::flatten(index, glyphs, cache);
        if flattened.is_empty() {
            return (0, 0, 0, 0);
        }
        let mut found = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for point in &flattened {
            let (x, y) = (otround(point.x), otround(point.y));
            found.0 = found.0.min(x);
            found.1 = found.1.min(y);
            found.2 = found.2.max(x);
            found.3 = found.3.max(y);
        }
        found
    }
}

pub struct Fold {
    pub deltas: HashMap<u32, f64>,
    pub factor: f64,
}

impl Fold {
    pub fn delta(&self, device: Option<&DeviceOrVariationIndex>) -> i32 {
        match device {
            Some(DeviceOrVariationIndex::VariationIndex(found)) => {
                let key = ((found.delta_set_outer_index as u32) << 16) | found.delta_set_inner_index as u32;
                otround(self.deltas.get(&key).copied().unwrap_or(0.0))
            }
            _ => 0,
        }
    }

    pub fn merge(&self, value: i16, delta: i32) -> i16 {
        otround(self.factor * (value as i32 + delta) as f64) as i16
    }

    pub fn value(&self, record: &mut ValueRecord) {
        let delta = self.delta(record.x_placement_device.as_ref());
        if let Some(value) = record.x_placement.as_mut() {
            *value = self.merge(*value, delta);
        }
        let delta = self.delta(record.y_placement_device.as_ref());
        if let Some(value) = record.y_placement.as_mut() {
            *value = self.merge(*value, delta);
        }
        let delta = self.delta(record.x_advance_device.as_ref());
        if let Some(value) = record.x_advance.as_mut() {
            *value = self.merge(*value, delta);
        }
        let delta = self.delta(record.y_advance_device.as_ref());
        if let Some(value) = record.y_advance.as_mut() {
            *value = self.merge(*value, delta);
        }
    }

    pub fn anchor(&self, anchor: &mut AnchorTable) {
        match anchor {
            AnchorTable::Format1(found) => {
                found.x_coordinate = self.merge(found.x_coordinate, 0);
                found.y_coordinate = self.merge(found.y_coordinate, 0);
            }
            AnchorTable::Format2(found) => {
                found.x_coordinate = self.merge(found.x_coordinate, 0);
                found.y_coordinate = self.merge(found.y_coordinate, 0);
            }
            AnchorTable::Format3(found) => {
                let x = self.delta(found.x_device.as_ref());
                let y = self.delta(found.y_device.as_ref());
                found.x_coordinate = self.merge(found.x_coordinate, x);
                found.y_coordinate = self.merge(found.y_coordinate, y);
            }
        }
    }

    pub fn single(&self, table: &mut SinglePos) {
        match table {
            SinglePos::Format1(found) => self.value(&mut found.value_record),
            SinglePos::Format2(found) => {
                for record in found.value_records.iter_mut() {
                    self.value(record);
                }
            }
        }
    }

    pub fn pair(&self, table: &mut PairPos) {
        match table {
            PairPos::Format1(found) => {
                for set in found.pair_sets.iter_mut() {
                    for record in set.pair_value_records.iter_mut() {
                        self.value(&mut record.value_record1);
                        self.value(&mut record.value_record2);
                    }
                }
            }
            PairPos::Format2(found) => {
                for class1 in found.class1_records.iter_mut() {
                    for class2 in class1.class2_records.iter_mut() {
                        self.value(&mut class2.value_record1);
                        self.value(&mut class2.value_record2);
                    }
                }
            }
        }
    }

    pub fn cursive(&self, table: &mut CursivePosFormat1) {
        for record in table.entry_exit_record.iter_mut() {
            if let Some(found) = record.entry_anchor.as_mut() {
                self.anchor(found);
            }
            if let Some(found) = record.exit_anchor.as_mut() {
                self.anchor(found);
            }
        }
    }

    pub fn base(&self, table: &mut MarkBasePosFormat1) {
        for record in table.mark_array.mark_records.iter_mut() {
            self.anchor(&mut record.mark_anchor);
        }
        for record in table.base_array.base_records.iter_mut() {
            for anchor in record.base_anchors.iter_mut() {
                if let Some(found) = anchor.as_mut() {
                    self.anchor(found);
                }
            }
        }
    }

    pub fn ligature(&self, table: &mut MarkLigPosFormat1) {
        for record in table.mark_array.mark_records.iter_mut() {
            self.anchor(&mut record.mark_anchor);
        }
        for attach in table.ligature_array.ligature_attaches.iter_mut() {
            for record in attach.component_records.iter_mut() {
                for anchor in record.ligature_anchors.iter_mut() {
                    if let Some(found) = anchor.as_mut() {
                        self.anchor(found);
                    }
                }
            }
        }
    }

    pub fn marks(&self, table: &mut MarkMarkPosFormat1) {
        for record in table.mark1_array.mark_records.iter_mut() {
            self.anchor(&mut record.mark_anchor);
        }
        for record in table.mark2_array.mark2_records.iter_mut() {
            for anchor in record.mark2_anchors.iter_mut() {
                if let Some(found) = anchor.as_mut() {
                    self.anchor(found);
                }
            }
        }
    }

    pub fn lookup(&self, lookup: &mut PositionLookup) {
        match lookup {
            PositionLookup::Single(found) => {
                for table in found.subtables.iter_mut() {
                    self.single(table);
                }
            }
            PositionLookup::Pair(found) => {
                for table in found.subtables.iter_mut() {
                    self.pair(table);
                }
            }
            PositionLookup::Cursive(found) => {
                for table in found.subtables.iter_mut() {
                    self.cursive(table);
                }
            }
            PositionLookup::MarkToBase(found) => {
                for table in found.subtables.iter_mut() {
                    self.base(table);
                }
            }
            PositionLookup::MarkToLig(found) => {
                for table in found.subtables.iter_mut() {
                    self.ligature(table);
                }
            }
            PositionLookup::MarkToMark(found) => {
                for table in found.subtables.iter_mut() {
                    self.marks(table);
                }
            }
            PositionLookup::Contextual(_) | PositionLookup::ChainContextual(_) => {}
            PositionLookup::Extension(found) => {
                for table in found.subtables.iter_mut() {
                    match &mut **table {
                        ExtensionSubtable::Single(inner) => self.single(&mut inner.extension),
                        ExtensionSubtable::Pair(inner) => self.pair(&mut inner.extension),
                        ExtensionSubtable::Cursive(inner) => self.cursive(&mut inner.extension),
                        ExtensionSubtable::MarkToBase(inner) => self.base(&mut inner.extension),
                        ExtensionSubtable::MarkToLig(inner) => self.ligature(&mut inner.extension),
                        ExtensionSubtable::MarkToMark(inner) => self.marks(&mut inner.extension),
                        ExtensionSubtable::Contextual(_) | ExtensionSubtable::ChainContextual(_) => {}
                    }
                }
            }
        }
    }

    pub fn gpos(&self, table: &mut Gpos) {
        for lookup in table.lookup_list.lookups.iter_mut() {
            self.lookup(lookup);
        }
    }

    pub fn carets(&self, table: &mut Gdef) {
        if let Some(list) = table.lig_caret_list.as_mut() {
            for glyph in list.lig_glyphs.iter_mut() {
                for caret in glyph.caret_values.iter_mut() {
                    match &mut **caret {
                        CaretValue::Format1(found) => found.coordinate = self.merge(found.coordinate, 0),
                        CaretValue::Format2(_) => {}
                        CaretValue::Format3(found) => {
                            let delta = self.delta(Some(&found.device));
                            found.coordinate = self.merge(found.coordinate, delta);
                        }
                    }
                }
            }
        }
    }
}

pub fn hvar(font: &mut Font) {
    if !font.contains(tags::FVAR) || !font.contains(tags::GVAR) {
        return;
    }
    font.remove(tags::HVAR);

    let data = font.data();
    let reference = FontRef::new(&data).expect("failed to parse font");
    let fvar = reference.fvar().expect("missing fvar");
    let axis_count = fvar.axis_count() as usize;
    let glyf = reference.glyf().expect("missing glyf");
    let loca = reference.loca(None).expect("missing loca");
    let gvar = reference.gvar().expect("missing gvar");

    let horizontal = font.metrics(tags::HHEA, tags::HMTX);
    let vertical = if font.contains(tags::VMTX) { Some(font.metrics(tags::VHEA, tags::VMTX)) } else { None };
    let count = font.glyph_count();

    let mut builder = VariationStoreBuilder::new(axis_count as u16);
    let mut keys = Vec::with_capacity(count);
    for index in 0..count {
        let identifier = GlyphId::new(index as u32);
        let mut deltas: Vec<(VariationRegion, i32)> = Vec::new();
        if let Ok(Some(variations)) = gvar.glyph_variation_data(identifier) {
            let glyph = loca.get_glyf(identifier, &glyf).expect("failed to parse glyph");
            let points = Points::of(glyph.as_ref(), &horizontal[index], vertical.as_ref().map(|found| &found[index]));
            let total = points.coordinates.len();

            for tuple in variations.tuples() {
                let peaks: Vec<F2Dot14> = tuple.peak().values().iter().map(|value| value.get()).collect();
                let (starts, ends): (Vec<F2Dot14>, Vec<F2Dot14>) = match (tuple.intermediate_start(), tuple.intermediate_end()) {
                    (Some(start), Some(end)) => (
                        start.values().iter().map(|value| value.get()).collect(),
                        end.values().iter().map(|value| value.get()).collect(),
                    ),
                    _ => (
                        peaks.iter().map(|peak| if peak.to_f32() < 0.0 { *peak } else { F2Dot14::from_f32(0.0) }).collect(),
                        peaks.iter().map(|peak| if peak.to_f32() > 0.0 { *peak } else { F2Dot14::from_f32(0.0) }).collect(),
                    ),
                };
                let region = VariationRegion::new(
                    (0..axis_count)
                        .map(|position| RegionAxisCoordinates::new(starts[position], peaks[position], ends[position]))
                        .collect(),
                );

                let (mut left, mut right) = (0i32, 0i32);
                if tuple.has_deltas_for_all_points() {
                    for (position, delta) in tuple.deltas().enumerate() {
                        if position == total - 4 {
                            left = delta.x_delta;
                        }
                        if position == total - 3 {
                            right = delta.x_delta;
                        }
                    }
                } else {
                    for delta in tuple.deltas() {
                        let position = delta.position as usize;
                        if position == total - 4 {
                            left = delta.x_delta;
                        }
                        if position == total - 3 {
                            right = delta.x_delta;
                        }
                    }
                }
                deltas.push((region, right - left));
            }
        }
        keys.push(builder.add_deltas(deltas));
    }

    let (store, remapping) = builder.build();
    let mapping: Vec<u32> = keys
        .iter()
        .map(|key| {
            remapping
                .get(*key)
                .map(|found| ((found.delta_set_outer_index as u32) << 16) | found.delta_set_inner_index as u32)
                .unwrap_or(0xFFFFFFFF)
        })
        .collect();
    let table = Hvar::new(store, Some(mapping.into_iter().collect()), None, None);
    font.put(tags::HVAR, &table);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(non_upper_case_globals)]
    pub const scratchpad: &str = "/private/tmp/claude-501/-Volumes-Developments-nercone-dev-fonts/d08e5eec-1bbb-4368-8fb4-36df636f3bff/scratchpad/statics-test";

    pub fn pinned(value: f64, name: &str) {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/build/files/NerconeSansJP-Variable.ttf");
        let data = std::fs::read(path).expect("missing variable font");
        let mut font = Font::new(&data);

        Pin::new(value).apply(&mut font);
        hvar(&mut font);

        std::fs::create_dir_all(scratchpad).expect("failed to create scratchpad");
        std::fs::write(format!("{}/{}", scratchpad, name), font.data()).expect("failed to write font");
    }

    #[test]
    fn pin700() {
        pinned(700.0, "rust-700.ttf");
    }

    #[test]
    fn pin400() {
        pinned(400.0, "rust-400.ttf");
    }

    #[allow(non_upper_case_globals)]
    pub const inputs: &str = "/private/tmp/claude-501/-Volumes-Developments-nercone-dev-fonts/d08e5eec-1bbb-4368-8fb4-36df636f3bff/scratchpad/verify/out";

    pub fn rebased(source: &str, minimum: f64, default: f64, maximum: f64, factor: f64, name: &str) {
        let data = std::fs::read(format!("{}/{}", inputs, source)).expect("missing rebase input");
        let mut font = Font::new(&data);

        Rebase::new(minimum, default, maximum, factor).apply(&mut font);

        std::fs::create_dir_all(scratchpad).expect("failed to create scratchpad");
        std::fs::write(format!("{}/{}", scratchpad, name), font.data()).expect("failed to write font");
    }

    #[test]
    fn rebasesans() {
        rebased("sub-sansjp.harfbuzz.ttf", 100.0, 400.0, 900.0, 2048.0 / 1000.0, "rust-rebase-sans.ttf");
    }

    #[test]
    fn rebaseserif() {
        rebased("sub-serifjp.harfbuzz.ttf", 200.0, 400.0, 900.0, 1.0, "rust-rebase-serif.ttf");
    }
}
