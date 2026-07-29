use std::collections::BTreeMap;

use kurbo::Vec2;
use read_fonts::{FontRead, FontRef, TableProvider};
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::tables::base::{Axis as BaseAxis, Base, BaseCoord, MinMax};
use write_fonts::tables::gdef::Gdef;
use write_fonts::tables::gpos::Gpos;
use write_fonts::tables::gsub::Gsub;
use write_fonts::tables::gvar::{GlyphDelta, GlyphDeltas, GlyphVariations, Gvar, Tent};
use write_fonts::tables::layout::{ClassDef, ClassDefFormat2, ClassRangeRecord, CoverageTable, Feature, FeatureList, FeatureRecord, LangSys, LangSysRecord, Script, ScriptList, ScriptRecord};
use write_fonts::tables::variations::{ItemVariationData, ItemVariationStore, RegionAxisCoordinates, VariationRegion, VariationRegionList};
use write_fonts::types::{F2Dot14, Fixed, GlyphId, GlyphId16, NameId, Tag};

use crate::design::{Axis, Space};
use crate::font::{tags, Font, Metric, Points};
use crate::layout::Shifter;
use crate::prepare::Component;

pub struct Tuple {
    pub tents: Vec<Tent>,
    pub deltas: Vec<Option<Vec2>>,
}

pub struct Merger {
    pub base: Component,
    pub addons: Vec<Component>,
    pub space: Space,
    pub variable: bool,

    pub glyphs: Vec<Vec<u8>>,
    pub horizontal: Vec<Metric>,
    pub vertical: Option<Vec<Option<Metric>>>,
    pub variations: Vec<Vec<Tuple>>,
    pub assignments: BTreeMap<u32, u16>,
    pub substitutions: Option<Gsub>,
    pub positioning: Option<Gpos>,
    pub definitions: Option<Gdef>,
}

impl Merger {
    pub fn new(base: Component, addons: Vec<Component>, space: Space, variable: bool) -> Merger {
        Merger {
            base,
            addons,
            space,
            variable,
            glyphs: Vec::new(),
            horizontal: Vec::new(),
            vertical: None,
            variations: Vec::new(),
            assignments: BTreeMap::new(),
            substitutions: None,
            positioning: None,
            definitions: None,
        }
    }

    pub fn build(mut self) -> Font {
        self.open();
        let addons = std::mem::take(&mut self.addons);
        for addon in addons {
            self.append(&addon);
        }
        self.close();
        self.base.font
    }

    pub fn axes(&self) -> Vec<Tag> {
        match self.base.font.read::<read_fonts::tables::fvar::Fvar>() {
            Some(fvar) => fvar.axes().expect("failed to parse fvar axes").iter().map(|entry| entry.axis_tag()).collect(),
            None => vec![Axis::tag()],
        }
    }

    pub fn open(&mut self) {
        if self.variable && !self.base.font.contains(tags::FVAR) {
            let axis = write_fonts::tables::fvar::VariationAxisRecord {
                axis_tag: Axis::tag(),
                min_value: Fixed::from_f64(self.space.axis.minimum),
                default_value: Fixed::from_f64(self.space.axis.default),
                max_value: Fixed::from_f64(self.space.axis.maximum),
                flags: Default::default(),
                axis_name_id: NameId::new(256),
            };
            let arrays = write_fonts::tables::fvar::AxisInstanceArrays::new(vec![axis], Vec::new());
            let fvar = write_fonts::tables::fvar::Fvar::new(arrays);
            self.base.font.put(tags::FVAR, &fvar);

            if let Some(avar) = self.space.avar() {
                self.base.font.put(tags::AVAR, &avar);
            }
        }

        self.defaults();

        let count = self.base.font.glyph_count();
        self.glyphs = self.base.font.glyphs();
        self.horizontal = self.base.font.metrics(tags::HHEA, tags::HMTX);
        self.variations = self.parse(&self.base.font, &self.axes());
        self.assignments = self.base.cmap();

        if self.base.font.contains(tags::VMTX) {
            self.vertical = Some(self.base.font.metrics(tags::VHEA, tags::VMTX).into_iter().map(Some).collect());
        } else {
            for addon in &self.addons {
                if addon.font.contains(tags::VHEA) && addon.font.contains(tags::VMTX) {
                    let vhea = addon.font.get(tags::VHEA).expect("missing vhea").to_vec();
                    self.base.font.set(tags::VHEA, vhea);
                    self.vertical = Some(vec![None; count]);
                    break;
                }
            }
        }

        self.baselines();

        self.substitutions = self
            .base
            .font
            .get(tags::GSUB)
            .map(|data| read_fonts::tables::gsub::Gsub::read(read_fonts::FontData::new(data)).expect("failed to parse GSUB").to_owned_table())
            .or_else(|| {
                self.addons.iter().any(|addon| addon.font.contains(tags::GSUB)).then(|| {
                    Gsub::new(ScriptList::new(Vec::new()), FeatureList::new(Vec::new()), write_fonts::tables::gsub::SubstitutionLookupList::new(Vec::new()))
                })
            });
        self.positioning = self
            .base
            .font
            .get(tags::GPOS)
            .map(|data| read_fonts::tables::gpos::Gpos::read(read_fonts::FontData::new(data)).expect("failed to parse GPOS").to_owned_table())
            .or_else(|| {
                self.addons.iter().any(|addon| addon.font.contains(tags::GPOS)).then(|| {
                    Gpos::new(ScriptList::new(Vec::new()), FeatureList::new(Vec::new()), write_fonts::tables::gpos::PositionLookupList::new(Vec::new()))
                })
            });
        self.definitions = self
            .base
            .font
            .get(tags::GDEF)
            .map(|data| read_fonts::tables::gdef::Gdef::read(read_fonts::FontData::new(data)).expect("failed to parse GDEF").to_owned_table())
            .or_else(|| self.addons.iter().any(|addon| addon.font.contains(tags::GDEF)).then(|| Gdef::new(None, None, None, None)));
    }

    pub fn baselines(&mut self) {
        if self.base.font.contains(tags::BASE) {
            return;
        }

        let mut carried: Option<Base> = None;
        for addon in &self.addons {
            if let Some(found) = addon.font.read::<read_fonts::tables::base::Base>() {
                carried = Some(found.to_owned_table());
                break;
            }
        }

        let Some(mut table) = carried else { return };
        table.item_var_store = None.into();
        for axis in [table.horiz_axis.as_mut(), table.vert_axis.as_mut()].into_iter().flatten() {
            Merger::direction(axis);
        }
        self.base.font.put(tags::BASE, &table);
    }

    pub fn direction(axis: &mut BaseAxis) {
        for record in axis.base_script_list.base_script_records.iter_mut() {
            let script = &mut record.base_script;
            if let Some(values) = script.base_values.as_mut() {
                for coordinate in values.base_coords.iter_mut() {
                    Merger::baseline(coordinate);
                }
            }
            if let Some(extremes) = script.default_min_max.as_mut() {
                Merger::extremes(extremes);
            }
            for entry in script.base_lang_sys_records.iter_mut() {
                Merger::extremes(&mut entry.min_max);
            }
        }
    }

    pub fn extremes(extremes: &mut MinMax) {
        for coordinate in [extremes.min_coord.as_mut(), extremes.max_coord.as_mut()].into_iter().flatten() {
            Merger::baseline(coordinate);
        }
        for record in extremes.feat_min_max_records.iter_mut() {
            for coordinate in [record.min_coord.as_mut(), record.max_coord.as_mut()].into_iter().flatten() {
                Merger::baseline(coordinate);
            }
        }
    }

    pub fn baseline(coordinate: &mut BaseCoord) {
        let value = match coordinate {
            BaseCoord::Format1(found) => found.coordinate,
            BaseCoord::Format2(found) => found.coordinate,
            BaseCoord::Format3(found) => found.coordinate,
        };
        *coordinate = BaseCoord::format_1(value);
    }

    pub fn defaults(&mut self) {
        for tag in [tags::GSUB, tags::GPOS] {
            let mut names: Vec<String> = Vec::new();
            let mut fonts: Vec<&Font> = Vec::new();
            fonts.push(&self.base.font);
            fonts.extend(self.addons.iter().map(|addon| &addon.font));

            for font in &fonts {
                for record in Merger::scripts(font, tag) {
                    if !names.contains(&record) {
                        names.push(record);
                    }
                }
            }
            names.sort();

            let expand = |font: &Font| -> Option<Vec<u8>> {
                let data = font.get(tag)?;

                match tag {
                    tags::GSUB => {
                        let parsed = read_fonts::tables::gsub::Gsub::read(read_fonts::FontData::new(data)).expect("failed to parse GSUB");
                        let mut owned: Gsub = parsed.to_owned_table();
                        Merger::spread(&mut owned.script_list, &names);
                        Some(write_fonts::dump_table(&owned).expect("failed to serialize GSUB"))
                    }
                    _ => {
                        let parsed = read_fonts::tables::gpos::Gpos::read(read_fonts::FontData::new(data)).expect("failed to parse GPOS");
                        let mut owned: Gpos = parsed.to_owned_table();
                        Merger::spread(&mut owned.script_list, &names);
                        Some(write_fonts::dump_table(&owned).expect("failed to serialize GPOS"))
                    }
                }
            };

            let base = expand(&self.base.font);
            if let Some(data) = base {
                self.base.font.set(tag, data);
            }
            for index in 0..self.addons.len() {
                if let Some(data) = expand(&self.addons[index].font) {
                    self.addons[index].font.set(tag, data);
                }
            }
        }
    }

    pub fn scripts(font: &Font, tag: Tag) -> Vec<String> {
        let Some(data) = font.get(tag) else {
            return Vec::new();
        };
        let list = match tag {
            tags::GSUB => read_fonts::tables::gsub::Gsub::read(read_fonts::FontData::new(data)).expect("failed to parse GSUB").script_list().expect("failed to parse scripts"),
            _ => read_fonts::tables::gpos::Gpos::read(read_fonts::FontData::new(data)).expect("failed to parse GPOS").script_list().expect("failed to parse scripts"),
        };
        list.script_records().iter().map(|record| record.script_tag().to_string()).collect()
    }

    pub fn spread(scripts: &mut ScriptList, names: &[String]) {
        let Some(default) = scripts.script_records.iter().find(|record| record.script_tag == Tag::new(b"DFLT")).map(|record| (*record.script).clone()) else {
            return;
        };

        for name in names {
            let tag = Tag::new(name.as_bytes().try_into().expect("tags are four bytes"));
            if !scripts.script_records.iter().any(|record| record.script_tag == tag) {
                scripts.script_records.push(ScriptRecord::new(tag, default.clone()));
            }
        }

        scripts.script_records.sort_by_key(|record| record.script_tag);
    }

    pub fn parse(&self, font: &Font, axes: &[Tag]) -> Vec<Vec<Tuple>> {
        let count = font.glyph_count();
        let mut found = vec![];

        if !self.variable || !font.contains(tags::GVAR) {
            found.resize_with(count, Vec::new);
            return found;
        }

        let source: Vec<Tag> = match font.read::<read_fonts::tables::fvar::Fvar>() {
            Some(fvar) => fvar.axes().expect("failed to parse fvar axes").iter().map(|entry| entry.axis_tag()).collect(),
            None => vec![Axis::tag()],
        };

        let data = font.data();
        let reference = FontRef::new(&data).expect("failed to parse font");
        let gvar = reference.gvar().expect("missing gvar");

        for index in 0..count {
            let mut tuples = Vec::new();
            if let Ok(Some(variations)) = gvar.glyph_variation_data(GlyphId::new(index as u32)) {
                for tuple in variations.tuples() {
                    let peaks: Vec<F2Dot14> = tuple.peak().values().iter().map(|value| value.get()).collect();
                    let native: Vec<Tent> = match (tuple.intermediate_start(), tuple.intermediate_end()) {
                        (Some(start), Some(end)) => peaks
                            .iter()
                            .zip(start.values().iter().zip(end.values()))
                            .map(|(peak, (low, high))| Tent::new(*peak, Some((low.get(), high.get()))))
                            .collect(),
                        _ => peaks.iter().map(|peak| Tent::new(*peak, None)).collect(),
                    };

                    let tents: Vec<Tent> = axes
                        .iter()
                        .map(|tag| match source.iter().position(|found| found == tag) {
                            Some(position) => native[position].clone(),
                            None => Tent::new(F2Dot14::from_f32(0.0), None),
                        })
                        .collect();

                    let mut deltas: Vec<Option<Vec2>> = Vec::new();
                    if tuple.has_deltas_for_all_points() {
                        for delta in tuple.deltas() {
                            deltas.push(Some(Vec2::new(delta.x_delta as f64, delta.y_delta as f64)));
                        }
                    } else {
                        let mut sparse: BTreeMap<usize, Vec2> = BTreeMap::new();
                        let mut highest = 0usize;
                        for delta in tuple.deltas() {
                            let position = delta.position as usize;
                            sparse.insert(position, Vec2::new(delta.x_delta as f64, delta.y_delta as f64));
                            highest = highest.max(position);
                        }
                        deltas.resize(highest + 1, None);
                        for (position, value) in sparse {
                            deltas[position] = Some(value);
                        }
                    }

                    tuples.push(Tuple { tents, deltas });
                }
            }
            found.push(tuples);
        }

        found
    }

    pub fn append(&mut self, addon: &Component) {
        let shift = self.glyphs.len() as u16 - 1;
        let axes = self.axes();

        for glyph in addon.font.glyphs().into_iter().skip(1) {
            self.glyphs.push(Merger::shift_components(glyph, shift));
        }

        self.horizontal.extend(addon.font.metrics(tags::HHEA, tags::HMTX).into_iter().skip(1));

        if let Some(vertical) = &mut self.vertical {
            if addon.font.contains(tags::VMTX) {
                vertical.extend(addon.font.metrics(tags::VHEA, tags::VMTX).into_iter().skip(1).map(Some));
            } else {
                vertical.extend(std::iter::repeat_with(|| None).take(addon.font.glyph_count() - 1));
            }
        }

        for (codepoint, glyph) in addon.cmap() {
            if glyph != 0 {
                self.assignments.entry(codepoint).or_insert(glyph + shift);
            }
        }

        let shifter = |lookups: u16, features: u16| Shifter {
            glyphs: 0,
            lookups,
            features,
            marks: self.definitions.as_ref().and_then(|found| found.mark_glyph_sets_def.as_ref().map(|sets| sets.coverages.len() as u16)).unwrap_or(0),
            outers: self
                .definitions
                .as_ref()
                .and_then(|found| found.item_var_store.as_ref().map(|store| store.item_variation_data.len() as u16))
                .unwrap_or(0),
        };

        let mut substitutions = addon.font.get(tags::GSUB).map(|data| {
            let parsed = read_fonts::tables::gsub::Gsub::read(read_fonts::FontData::new(data)).expect("failed to parse GSUB");
            let owned: Gsub = parsed.to_owned_table();
            owned
        });
        let mut positioning = addon.font.get(tags::GPOS).map(|data| {
            let parsed = read_fonts::tables::gpos::Gpos::read(read_fonts::FontData::new(data)).expect("failed to parse GPOS");
            let owned: Gpos = parsed.to_owned_table();
            owned
        });
        let mut definitions = addon.font.get(tags::GDEF).map(|data| {
            let parsed = read_fonts::tables::gdef::Gdef::read(read_fonts::FontData::new(data)).expect("failed to parse GDEF");
            let owned: Gdef = parsed.to_owned_table();
            owned
        });

        let mut renumber = |glyph: &mut write_fonts::types::GlyphId16| {
            if glyph.to_u16() != 0 {
                *glyph = write_fonts::types::GlyphId16::new(glyph.to_u16() + shift);
            }
        };
        if let Some(table) = &mut substitutions {
            let base = self.substitutions.as_ref().expect("GSUB accumulator missing");
            shifter(base.lookup_list.lookups.len() as u16, base.feature_list.feature_records.len() as u16).gsub(table);
            crate::layout::Visit::glyphs(table, &mut renumber);
        }
        if let Some(table) = &mut positioning {
            let base = self.positioning.as_ref().expect("GPOS accumulator missing");
            shifter(base.lookup_list.lookups.len() as u16, base.feature_list.feature_records.len() as u16).gpos(table);
            crate::layout::Visit::glyphs(table, &mut renumber);
        }
        if let Some(table) = &mut definitions {
            shifter(0, 0).gdef(table);
            crate::layout::Visit::glyphs(table, &mut renumber);
        }

        self.variations.extend(self.parse(&addon.font, &axes).into_iter().skip(1));

        self.store(definitions.as_mut());

        if let (Some(base), Some(table)) = (&mut self.substitutions, substitutions) {
            base.lookup_list.lookups.extend(table.lookup_list.into_inner().lookups);
            let features = std::mem::take(&mut base.feature_list.feature_records);
            let merged = Merger::records(features, table.feature_list.into_inner().feature_records, &mut base.script_list, table.script_list.into_inner());
            base.feature_list.feature_records = merged;
        }
        if let (Some(base), Some(table)) = (&mut self.positioning, positioning) {
            base.lookup_list.lookups.extend(table.lookup_list.into_inner().lookups);
            let features = std::mem::take(&mut base.feature_list.feature_records);
            let merged = Merger::records(features, table.feature_list.into_inner().feature_records, &mut base.script_list, table.script_list.into_inner());
            base.feature_list.feature_records = merged;
        }

        if let Some(table) = definitions {
            self.classes(table);
        }
    }

    pub fn shift_components(glyph: Vec<u8>, shift: u16) -> Vec<u8> {
        if glyph.len() < 10 {
            return glyph;
        }
        let contours = i16::from_be_bytes(glyph[0..2].try_into().unwrap());
        if contours >= 0 {
            return glyph;
        }

        let mut data = glyph;
        let mut position = 10;
        loop {
            let flags = u16::from_be_bytes(data[position..position + 2].try_into().unwrap());
            let index = u16::from_be_bytes(data[position + 2..position + 4].try_into().unwrap());
            if index != 0 {
                data[position + 2..position + 4].copy_from_slice(&(index + shift).to_be_bytes());
            }

            position += 4;
            position += if flags & 0x0001 != 0 { 4 } else { 2 };
            if flags & 0x0008 != 0 {
                position += 2;
            }
            if flags & 0x0040 != 0 {
                position += 4;
            }
            if flags & 0x0080 != 0 {
                position += 8;
            }
            if flags & 0x0020 == 0 {
                break;
            }
        }
        data
    }

    pub fn store(&mut self, definitions: Option<&mut Gdef>) {
        let Some(source) = definitions.and_then(|table| std::mem::take(&mut table.item_var_store).into_inner()) else {
            return;
        };

        let axes = self.axes();
        let base = self.definitions.as_mut().expect("GDEF accumulator missing");

        let aligned: Vec<VariationRegion> = source
            .variation_region_list
            .variation_regions
            .iter()
            .map(|region| {
                if region.region_axes.len() == axes.len() {
                    return region.clone();
                }
                let mut rebuilt = region.region_axes.clone();
                while rebuilt.len() < axes.len() {
                    rebuilt.insert(0, RegionAxisCoordinates::new(F2Dot14::from_f32(0.0), F2Dot14::from_f32(0.0), F2Dot14::from_f32(0.0)));
                }
                VariationRegion::new(rebuilt)
            })
            .collect();

        if base.item_var_store.is_none() {
            let empty = ItemVariationStore::new(VariationRegionList::new(axes.len() as u16, Vec::new()), Vec::new());
            base.item_var_store = Some(empty).into();
        }

        let target = base.item_var_store.as_mut().expect("VarStore accumulator missing");
        let regions = target.variation_region_list.variation_regions.len() as u16;

        for data in source.item_variation_data.iter() {
            let rebuilt = data.as_ref().map(|found| {
                ItemVariationData::new(
                    found.item_count,
                    found.word_delta_count,
                    found.region_indexes.iter().map(|index| index + regions).collect(),
                    found.delta_sets.clone(),
                )
            });
            target.item_variation_data.push(rebuilt.into());
        }
        target.variation_region_list.variation_regions.extend(aligned);
        target.variation_region_list.axis_count = axes.len() as u16;
    }

    pub fn records(base: Vec<FeatureRecord>, extra: Vec<FeatureRecord>, scripts: &mut ScriptList, others: ScriptList) -> Vec<FeatureRecord> {
        let mut features: Vec<FeatureRecord> = base;
        features.extend(extra);

        let mut merged: BTreeMap<Tag, Vec<ScriptRecord>> = BTreeMap::new();
        for record in scripts.script_records.drain(..) {
            merged.entry(record.script_tag).or_default().push(record);
        }
        for record in others.script_records {
            merged.entry(record.script_tag).or_default().push(record);
        }

        let mut rebuilt = Vec::new();
        for (tag, group) in merged {
            if group.len() == 1 {
                rebuilt.push(group.into_iter().next().unwrap());
                continue;
            }

            let mut languages: BTreeMap<Tag, Vec<LangSys>> = BTreeMap::new();
            let mut defaults: Vec<LangSys> = Vec::new();
            for record in &group {
                if let Some(found) = record.script.default_lang_sys.as_ref() {
                    defaults.push(found.clone());
                }
                for language in &record.script.lang_sys_records {
                    languages.entry(language.lang_sys_tag).or_default().push((*language.lang_sys).clone());
                }
            }

            let mut script = Script::new(None, Vec::new());
            if !defaults.is_empty() {
                script.default_lang_sys = Some(Merger::languages(defaults, &mut features)).into();
            }
            for (language, group) in languages {
                script.lang_sys_records.push(LangSysRecord::new(language, Merger::languages(group, &mut features)));
            }

            rebuilt.push(ScriptRecord::new(tag, script));
        }

        scripts.script_records = rebuilt;
        features
    }

    pub fn languages(group: Vec<LangSys>, features: &mut Vec<FeatureRecord>) -> LangSys {
        if group.len() == 1 {
            return group.into_iter().next().unwrap();
        }

        let mut ordered: Vec<Tag> = Vec::new();
        let mut lookups: BTreeMap<Tag, Vec<u16>> = BTreeMap::new();
        for entry in &group {
            for index in &entry.feature_indices {
                let record = &features[*index as usize];
                let tag = record.feature_tag;
                if !ordered.contains(&tag) {
                    ordered.push(tag);
                }
                lookups.entry(tag).or_default().extend(record.feature.lookup_list_indices.iter().copied());
            }
        }

        let mut indices = Vec::new();
        for tag in ordered {
            let feature = Feature::new(None, lookups.remove(&tag).unwrap_or_default());
            features.push(FeatureRecord::new(tag, feature));
            indices.push((features.len() - 1) as u16);
        }

        LangSys::new(indices)
    }

    pub fn classes(&mut self, addon: Gdef) {
        let base = self.definitions.as_mut().expect("GDEF accumulator missing");

        for (own, other) in [
            (&mut base.glyph_class_def, addon.glyph_class_def),
            (&mut base.mark_attach_class_def, addon.mark_attach_class_def),
        ] {
            let Some(other) = other.into_inner() else { continue };
            match own.as_mut() {
                None => *own = Some(other).into(),
                Some(found) => {
                    let mut mapping: BTreeMap<u16, u16> = Merger::mapping(found);
                    mapping.extend(Merger::mapping(&other));
                    *found = Merger::classdef(&mapping);
                }
            }
        }

        if let Some(other) = addon.attach_list.into_inner() {
            match base.attach_list.as_mut() {
                None => base.attach_list = Some(other).into(),
                Some(found) => {
                    let mut glyphs = found.coverage.iter().collect::<Vec<GlyphId16>>();
                    glyphs.extend(other.coverage.iter());
                    found.coverage = glyphs.into_iter().collect::<CoverageTable>().into();
                    found.attach_points.extend(other.attach_points);
                }
            }
        }

        if let Some(other) = addon.lig_caret_list.into_inner() {
            match base.lig_caret_list.as_mut() {
                None => base.lig_caret_list = Some(other).into(),
                Some(found) => {
                    let mut glyphs = found.coverage.iter().collect::<Vec<GlyphId16>>();
                    glyphs.extend(other.coverage.iter());
                    found.coverage = glyphs.into_iter().collect::<CoverageTable>().into();
                    found.lig_glyphs.extend(other.lig_glyphs);
                }
            }
        }

        if let Some(other) = addon.mark_glyph_sets_def.into_inner() {
            match base.mark_glyph_sets_def.as_mut() {
                None => base.mark_glyph_sets_def = Some(other).into(),
                Some(found) => {
                    found.coverages.extend(other.coverages);
                }
            }
        }
    }

    pub fn mapping(definition: &ClassDef) -> BTreeMap<u16, u16> {
        let mut found = BTreeMap::new();
        match definition {
            ClassDef::Format1(table) => {
                for (index, class) in table.class_value_array.iter().enumerate() {
                    if *class != 0 {
                        found.insert(table.start_glyph_id.to_u16() + index as u16, *class);
                    }
                }
            }
            ClassDef::Format2(table) => {
                for range in &table.class_range_records {
                    if range.class != 0 {
                        for glyph in range.start_glyph_id.to_u16()..=range.end_glyph_id.to_u16() {
                            found.insert(glyph, range.class);
                        }
                    }
                }
            }
        }
        found
    }

    pub fn classdef(mapping: &BTreeMap<u16, u16>) -> ClassDef {
        let mut ranges: Vec<ClassRangeRecord> = Vec::new();
        for (glyph, class) in mapping {
            match ranges.last_mut() {
                Some(range) if range.end_glyph_id.to_u16() + 1 == *glyph && range.class == *class => {
                    range.end_glyph_id = GlyphId16::new(*glyph);
                }
                _ => ranges.push(ClassRangeRecord::new(GlyphId16::new(*glyph), GlyphId16::new(*glyph), *class)),
            }
        }
        ClassDef::Format2(ClassDefFormat2::new(ranges))
    }

    pub fn close(&mut self) {
        let glyphs = std::mem::take(&mut self.glyphs);
        self.base.font.set_glyphs(&glyphs);

        let horizontal = std::mem::take(&mut self.horizontal);
        self.base.font.set_metrics(tags::HHEA, tags::HMTX, &horizontal);

        self.characters();

        for table in [tags::GSUB, tags::GPOS] {
            self.collect(table);
        }

        if let Some(definitions) = &self.definitions {
            self.base.font.put(tags::GDEF, definitions);
        }

        if self.variable {
            self.gvar();
        }

        if self.vertical.is_some() {
            self.heights();
        }
    }

    pub fn characters(&mut self) {
        let table = crate::font::charmap(&self.assignments);
        self.base.font.set(tags::CMAP, table);
    }

    pub fn collect(&mut self, table: Tag) {
        match table {
            tags::GSUB => {
                let Some(mut owned) = self.substitutions.take() else { return };
                if owned.lookup_list.lookups.is_empty() {
                    self.base.font.remove(tags::GSUB);
                    return;
                }
                crate::layout::prune_gsub(&mut owned);
                self.base.font.put(tags::GSUB, &owned);
            }
            _ => {
                let Some(mut owned) = self.positioning.take() else { return };
                if owned.lookup_list.lookups.is_empty() {
                    self.base.font.remove(tags::GPOS);
                    return;
                }
                crate::layout::prune_gpos(&mut owned);
                self.base.font.put(tags::GPOS, &owned);
            }
        }
    }

    pub fn gvar(&mut self) {
        let axes = self.axes();
        let count = self.base.font.glyph_count();

        let data = self.base.font.data();
        let reference = FontRef::new(&data).expect("failed to parse font");
        let glyf = reference.glyf().expect("missing glyf");
        let loca = reference.loca(None).expect("missing loca");

        let horizontal = self.base.font.metrics(tags::HHEA, tags::HMTX);
        let variations = std::mem::take(&mut self.variations);

        let mut rebuilt = Vec::with_capacity(count);
        for index in 0..count {
            let identifier = GlyphId::new(index as u32);
            let tuples = variations.get(index).map(|found| found.as_slice()).unwrap_or(&[]);
            if tuples.is_empty() {
                rebuilt.push(GlyphVariations::new(identifier, Vec::new()));
                continue;
            }

            let glyph = loca.get_glyf(identifier, &glyf).expect("failed to parse glyph");
            let geometry = Points::of(glyph.as_ref(), &horizontal[index], None);
            let total = geometry.coordinates.len();

            let mut list = Vec::new();
            for tuple in tuples {
                let mut deltas = tuple.deltas.clone();
                deltas.resize(total, None);
                let dense = crate::design::iup_delta(&deltas, &geometry.coordinates, &geometry.ends);
                let flattened: Vec<GlyphDelta> = deltas
                    .iter()
                    .zip(&dense)
                    .map(|(delta, inferred)| match delta {
                        Some(value) => GlyphDelta::required(value.x as i16, value.y as i16),
                        None => GlyphDelta::optional(inferred.x as i16, inferred.y as i16),
                    })
                    .collect();
                list.push(GlyphDeltas::new(tuple.tents.clone(), flattened));
            }
            rebuilt.push(GlyphVariations::new(identifier, list));
        }

        let table = Gvar::new(rebuilt, axes.len() as u16).expect("failed to build gvar");
        self.base.font.put(tags::GVAR, &table);
    }

    pub fn heights(&mut self) {
        let Some(vertical) = std::mem::take(&mut self.vertical) else {
            return;
        };

        let upem = self.base.font.upem() as i32;
        let ascent = self
            .base
            .font
            .get(tags::VHEA)
            .map(|data| i16::from_be_bytes(data[4..6].try_into().unwrap()) as i32)
            .unwrap_or(upem / 2);

        let glyphs = self.base.font.glyphs();
        let filled: Vec<Metric> = vertical
            .into_iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                Some(metric) => metric,
                None => {
                    let top = glyphs
                        .get(index)
                        .filter(|data| data.len() >= 10)
                        .map(|data| i16::from_be_bytes(data[8..10].try_into().unwrap()) as i32)
                        .unwrap_or(0);
                    Metric { advance: upem as u16, bearing: (ascent - top) as i16 }
                }
            })
            .collect();

        self.base.font.set_metrics(tags::VHEA, tags::VMTX, &filled);
    }
}
