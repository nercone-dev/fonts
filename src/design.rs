use std::collections::{HashMap, HashSet};

use fontdrasil::coords::NormalizedLocation;
use fontdrasil::variations::{VariationModel, VariationRegion as ModelRegion};
use kurbo::{Point, Vec2};
use read_fonts::tables::variations::ItemVariationStore;
use read_fonts::{FontData, FontRead, FontRef, TableProvider};
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::tables::avar::{Avar, AxisValueMap, SegmentMaps};
use write_fonts::tables::gvar::iup::iup_delta_optimize;
use write_fonts::tables::gvar::{GlyphDelta, GlyphDeltas, GlyphVariations, Gvar, Tent};
use write_fonts::tables::variations::{
    ItemVariationData, ItemVariationStore as ItemVariationStoreOwned, RegionAxisCoordinates,
    VariationRegion, VariationRegionList,
};
use write_fonts::types::{F2Dot14, GlyphId, Tag};

use crate::font::{tags, Font, Points};
use crate::models::Weight;

pub fn tagged(tag: Tag) -> fontdrasil::types::Tag {
    fontdrasil::types::Tag::new(&tag.to_be_bytes())
}

#[allow(non_upper_case_globals)]
pub const tolerance: f64 = 0.5;
#[allow(non_upper_case_globals)]
pub const epsilon: f64 = 1e-9;

pub fn interpolate(value: f64, pairs: &[(f64, f64)]) -> f64 {
    if value <= pairs[0].0 {
        return pairs[0].1;
    }
    if value >= pairs[pairs.len() - 1].0 {
        return pairs[pairs.len() - 1].1;
    }

    for window in pairs.windows(2) {
        let ((left, lower), (right, upper)) = (window[0], window[1]);
        if left <= value && value <= right {
            if right == left {
                return lower;
            }
            return lower + (upper - lower) * (value - left) / (right - left);
        }
    }

    pairs[pairs.len() - 1].1
}

pub fn support_scalar(location: &HashMap<Tag, f64>, support: &[(Tag, (f64, f64, f64))]) -> f64 {
    let mut scalar = 1.0;
    for (tag, (lower, peak, upper)) in support {
        if *peak == 0.0 {
            continue;
        }
        if lower > peak || peak > upper {
            continue;
        }
        if *lower < 0.0 && *upper > 0.0 {
            continue;
        }
        let value = location.get(tag).copied().unwrap_or(0.0);
        if value == *peak {
            continue;
        }
        if value <= *lower || *upper <= value {
            return 0.0;
        }
        if value < *peak {
            scalar *= (value - lower) / (peak - lower);
        } else {
            scalar *= (value - upper) / (peak - upper);
        }
    }
    scalar
}

pub fn iup_segment(coordinates: &[Point], rc1: Point, rd1: Vec2, rc2: Point, rd2: Vec2) -> Vec<Vec2> {
    let mut out = vec![Vec2::ZERO; coordinates.len()];
    for j in 0..2 {
        let (mut x1, mut x2, mut d1, mut d2) = match j {
            0 => (rc1.x, rc2.x, rd1.x, rd2.x),
            _ => (rc1.y, rc2.y, rd1.y, rd2.y),
        };

        if x1 == x2 {
            let value = if d1 == d2 { d1 } else { 0.0 };
            for entry in out.iter_mut() {
                match j {
                    0 => entry.x = value,
                    _ => entry.y = value,
                }
            }
            continue;
        }

        if x1 > x2 {
            std::mem::swap(&mut x1, &mut x2);
            std::mem::swap(&mut d1, &mut d2);
        }

        let scale = (d2 - d1) / (x2 - x1);
        for (entry, coordinate) in out.iter_mut().zip(coordinates) {
            let x = match j {
                0 => coordinate.x,
                _ => coordinate.y,
            };
            let value = if x <= x1 {
                d1
            } else if x >= x2 {
                d2
            } else {
                d1 + (x - x1) * scale
            };
            match j {
                0 => entry.x = value,
                _ => entry.y = value,
            }
        }
    }
    out
}

pub fn iup_contour(deltas: &[Option<Vec2>], coordinates: &[Point]) -> Vec<Vec2> {
    if deltas.iter().all(|delta| delta.is_some()) {
        return deltas.iter().map(|delta| delta.unwrap()).collect();
    }

    let n = deltas.len();
    let indices: Vec<usize> = (0..n).filter(|index| deltas[*index].is_some()).collect();
    if indices.is_empty() {
        return vec![Vec2::ZERO; n];
    }

    let mut out = Vec::with_capacity(n);
    let start = indices[0];
    if start != 0 {
        let (i1, i2, ri1, ri2) = (0, start, start, indices[indices.len() - 1]);
        out.extend(iup_segment(&coordinates[i1..i2], coordinates[ri1], deltas[ri1].unwrap(), coordinates[ri2], deltas[ri2].unwrap()));
    }
    out.push(deltas[start].unwrap());

    for pair in indices.windows(2) {
        let (ri1, ri2) = (pair[0], pair[1]);
        if ri2 - ri1 > 1 {
            out.extend(iup_segment(&coordinates[ri1 + 1..ri2], coordinates[ri1], deltas[ri1].unwrap(), coordinates[ri2], deltas[ri2].unwrap()));
        }
        out.push(deltas[ri2].unwrap());
    }

    let end = indices[indices.len() - 1];
    if end != n - 1 {
        let (i1, i2, ri1, ri2) = (end + 1, n, end, start);
        out.extend(iup_segment(&coordinates[i1..i2], coordinates[ri1], deltas[ri1].unwrap(), coordinates[ri2], deltas[ri2].unwrap()));
    }

    out
}

pub fn iup_delta(deltas: &[Option<Vec2>], coordinates: &[Point], ends: &[usize]) -> Vec<Vec2> {
    let n = coordinates.len();
    let mut boundaries = ends.to_vec();
    boundaries.extend([n - 4, n - 3, n - 2, n - 1]);

    let mut out = Vec::with_capacity(n);
    let mut start = 0;
    for end in boundaries {
        let contour = iup_contour(&deltas[start..end + 1], &coordinates[start..end + 1]);
        out.extend(contour);
        start = end + 1;
    }
    out
}

pub struct Axis {
    pub minimum: f64,
    pub default: f64,
    pub maximum: f64,
}

impl Axis {
    pub fn tag() -> Tag {
        Tag::new(b"wght")
    }

    pub fn new(minimum: f64, default: f64, maximum: f64) -> Axis {
        Axis { minimum, default, maximum }
    }

    pub fn clamp(&self, value: f64) -> f64 {
        self.minimum.max(self.maximum.min(value))
    }

    pub fn normalize(&self, value: f64) -> f64 {
        let value = self.clamp(value);
        if value < self.default {
            if self.default > self.minimum {
                return (value - self.default) / (self.default - self.minimum);
            }
            return 0.0;
        }
        if value > self.default {
            if self.maximum > self.default {
                return (value - self.default) / (self.maximum - self.default);
            }
            return 0.0;
        }
        0.0
    }

    pub fn denormalize(&self, coordinate: f64) -> f64 {
        let coordinate = coordinate.clamp(-1.0, 1.0);
        if coordinate < 0.0 {
            return self.default + coordinate * (self.default - self.minimum);
        }
        if coordinate > 0.0 {
            return self.default + coordinate * (self.maximum - self.default);
        }
        self.default
    }

    pub fn weights(&self) -> Vec<Weight> {
        Weight::all().into_iter().filter(|weight| self.minimum <= weight.value() as f64 && weight.value() as f64 <= self.maximum).collect()
    }

    pub fn matches(&self, other: &Axis) -> bool {
        (self.minimum - other.minimum).abs() < epsilon && (self.default - other.default).abs() < epsilon && (self.maximum - other.maximum).abs() < epsilon
    }

    pub fn of(fonts: &[&Font], default: f64) -> Axis {
        let mut ranges = Vec::new();
        for font in fonts {
            if let Some(fvar) = font.read::<read_fonts::tables::fvar::Fvar>() {
                for entry in fvar.axes().expect("failed to parse fvar axes") {
                    if entry.axis_tag() == Axis::tag() {
                        ranges.push((entry.min_value().to_f64(), entry.max_value().to_f64()));
                    }
                }
            }
        }

        if ranges.is_empty() {
            return Axis::new(default, default, default);
        }

        let minimum = ranges.iter().map(|(minimum, _)| *minimum).fold(f64::MIN, f64::max);
        let maximum = ranges.iter().map(|(_, maximum)| *maximum).fold(f64::MAX, f64::min);
        Axis::new(minimum, default, maximum)
    }
}

pub struct Space {
    pub axis: Axis,
    pub segments: Vec<(f64, f64)>,
}

impl Space {
    pub fn identity() -> Vec<(f64, f64)> {
        vec![(-1.0, -1.0), (0.0, 0.0), (1.0, 1.0)]
    }

    pub fn new(axis: Axis, segments: Option<Vec<(f64, f64)>>) -> Space {
        let mut segments = segments.unwrap_or_else(Space::identity);
        segments.sort_by(|a, b| a.partial_cmp(b).unwrap());
        Space { axis, segments }
    }

    pub fn read(font: &Font) -> Option<Space> {
        let fvar = font.read::<read_fonts::tables::fvar::Fvar>()?;
        let mappings = Space::mappings(font);

        for (index, entry) in fvar.axes().expect("failed to parse fvar axes").iter().enumerate() {
            if entry.axis_tag() != Axis::tag() {
                continue;
            }
            let segments = mappings.get(index).and_then(|found| if found.is_empty() { None } else { Some(found.clone()) });
            return Some(Space::new(Axis::new(entry.min_value().to_f64(), entry.default_value().to_f64(), entry.max_value().to_f64()), segments));
        }

        None
    }

    pub fn mappings(font: &Font) -> Vec<Vec<(f64, f64)>> {
        let Some(avar) = font.read::<read_fonts::tables::avar::Avar>() else {
            return Vec::new();
        };

        avar.axis_segment_maps()
            .iter()
            .map(|map| {
                let map = map.as_ref().expect("failed to parse avar segment");
                map.axis_value_maps()
                    .iter()
                    .map(|value| (value.from_coordinate().to_f32() as f64, value.to_coordinate().to_f32() as f64))
                    .collect()
            })
            .collect()
    }

    pub fn inverse(&self) -> Vec<(f64, f64)> {
        let mut pairs: Vec<(f64, f64)> = self.segments.iter().map(|(plain, mapped)| (*mapped, *plain)).collect();
        pairs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        pairs
    }

    pub fn normalize(&self, weight: f64) -> f64 {
        interpolate(self.axis.normalize(weight), &self.segments)
    }

    pub fn denormalize(&self, coordinate: f64) -> f64 {
        self.axis.denormalize(interpolate(coordinate, &self.inverse()))
    }

    pub fn linear(&self) -> bool {
        self.segments.iter().all(|(plain, mapped)| (plain - mapped).abs() < epsilon)
    }

    pub fn matches(&self, other: &Space) -> bool {
        self.axis.matches(&other.axis)
            && self.segments.len() == other.segments.len()
            && self.segments.iter().zip(&other.segments).all(|((a, b), (c, d))| (a - c).abs() < epsilon && (b - d).abs() < epsilon)
    }

    pub fn breakpoints(&self) -> Vec<f64> {
        let mut found: Vec<f64> = self.segments.iter().map(|(plain, _)| self.axis.denormalize(*plain)).collect();
        found.push(self.axis.minimum);
        found.push(self.axis.default);
        found.push(self.axis.maximum);
        found.sort_by(f64::total_cmp);
        found.dedup();
        found
    }

    pub fn segment_map(&self) -> SegmentMaps {
        SegmentMaps::new(
            self.segments
                .iter()
                .map(|(plain, mapped)| AxisValueMap::new(F2Dot14::from_f32(*plain as f32), F2Dot14::from_f32(*mapped as f32)))
                .collect(),
        )
    }

    pub fn avar(&self) -> Option<Avar> {
        if self.linear() {
            return None;
        }
        Some(Avar::new(vec![self.segment_map()]))
    }
}

pub struct Mapping {
    pub space: Space,
    pub source: Option<Space>,
}

impl Mapping {
    pub fn new(font: &Font, space: &Space) -> Mapping {
        Mapping {
            space: Space::new(Axis::new(space.axis.minimum, space.axis.default, space.axis.maximum), Some(space.segments.clone())),
            source: Space::read(font),
        }
    }

    pub fn coordinate(&self, weight: f64) -> f64 {
        self.source.as_ref().expect("mapping has no source").normalize(weight)
    }

    pub fn weight(&self, coordinate: f64) -> f64 {
        self.source.as_ref().expect("mapping has no source").denormalize(coordinate)
    }

    pub fn settled(&self) -> bool {
        match &self.source {
            None => true,
            Some(source) => source.matches(&self.space),
        }
    }

    pub fn axes(&self, font: &Font) -> Vec<Tag> {
        let Some(fvar) = font.read::<read_fonts::tables::fvar::Fvar>() else {
            return vec![Axis::tag()];
        };
        fvar.axes().expect("failed to parse fvar axes").iter().map(|entry| entry.axis_tag()).collect()
    }

    pub fn supports(&self, font: &Font) -> Vec<(f64, f64, f64)> {
        let tags = self.axes(font);
        let mut found = Vec::new();

        if let Some(position) = tags.iter().position(|tag| *tag == Axis::tag()) {
            for tuple in Mapping::tuples(font) {
                found.push(tuple[position]);
            }
        }

        for store in Mapping::stores(font) {
            for region in Mapping::regions(&store) {
                for (tag, entry) in tags.iter().zip(region) {
                    if *tag == Axis::tag() && entry.1 != 0.0 {
                        found.push(entry);
                    }
                }
            }
        }

        found
    }

    pub fn levels(&self, font: &Font) -> Vec<(Tag, Vec<f64>)> {
        let tags = self.axes(font);
        let mut found: Vec<(Tag, Vec<f64>)> = tags.iter().filter(|tag| **tag != Axis::tag()).map(|tag| (*tag, vec![0.0])).collect();
        if found.is_empty() {
            return found;
        }

        for tuple in Mapping::tuples(font) {
            for (position, tag) in tags.iter().enumerate() {
                if let Some(entry) = found.iter_mut().find(|(found, _)| found == tag) {
                    let (start, peak, end) = tuple[position];
                    entry.1.extend([start, peak, end]);
                }
            }
        }

        for store in Mapping::stores(font) {
            for region in Mapping::regions(&store) {
                for (tag, (start, peak, end)) in tags.iter().zip(region) {
                    if peak != 0.0 {
                        if let Some(entry) = found.iter_mut().find(|(found, _)| found == tag) {
                            entry.1.extend([start, peak, end]);
                        }
                    }
                }
            }
        }

        for (_, values) in found.iter_mut() {
            values.sort_by(f64::total_cmp);
            values.dedup();
        }
        found
    }

    pub fn tuples(font: &Font) -> Vec<Vec<(f64, f64, f64)>> {
        let data = font.data();
        let Ok(reference) = FontRef::new(&data) else {
            return Vec::new();
        };
        let Ok(gvar) = reference.gvar() else {
            return Vec::new();
        };

        let mut found = Vec::new();
        let count = font.glyph_count();
        for index in 0..count {
            let Ok(Some(variations)) = gvar.glyph_variation_data(GlyphId::new(index as u32)) else {
                continue;
            };
            for tuple in variations.tuples() {
                let peaks: Vec<f64> = tuple.peak().values().iter().map(|value| value.get().to_f32() as f64).collect();
                let (starts, ends) = match (tuple.intermediate_start(), tuple.intermediate_end()) {
                    (Some(start), Some(end)) => (
                        start.values().iter().map(|value| value.get().to_f32() as f64).collect(),
                        end.values().iter().map(|value| value.get().to_f32() as f64).collect(),
                    ),
                    _ => (
                        peaks.iter().map(|peak| peak.min(0.0)).collect::<Vec<f64>>(),
                        peaks.iter().map(|peak| peak.max(0.0)).collect::<Vec<f64>>(),
                    ),
                };
                found.push(peaks.iter().zip(starts.iter().zip(&ends)).map(|(peak, (start, end))| (*start, *peak, *end)).collect());
            }
        }
        found
    }

    pub fn stores(font: &Font) -> Vec<Vec<u8>> {
        let mut found = Vec::new();

        if let Some(gdef) = font.read::<read_fonts::tables::gdef::Gdef>() {
            if let Some(Ok(store)) = gdef.item_var_store() {
                found.push(store.offset_data().as_bytes().to_vec());
            }
        }
        for tag in [tags::HVAR, tags::VVAR, tags::MVAR] {
            let Some(data) = font.get(tag) else { continue };
            let offset = u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize;
            if offset != 0 && offset < data.len() {
                found.push(data[offset..].to_vec());
            }
        }

        found
    }

    pub fn regions(store: &[u8]) -> Vec<Vec<(f64, f64, f64)>> {
        let Ok(parsed) = ItemVariationStore::read(FontData::new(store)) else {
            return Vec::new();
        };
        let Ok(list) = parsed.variation_region_list() else {
            return Vec::new();
        };
        list.variation_regions()
            .iter()
            .map(|region| {
                region
                    .expect("failed to parse region")
                    .region_axes()
                    .iter()
                    .map(|axis| (axis.start_coord().to_f32() as f64, axis.peak_coord().to_f32() as f64, axis.end_coord().to_f32() as f64))
                    .collect()
            })
            .collect()
    }

    pub fn breakpoints(&self, font: &Font) -> Vec<f64> {
        if self.source.is_none() {
            return Vec::new();
        }

        let mut found = self.source.as_ref().unwrap().breakpoints();
        for (start, peak, end) in self.supports(font) {
            found.push(self.weight(start));
            found.push(self.weight(peak));
            found.push(self.weight(end));
        }

        found.sort_by(f64::total_cmp);
        found.dedup();
        found
    }

    pub fn apply(&self, font: &mut Font, masters: &[f64]) {
        for tag in [tags::HVAR, tags::VVAR, tags::MVAR] {
            font.remove(tag);
        }

        if self.source.is_some() && !self.settled() {
            let axes = self.axes(font);
            let levels = self.levels(font);

            let mut pairs: Vec<(NormalizedLocation, HashMap<Tag, f64>)> = Vec::new();
            for chosen in Mapping::product(&levels) {
                let extra: Vec<(Tag, f64)> = levels.iter().map(|(tag, _)| *tag).zip(chosen.iter().copied()).filter(|(_, value)| *value != 0.0).collect();
                for weight in masters {
                    let mut location = NormalizedLocation::new();
                    let mut coordinate = HashMap::new();
                    for (tag, value) in &extra {
                        location.insert(tagged(*tag), fontdrasil::coords::NormalizedCoord::new(*value));
                        coordinate.insert(*tag, *value);
                    }
                    if self.space.normalize(*weight) != 0.0 {
                        location.insert(tagged(Axis::tag()), fontdrasil::coords::NormalizedCoord::new(self.space.normalize(*weight)));
                    }
                    coordinate.insert(Axis::tag(), self.coordinate(*weight));
                    pairs.push((location, coordinate));
                }
            }

            let locations: HashSet<NormalizedLocation> = pairs.iter().map(|(location, _)| location.clone()).collect();
            let model = VariationModel::new(locations, axes.iter().map(|tag| tagged(*tag)).collect());

            if font.contains(tags::GVAR) {
                self.outlines(font, &model, &pairs, &axes);
            }
            self.deltas(font, &model, &pairs, &axes);
        }

        self.declare(font);
    }

    pub fn product(levels: &[(Tag, Vec<f64>)]) -> Vec<Vec<f64>> {
        let mut combinations = vec![Vec::new()];
        for (_, values) in levels {
            let mut extended = Vec::new();
            for combination in &combinations {
                for value in values {
                    let mut entry = combination.clone();
                    entry.push(*value);
                    extended.push(entry);
                }
            }
            combinations = extended;
        }
        combinations
    }

    pub fn tents(region: &ModelRegion, axes: &[Tag]) -> Vec<Tent> {
        axes.iter()
            .map(|tag| match region.get(&tagged(*tag)) {
                Some(tent) => Tent::new(
                    F2Dot14::from_f32(tent.peak.to_f64() as f32),
                    Some((F2Dot14::from_f32(tent.min.to_f64() as f32), F2Dot14::from_f32(tent.max.to_f64() as f32))),
                ),
                None => Tent::new(F2Dot14::from_f32(0.0), None),
            })
            .collect()
    }

    pub fn outlines(&self, font: &mut Font, model: &VariationModel, pairs: &[(NormalizedLocation, HashMap<Tag, f64>)], axes: &[Tag]) {
        let data = font.data();
        let reference = FontRef::new(&data).expect("failed to parse font");
        let glyf = reference.glyf().expect("missing glyf");
        let loca = reference.loca(None).expect("missing loca");
        let gvar = reference.gvar().expect("missing gvar");

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
            let points = Points::of(glyph.as_ref(), &horizontal[index], vertical.as_ref().map(|found| &found[index]));
            let total = points.coordinates.len();

            let mut parsed: Vec<(Vec<(Tag, (f64, f64, f64))>, Vec<Vec2>)> = Vec::new();
            for tuple in variations.tuples() {
                let peaks: Vec<f64> = tuple.peak().values().iter().map(|value| value.get().to_f32() as f64).collect();
                let (starts, ends): (Vec<f64>, Vec<f64>) = match (tuple.intermediate_start(), tuple.intermediate_end()) {
                    (Some(start), Some(end)) => (
                        start.values().iter().map(|value| value.get().to_f32() as f64).collect(),
                        end.values().iter().map(|value| value.get().to_f32() as f64).collect(),
                    ),
                    _ => (peaks.iter().map(|peak| peak.min(0.0)).collect(), peaks.iter().map(|peak| peak.max(0.0)).collect()),
                };
                let support: Vec<(Tag, (f64, f64, f64))> = axes.iter().copied().zip(peaks.iter().zip(starts.iter().zip(&ends)).map(|(peak, (start, end))| (*start, *peak, *end))).collect();

                let mut deltas: Vec<Option<Vec2>> = vec![None; total];
                if tuple.has_deltas_for_all_points() {
                    for (position, delta) in tuple.deltas().enumerate() {
                        if position < total {
                            deltas[position] = Some(Vec2::new(delta.x_delta as f64, delta.y_delta as f64));
                        }
                    }
                } else {
                    for delta in tuple.deltas() {
                        let position = delta.position as usize;
                        if position < total {
                            deltas[position] = Some(Vec2::new(delta.x_delta as f64, delta.y_delta as f64));
                        }
                    }
                }

                let dense = iup_delta(&deltas, &points.coordinates, &points.ends);
                parsed.push((support, dense));
            }

            let mut sequences: HashMap<NormalizedLocation, Vec<Point>> = HashMap::new();
            for (location, coordinate) in pairs {
                let mut sample = points.coordinates.clone();
                for (support, dense) in &parsed {
                    let scalar = support_scalar(coordinate, support);
                    if scalar != 0.0 {
                        for (value, delta) in sample.iter_mut().zip(dense) {
                            *value += *delta * scalar;
                        }
                    }
                }
                sequences.insert(location.clone(), sample);
            }

            let deltas = model.deltas::<Point, Vec2>(&sequences).expect("failed to compute deltas");

            let default = deltas.iter().find(|(region, _)| region.is_default()).expect("model has no default");
            let origins: Vec<Point> = default.1.iter().map(|value| Point::new(value.x, value.y)).collect();

            let mut tuples = Vec::new();
            for (region, values) in &deltas {
                if region.is_default() {
                    continue;
                }
                if values.iter().all(|value| value.x == 0.0 && value.y == 0.0) {
                    continue;
                }
                let optimized: Vec<GlyphDelta> = iup_delta_optimize(
                    values.iter().map(|value| kurbo13::Vec2::new(value.x, value.y)).collect(),
                    origins.iter().map(|value| kurbo13::Point::new(value.x, value.y)).collect(),
                    tolerance,
                    &points.ends,
                ).expect("failed to optimize deltas");
                tuples.push(GlyphDeltas::new(Mapping::tents(region, axes), optimized));
            }

            rebuilt.push(GlyphVariations::new(identifier, tuples));
        }

        let table = Gvar::new(rebuilt, axes.len() as u16).expect("failed to build gvar");
        font.put(tags::GVAR, &table);
    }

    pub fn deltas(&self, font: &mut Font, model: &VariationModel, pairs: &[(NormalizedLocation, HashMap<Tag, f64>)], axes: &[Tag]) {
        let Some(gdef) = font.read::<read_fonts::tables::gdef::Gdef>() else {
            return;
        };
        let Some(Ok(store)) = gdef.item_var_store() else {
            return;
        };

        let regions = Mapping::regions(store.offset_data().as_bytes());

        let mut rebuilt_data = Vec::new();
        for data in store.item_variation_data().iter() {
            let Some(Ok(data)) = data else {
                rebuilt_data.push(None);
                continue;
            };

            let supports: Vec<Vec<(Tag, (f64, f64, f64))>> = data
                .region_indexes()
                .iter()
                .map(|index| axes.iter().copied().zip(regions[index.get() as usize].iter().copied()).collect())
                .collect();

            let count = data.item_count() as usize;
            let mut sequences: HashMap<NormalizedLocation, Vec<f64>> = HashMap::new();
            for (location, coordinate) in pairs {
                let scalars: Vec<f64> = supports.iter().map(|support| support_scalar(coordinate, support)).collect();
                let mut values = Vec::with_capacity(count);
                for inner in 0..count {
                    let deltas: Vec<f64> = data.delta_set(inner as u16).map(|value| value as f64).collect();
                    values.push(deltas.iter().zip(&scalars).map(|(delta, scalar)| delta * scalar).sum());
                }
                sequences.insert(location.clone(), values);
            }

            let deltas = model.deltas::<f64, f64>(&sequences).expect("failed to compute deltas");
            rebuilt_data.push(Some(deltas));
        }

        let mut model_regions: Vec<&ModelRegion> = Vec::new();
        for deltas in rebuilt_data.iter().flatten() {
            for (region, _) in deltas {
                if !region.is_default() && !model_regions.iter().any(|found| **found == *region) {
                    model_regions.push(region);
                }
            }
            break;
        }

        let region_list = VariationRegionList::new(
            axes.len() as u16,
            model_regions
                .iter()
                .map(|region| {
                    VariationRegion::new(
                        axes.iter()
                            .map(|tag| match region.get(&tagged(*tag)) {
                                Some(tent) => RegionAxisCoordinates::new(
                                    F2Dot14::from_f32(tent.min.to_f64() as f32),
                                    F2Dot14::from_f32(tent.peak.to_f64() as f32),
                                    F2Dot14::from_f32(tent.max.to_f64() as f32),
                                ),
                                None => RegionAxisCoordinates::new(F2Dot14::from_f32(0.0), F2Dot14::from_f32(0.0), F2Dot14::from_f32(0.0)),
                            })
                            .collect(),
                    )
                })
                .collect(),
        );

        let variation_data: Vec<Option<ItemVariationData>> = rebuilt_data
            .iter()
            .map(|deltas| {
                let deltas = deltas.as_ref()?;
                let items = deltas.iter().find(|(region, _)| !region.is_default()).map(|(_, values)| values.len()).unwrap_or(0);
                let mut rows = Vec::new();
                for item in 0..items {
                    for region in &model_regions {
                        let value = deltas.iter().find(|(found, _)| *found == **region).map(|(_, values)| values[item]).unwrap_or(0.0);
                        rows.extend((value as i16).to_be_bytes());
                    }
                }
                Some(ItemVariationData::new(
                    items as u16,
                    model_regions.len() as u16,
                    (0..model_regions.len() as u16).collect(),
                    rows,
                ))
            })
            .collect();

        let rebuilt = ItemVariationStoreOwned::new(region_list, variation_data);

        let mut owned: write_fonts::tables::gdef::Gdef = gdef.to_owned_table();
        owned.item_var_store = Some(rebuilt).into();
        font.put(tags::GDEF, &owned);
    }

    pub fn declare(&self, font: &mut Font) {
        let Some(fvar) = font.read::<read_fonts::tables::fvar::Fvar>() else {
            return;
        };
        let tags: Vec<Tag> = fvar.axes().expect("failed to parse fvar axes").iter().map(|entry| entry.axis_tag()).collect();

        let mut owned: write_fonts::tables::fvar::Fvar = fvar.to_owned_table();
        {
            let arrays = &mut owned.axis_instance_arrays;
            for entry in arrays.axes.iter_mut() {
                if entry.axis_tag == Axis::tag() {
                    entry.min_value = write_fonts::types::Fixed::from_f64(self.space.axis.minimum);
                    entry.default_value = write_fonts::types::Fixed::from_f64(self.space.axis.default);
                    entry.max_value = write_fonts::types::Fixed::from_f64(self.space.axis.maximum);
                }
            }
            arrays.instances.clear();
        }
        font.put(tags::FVAR, &owned);

        let mut segments: Vec<(Tag, Vec<(f64, f64)>)> = Vec::new();
        let mappings = Space::mappings(font);
        for (index, tag) in tags.iter().enumerate() {
            if let Some(found) = mappings.get(index) {
                if !found.is_empty() {
                    segments.push((*tag, found.clone()));
                }
            }
        }

        match self.space.avar() {
            None => segments.retain(|(tag, _)| *tag != Axis::tag()),
            Some(_) => {
                segments.retain(|(tag, _)| *tag != Axis::tag());
                segments.push((Axis::tag(), self.space.segments.clone()));
            }
        }

        let curved = segments.iter().any(|(_, pairs)| pairs.iter().any(|(plain, mapped)| (plain - mapped).abs() > epsilon));
        if curved {
            let table = Avar::new(
                tags.iter()
                    .map(|tag| {
                        let pairs = segments
                            .iter()
                            .find(|(found, _)| found == tag)
                            .map(|(_, pairs)| pairs.clone())
                            .unwrap_or_else(Space::identity);
                        SegmentMaps::new(pairs.iter().map(|(plain, mapped)| AxisValueMap::new(F2Dot14::from_f32(*plain as f32), F2Dot14::from_f32(*mapped as f32))).collect())
                    })
                    .collect(),
            );
            font.put(tags::AVAR, &table);
        } else {
            font.remove(tags::AVAR);
        }
    }
}
