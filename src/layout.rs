use write_fonts::tables::layout::LookupFlag;
use write_fonts::tables::{gdef, gpos, gsub, layout};
use write_fonts::types::GlyphId16;

pub trait Visitor {
    fn glyph(&mut self, glyph: &mut GlyphId16) {
        let _ = glyph;
    }

    fn lookup(&mut self, index: &mut u16) {
        let _ = index;
    }

    fn feature(&mut self, index: &mut u16) {
        let _ = index;
    }

    fn mark_set(&mut self, index: &mut u16) {
        let _ = index;
    }

    fn outer(&mut self, index: &mut u16) {
        let _ = index;
    }
}

pub struct Glyphs<F>(pub F);

impl<F: FnMut(&mut GlyphId16)> Visitor for Glyphs<F> {
    fn glyph(&mut self, glyph: &mut GlyphId16) {
        (self.0)(glyph);
    }
}

pub struct Lookups<F>(pub F);

impl<F: FnMut(&mut u16)> Visitor for Lookups<F> {
    fn lookup(&mut self, index: &mut u16) {
        (self.0)(index);
    }
}

pub struct Features<F>(pub F);

impl<F: FnMut(&mut u16)> Visitor for Features<F> {
    fn feature(&mut self, index: &mut u16) {
        (self.0)(index);
    }
}

pub struct Marks<F>(pub F);

impl<F: FnMut(&mut u16)> Visitor for Marks<F> {
    fn mark_set(&mut self, index: &mut u16) {
        (self.0)(index);
    }
}

pub struct Outers<F>(pub F);

impl<F: FnMut(&mut u16)> Visitor for Outers<F> {
    fn outer(&mut self, index: &mut u16) {
        (self.0)(index);
    }
}

pub trait Visit {
    fn visit(&mut self, visitor: &mut impl Visitor);

    fn glyphs(&mut self, f: &mut impl FnMut(&mut GlyphId16)) {
        self.visit(&mut Glyphs(f));
    }

    fn lookups(&mut self, f: &mut impl FnMut(&mut u16)) {
        self.visit(&mut Lookups(f));
    }

    fn features(&mut self, f: &mut impl FnMut(&mut u16)) {
        self.visit(&mut Features(f));
    }

    fn marks(&mut self, f: &mut impl FnMut(&mut u16)) {
        self.visit(&mut Marks(f));
    }

    fn outers(&mut self, f: &mut impl FnMut(&mut u16)) {
        self.visit(&mut Outers(f));
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shifter {
    pub glyphs: u16,
    pub lookups: u16,
    pub features: u16,
    pub marks: u16,
    pub outers: u16,
}

impl Visitor for Shifter {
    fn glyph(&mut self, glyph: &mut GlyphId16) {
        *glyph = GlyphId16::new(glyph.to_u16() + self.glyphs);
    }

    fn lookup(&mut self, index: &mut u16) {
        *index += self.lookups;
    }

    fn feature(&mut self, index: &mut u16) {
        *index += self.features;
    }

    fn mark_set(&mut self, index: &mut u16) {
        *index += self.marks;
    }

    fn outer(&mut self, index: &mut u16) {
        *index += self.outers;
    }
}

impl Shifter {
    pub fn gsub(&self, table: &mut gsub::Gsub) {
        let mut visitor = *self;
        table.visit(&mut visitor);
    }

    pub fn gpos(&self, table: &mut gpos::Gpos) {
        let mut visitor = *self;
        table.visit(&mut visitor);
    }

    pub fn gdef(&self, table: &mut gdef::Gdef) {
        let mut visitor = *self;
        table.visit(&mut visitor);
    }
}

pub fn prune_map(used: &[bool]) -> Vec<u16> {
    let mut map = Vec::with_capacity(used.len());
    let mut next = 0;
    for keep in used {
        map.push(next);
        if *keep {
            next += 1;
        }
    }
    map
}

pub fn prune_retain<T>(items: &mut Vec<T>, used: &[bool]) {
    let mut index = 0;
    items.retain(|item| {
        let _ = item;
        let keep = used[index];
        index += 1;
        keep
    });
}

pub fn prune_gsub(table: &mut gsub::Gsub) {
    let mut used = vec![false; table.feature_list.feature_records.len()];
    table.features(&mut |index| used[*index as usize] = true);
    let map = prune_map(&used);
    prune_retain(&mut table.feature_list.feature_records, &used);
    table.features(&mut |index| *index = map[*index as usize]);

    let mut used = vec![false; table.lookup_list.lookups.len()];
    table.lookups(&mut |index| used[*index as usize] = true);
    let map = prune_map(&used);
    prune_retain(&mut table.lookup_list.lookups, &used);
    table.lookups(&mut |index| *index = map[*index as usize]);
}

pub fn prune_gpos(table: &mut gpos::Gpos) {
    let mut used = vec![false; table.feature_list.feature_records.len()];
    table.features(&mut |index| used[*index as usize] = true);
    let map = prune_map(&used);
    prune_retain(&mut table.feature_list.feature_records, &used);
    table.features(&mut |index| *index = map[*index as usize]);

    let mut used = vec![false; table.lookup_list.lookups.len()];
    table.lookups(&mut |index| used[*index as usize] = true);
    let map = prune_map(&used);
    prune_retain(&mut table.lookup_list.lookups, &used);
    table.lookups(&mut |index| *index = map[*index as usize]);
}

impl Visit for layout::CoverageTable {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            layout::CoverageTable::Format1(table) => {
                for glyph in &mut table.glyph_array {
                    visitor.glyph(glyph);
                }
            }
            layout::CoverageTable::Format2(table) => {
                for record in &mut table.range_records {
                    record.visit(visitor);
                }
            }
        }
    }
}

impl Visit for layout::RangeRecord {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        visitor.glyph(&mut self.start_glyph_id);
        visitor.glyph(&mut self.end_glyph_id);
    }
}

impl Visit for layout::ClassDef {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            layout::ClassDef::Format1(table) => visitor.glyph(&mut table.start_glyph_id),
            layout::ClassDef::Format2(table) => {
                for record in &mut table.class_range_records {
                    record.visit(visitor);
                }
            }
        }
    }
}

impl Visit for layout::ClassRangeRecord {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        visitor.glyph(&mut self.start_glyph_id);
        visitor.glyph(&mut self.end_glyph_id);
    }
}

impl Visit for layout::ScriptList {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for record in &mut self.script_records {
            record.script.visit(visitor);
        }
    }
}

impl Visit for layout::Script {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        if let Some(lang_sys) = self.default_lang_sys.as_mut() {
            lang_sys.visit(visitor);
        }
        for record in &mut self.lang_sys_records {
            record.lang_sys.visit(visitor);
        }
    }
}

impl Visit for layout::LangSys {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        if self.required_feature_index != 0xFFFF {
            visitor.feature(&mut self.required_feature_index);
        }
        for index in &mut self.feature_indices {
            visitor.feature(index);
        }
    }
}

impl Visit for layout::FeatureList {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for record in &mut self.feature_records {
            record.feature.visit(visitor);
        }
    }
}

impl Visit for layout::Feature {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for index in &mut self.lookup_list_indices {
            visitor.lookup(index);
        }
    }
}

impl<T: Visit> Visit for layout::LookupList<T> {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for lookup in &mut self.lookups {
            lookup.visit(visitor);
        }
    }
}

impl<T: Visit> Visit for layout::Lookup<T> {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        if self.lookup_flag.contains(LookupFlag::USE_MARK_FILTERING_SET) {
            if let Some(set) = &mut self.mark_filtering_set {
                visitor.mark_set(set);
            }
        }
        for subtable in &mut self.subtables {
            subtable.visit(visitor);
        }
    }
}

impl Visit for layout::SequenceLookupRecord {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        visitor.lookup(&mut self.lookup_list_index);
    }
}

impl Visit for layout::SequenceContext {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            layout::SequenceContext::Format1(table) => table.visit(visitor),
            layout::SequenceContext::Format2(table) => table.visit(visitor),
            layout::SequenceContext::Format3(table) => table.visit(visitor),
        }
    }
}

impl Visit for layout::SequenceContextFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        for set in &mut self.seq_rule_sets {
            if let Some(set) = set.as_mut() {
                set.visit(visitor);
            }
        }
    }
}

impl Visit for layout::SequenceRuleSet {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for rule in &mut self.seq_rules {
            rule.visit(visitor);
        }
    }
}

impl Visit for layout::SequenceRule {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for glyph in &mut self.input_sequence {
            visitor.glyph(glyph);
        }
        for record in &mut self.seq_lookup_records {
            record.visit(visitor);
        }
    }
}

impl Visit for layout::SequenceContextFormat2 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        self.class_def.visit(visitor);
        for set in &mut self.class_seq_rule_sets {
            if let Some(set) = set.as_mut() {
                set.visit(visitor);
            }
        }
    }
}

impl Visit for layout::ClassSequenceRuleSet {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for rule in &mut self.class_seq_rules {
            rule.visit(visitor);
        }
    }
}

impl Visit for layout::ClassSequenceRule {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for record in &mut self.seq_lookup_records {
            record.visit(visitor);
        }
    }
}

impl Visit for layout::SequenceContextFormat3 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for coverage in &mut self.coverages {
            coverage.visit(visitor);
        }
        for record in &mut self.seq_lookup_records {
            record.visit(visitor);
        }
    }
}

impl Visit for layout::ChainedSequenceContext {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            layout::ChainedSequenceContext::Format1(table) => table.visit(visitor),
            layout::ChainedSequenceContext::Format2(table) => table.visit(visitor),
            layout::ChainedSequenceContext::Format3(table) => table.visit(visitor),
        }
    }
}

impl Visit for layout::ChainedSequenceContextFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        for set in &mut self.chained_seq_rule_sets {
            if let Some(set) = set.as_mut() {
                set.visit(visitor);
            }
        }
    }
}

impl Visit for layout::ChainedSequenceRuleSet {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for rule in &mut self.chained_seq_rules {
            rule.visit(visitor);
        }
    }
}

impl Visit for layout::ChainedSequenceRule {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for glyph in &mut self.backtrack_sequence {
            visitor.glyph(glyph);
        }
        for glyph in &mut self.input_sequence {
            visitor.glyph(glyph);
        }
        for glyph in &mut self.lookahead_sequence {
            visitor.glyph(glyph);
        }
        for record in &mut self.seq_lookup_records {
            record.visit(visitor);
        }
    }
}

impl Visit for layout::ChainedSequenceContextFormat2 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        self.backtrack_class_def.visit(visitor);
        self.input_class_def.visit(visitor);
        self.lookahead_class_def.visit(visitor);
        for set in &mut self.chained_class_seq_rule_sets {
            if let Some(set) = set.as_mut() {
                set.visit(visitor);
            }
        }
    }
}

impl Visit for layout::ChainedClassSequenceRuleSet {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for rule in &mut self.chained_class_seq_rules {
            rule.visit(visitor);
        }
    }
}

impl Visit for layout::ChainedClassSequenceRule {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for record in &mut self.seq_lookup_records {
            record.visit(visitor);
        }
    }
}

impl Visit for layout::ChainedSequenceContextFormat3 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for coverage in &mut self.backtrack_coverages {
            coverage.visit(visitor);
        }
        for coverage in &mut self.input_coverages {
            coverage.visit(visitor);
        }
        for coverage in &mut self.lookahead_coverages {
            coverage.visit(visitor);
        }
        for record in &mut self.seq_lookup_records {
            record.visit(visitor);
        }
    }
}

impl Visit for layout::DeviceOrVariationIndex {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            layout::DeviceOrVariationIndex::Device(_) => {}
            layout::DeviceOrVariationIndex::VariationIndex(table) => table.visit(visitor),
            layout::DeviceOrVariationIndex::PendingVariationIndex(_) => {}
        }
    }
}

impl Visit for layout::VariationIndex {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        visitor.outer(&mut self.delta_set_outer_index);
    }
}

impl Visit for layout::FeatureVariations {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for record in &mut self.feature_variation_records {
            record.visit(visitor);
        }
    }
}

impl Visit for layout::FeatureVariationRecord {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        if let Some(substitution) = self.feature_table_substitution.as_mut() {
            substitution.visit(visitor);
        }
    }
}

impl Visit for layout::FeatureTableSubstitution {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for record in &mut self.substitutions {
            record.visit(visitor);
        }
    }
}

impl Visit for layout::FeatureTableSubstitutionRecord {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        visitor.feature(&mut self.feature_index);
        self.alternate_feature.visit(visitor);
    }
}

impl Visit for gsub::Gsub {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.script_list.visit(visitor);
        self.feature_list.visit(visitor);
        self.lookup_list.visit(visitor);
        if let Some(variations) = self.feature_variations.as_mut() {
            variations.visit(visitor);
        }
    }
}

impl Visit for gsub::SubstitutionLookup {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            gsub::SubstitutionLookup::Single(lookup) => lookup.visit(visitor),
            gsub::SubstitutionLookup::Multiple(lookup) => lookup.visit(visitor),
            gsub::SubstitutionLookup::Alternate(lookup) => lookup.visit(visitor),
            gsub::SubstitutionLookup::Ligature(lookup) => lookup.visit(visitor),
            gsub::SubstitutionLookup::Contextual(lookup) => lookup.visit(visitor),
            gsub::SubstitutionLookup::ChainContextual(lookup) => lookup.visit(visitor),
            gsub::SubstitutionLookup::Extension(lookup) => lookup.visit(visitor),
            gsub::SubstitutionLookup::Reverse(lookup) => lookup.visit(visitor),
        }
    }
}

impl Visit for gsub::SingleSubst {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            gsub::SingleSubst::Format1(table) => table.visit(visitor),
            gsub::SingleSubst::Format2(table) => table.visit(visitor),
        }
    }
}

impl Visit for gsub::SingleSubstFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
    }
}

impl Visit for gsub::SingleSubstFormat2 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        for glyph in &mut self.substitute_glyph_ids {
            visitor.glyph(glyph);
        }
    }
}

impl Visit for gsub::MultipleSubstFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        for sequence in &mut self.sequences {
            sequence.visit(visitor);
        }
    }
}

impl Visit for gsub::Sequence {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for glyph in &mut self.substitute_glyph_ids {
            visitor.glyph(glyph);
        }
    }
}

impl Visit for gsub::AlternateSubstFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        for set in &mut self.alternate_sets {
            set.visit(visitor);
        }
    }
}

impl Visit for gsub::AlternateSet {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for glyph in &mut self.alternate_glyph_ids {
            visitor.glyph(glyph);
        }
    }
}

impl Visit for gsub::LigatureSubstFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        for set in &mut self.ligature_sets {
            set.visit(visitor);
        }
    }
}

impl Visit for gsub::LigatureSet {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for ligature in &mut self.ligatures {
            ligature.visit(visitor);
        }
    }
}

impl Visit for gsub::Ligature {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        visitor.glyph(&mut self.ligature_glyph);
        for glyph in &mut self.component_glyph_ids {
            visitor.glyph(glyph);
        }
    }
}

impl Visit for gsub::SubstitutionSequenceContext {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        (**self).visit(visitor);
    }
}

impl Visit for gsub::SubstitutionChainContext {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        (**self).visit(visitor);
    }
}

impl<T: Visit> Visit for gsub::ExtensionSubstFormat1<T> {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.extension.visit(visitor);
    }
}

impl Visit for gsub::ExtensionSubtable {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            gsub::ExtensionSubtable::Single(table) => table.visit(visitor),
            gsub::ExtensionSubtable::Multiple(table) => table.visit(visitor),
            gsub::ExtensionSubtable::Alternate(table) => table.visit(visitor),
            gsub::ExtensionSubtable::Ligature(table) => table.visit(visitor),
            gsub::ExtensionSubtable::Contextual(table) => table.visit(visitor),
            gsub::ExtensionSubtable::ChainContextual(table) => table.visit(visitor),
            gsub::ExtensionSubtable::Reverse(table) => table.visit(visitor),
        }
    }
}

impl Visit for gsub::ReverseChainSingleSubstFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        for coverage in &mut self.backtrack_coverages {
            coverage.visit(visitor);
        }
        for coverage in &mut self.lookahead_coverages {
            coverage.visit(visitor);
        }
        for glyph in &mut self.substitute_glyph_ids {
            visitor.glyph(glyph);
        }
    }
}

impl Visit for gpos::Gpos {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.script_list.visit(visitor);
        self.feature_list.visit(visitor);
        self.lookup_list.visit(visitor);
        if let Some(variations) = self.feature_variations.as_mut() {
            variations.visit(visitor);
        }
    }
}

impl Visit for gpos::PositionLookup {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            gpos::PositionLookup::Single(lookup) => lookup.visit(visitor),
            gpos::PositionLookup::Pair(lookup) => lookup.visit(visitor),
            gpos::PositionLookup::Cursive(lookup) => lookup.visit(visitor),
            gpos::PositionLookup::MarkToBase(lookup) => lookup.visit(visitor),
            gpos::PositionLookup::MarkToLig(lookup) => lookup.visit(visitor),
            gpos::PositionLookup::MarkToMark(lookup) => lookup.visit(visitor),
            gpos::PositionLookup::Contextual(lookup) => lookup.visit(visitor),
            gpos::PositionLookup::ChainContextual(lookup) => lookup.visit(visitor),
            gpos::PositionLookup::Extension(lookup) => lookup.visit(visitor),
        }
    }
}

impl Visit for gpos::SinglePos {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            gpos::SinglePos::Format1(table) => table.visit(visitor),
            gpos::SinglePos::Format2(table) => table.visit(visitor),
        }
    }
}

impl Visit for gpos::SinglePosFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        self.value_record.visit(visitor);
    }
}

impl Visit for gpos::SinglePosFormat2 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        for record in &mut self.value_records {
            record.visit(visitor);
        }
    }
}

impl Visit for gpos::ValueRecord {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        if let Some(device) = self.x_placement_device.as_mut() {
            device.visit(visitor);
        }
        if let Some(device) = self.y_placement_device.as_mut() {
            device.visit(visitor);
        }
        if let Some(device) = self.x_advance_device.as_mut() {
            device.visit(visitor);
        }
        if let Some(device) = self.y_advance_device.as_mut() {
            device.visit(visitor);
        }
    }
}

impl Visit for gpos::AnchorTable {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            gpos::AnchorTable::Format1(_) => {}
            gpos::AnchorTable::Format2(_) => {}
            gpos::AnchorTable::Format3(table) => table.visit(visitor),
        }
    }
}

impl Visit for gpos::AnchorFormat3 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        if let Some(device) = self.x_device.as_mut() {
            device.visit(visitor);
        }
        if let Some(device) = self.y_device.as_mut() {
            device.visit(visitor);
        }
    }
}

impl Visit for gpos::PairPos {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            gpos::PairPos::Format1(table) => table.visit(visitor),
            gpos::PairPos::Format2(table) => table.visit(visitor),
        }
    }
}

impl Visit for gpos::PairPosFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        for set in &mut self.pair_sets {
            set.visit(visitor);
        }
    }
}

impl Visit for gpos::PairSet {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for record in &mut self.pair_value_records {
            record.visit(visitor);
        }
    }
}

impl Visit for gpos::PairValueRecord {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        visitor.glyph(&mut self.second_glyph);
        self.value_record1.visit(visitor);
        self.value_record2.visit(visitor);
    }
}

impl Visit for gpos::PairPosFormat2 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        self.class_def1.visit(visitor);
        self.class_def2.visit(visitor);
        for record in &mut self.class1_records {
            record.visit(visitor);
        }
    }
}

impl Visit for gpos::Class1Record {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for record in &mut self.class2_records {
            record.visit(visitor);
        }
    }
}

impl Visit for gpos::Class2Record {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.value_record1.visit(visitor);
        self.value_record2.visit(visitor);
    }
}

impl Visit for gpos::CursivePosFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        for record in &mut self.entry_exit_record {
            record.visit(visitor);
        }
    }
}

impl Visit for gpos::EntryExitRecord {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        if let Some(anchor) = self.entry_anchor.as_mut() {
            anchor.visit(visitor);
        }
        if let Some(anchor) = self.exit_anchor.as_mut() {
            anchor.visit(visitor);
        }
    }
}

impl Visit for gpos::MarkBasePosFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.mark_coverage.visit(visitor);
        self.base_coverage.visit(visitor);
        self.mark_array.visit(visitor);
        self.base_array.visit(visitor);
    }
}

impl Visit for gpos::MarkArray {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for record in &mut self.mark_records {
            record.visit(visitor);
        }
    }
}

impl Visit for gpos::MarkRecord {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.mark_anchor.visit(visitor);
    }
}

impl Visit for gpos::BaseArray {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for record in &mut self.base_records {
            record.visit(visitor);
        }
    }
}

impl Visit for gpos::BaseRecord {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for anchor in &mut self.base_anchors {
            if let Some(anchor) = anchor.as_mut() {
                anchor.visit(visitor);
            }
        }
    }
}

impl Visit for gpos::MarkLigPosFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.mark_coverage.visit(visitor);
        self.ligature_coverage.visit(visitor);
        self.mark_array.visit(visitor);
        self.ligature_array.visit(visitor);
    }
}

impl Visit for gpos::LigatureArray {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for attach in &mut self.ligature_attaches {
            attach.visit(visitor);
        }
    }
}

impl Visit for gpos::LigatureAttach {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for record in &mut self.component_records {
            record.visit(visitor);
        }
    }
}

impl Visit for gpos::ComponentRecord {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for anchor in &mut self.ligature_anchors {
            if let Some(anchor) = anchor.as_mut() {
                anchor.visit(visitor);
            }
        }
    }
}

impl Visit for gpos::MarkMarkPosFormat1 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.mark1_coverage.visit(visitor);
        self.mark2_coverage.visit(visitor);
        self.mark1_array.visit(visitor);
        self.mark2_array.visit(visitor);
    }
}

impl Visit for gpos::Mark2Array {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for record in &mut self.mark2_records {
            record.visit(visitor);
        }
    }
}

impl Visit for gpos::Mark2Record {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for anchor in &mut self.mark2_anchors {
            if let Some(anchor) = anchor.as_mut() {
                anchor.visit(visitor);
            }
        }
    }
}

impl Visit for gpos::PositionSequenceContext {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        (**self).visit(visitor);
    }
}

impl Visit for gpos::PositionChainContext {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        (**self).visit(visitor);
    }
}

impl<T: Visit> Visit for gpos::ExtensionPosFormat1<T> {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.extension.visit(visitor);
    }
}

impl Visit for gpos::ExtensionSubtable {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            gpos::ExtensionSubtable::Single(table) => table.visit(visitor),
            gpos::ExtensionSubtable::Pair(table) => table.visit(visitor),
            gpos::ExtensionSubtable::Cursive(table) => table.visit(visitor),
            gpos::ExtensionSubtable::MarkToBase(table) => table.visit(visitor),
            gpos::ExtensionSubtable::MarkToLig(table) => table.visit(visitor),
            gpos::ExtensionSubtable::MarkToMark(table) => table.visit(visitor),
            gpos::ExtensionSubtable::Contextual(table) => table.visit(visitor),
            gpos::ExtensionSubtable::ChainContextual(table) => table.visit(visitor),
        }
    }
}

impl Visit for gdef::Gdef {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        if let Some(class_def) = self.glyph_class_def.as_mut() {
            class_def.visit(visitor);
        }
        if let Some(attach_list) = self.attach_list.as_mut() {
            attach_list.visit(visitor);
        }
        if let Some(lig_caret_list) = self.lig_caret_list.as_mut() {
            lig_caret_list.visit(visitor);
        }
        if let Some(class_def) = self.mark_attach_class_def.as_mut() {
            class_def.visit(visitor);
        }
        if let Some(mark_glyph_sets) = self.mark_glyph_sets_def.as_mut() {
            mark_glyph_sets.visit(visitor);
        }
    }
}

impl Visit for gdef::AttachList {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
    }
}

impl Visit for gdef::LigCaretList {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.coverage.visit(visitor);
        for lig_glyph in &mut self.lig_glyphs {
            lig_glyph.visit(visitor);
        }
    }
}

impl Visit for gdef::LigGlyph {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for caret_value in &mut self.caret_values {
            caret_value.visit(visitor);
        }
    }
}

impl Visit for gdef::CaretValue {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        match self {
            gdef::CaretValue::Format1(_) => {}
            gdef::CaretValue::Format2(_) => {}
            gdef::CaretValue::Format3(table) => table.visit(visitor),
        }
    }
}

impl Visit for gdef::CaretValueFormat3 {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        self.device.visit(visitor);
    }
}

impl Visit for gdef::MarkGlyphSets {
    fn visit(&mut self, visitor: &mut impl Visitor) {
        for coverage in &mut self.coverages {
            coverage.visit(visitor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use read_fonts::{FontRef, TableProvider};
    use write_fonts::dump_table;
    use write_fonts::from_obj::ToOwnedTable;
    use write_fonts::validate::Validate;
    use write_fonts::FontWrite;

    pub fn verify<T: Clone + FontWrite + Validate>(table: &T, apply: impl Fn(&Shifter, &mut T)) {
        let baseline = dump_table(table).expect("failed to serialize table");

        let mut zeroed = table.clone();
        apply(&Shifter::default(), &mut zeroed);
        assert_eq!(dump_table(&zeroed).expect("failed to serialize table"), baseline);

        let step = Shifter { glyphs: 7, lookups: 3, features: 2, marks: 1, outers: 4 };
        let mut twice = table.clone();
        apply(&step, &mut twice);
        apply(&step, &mut twice);
        let mut once = table.clone();
        apply(&Shifter { glyphs: 14, lookups: 6, features: 4, marks: 2, outers: 8 }, &mut once);
        assert_eq!(
            dump_table(&twice).expect("failed to serialize table"),
            dump_table(&once).expect("failed to serialize table"),
        );
    }

    pub fn check(path: &str) {
        let data = std::fs::read(path).expect("failed to read font");
        let font = FontRef::new(&data).expect("failed to parse font");

        let table: gsub::Gsub = font.gsub().expect("missing GSUB").to_owned_table();
        verify(&table, |shifter, table| shifter.gsub(table));
        let table: gpos::Gpos = font.gpos().expect("missing GPOS").to_owned_table();
        verify(&table, |shifter, table| shifter.gpos(table));
        let table: gdef::Gdef = font.gdef().expect("missing GDEF").to_owned_table();
        verify(&table, |shifter, table| shifter.gdef(table));
    }

    #[test]
    fn shift_inter() {
        check("build/sources/inter/InterVariable.ttf");
    }

    #[test]
    fn shift_noto_sans_jp() {
        check("build/sources/noto/NotoSansJP.ttf");
    }

    #[test]
    fn prune_inter() {
        let data = std::fs::read("build/sources/inter/InterVariable.ttf").expect("failed to read font");
        let font = FontRef::new(&data).expect("failed to parse font");

        let mut table: gsub::Gsub = font.gsub().expect("missing GSUB").to_owned_table();
        let mut expected = table.clone();
        expected.lookup_list.lookups.remove(2);
        expected.lookups(&mut |index| {
            if *index > 2 {
                *index -= 1;
            }
        });
        prune_gsub(&mut table);
        let pruned = dump_table(&table).expect("failed to serialize table");
        assert_eq!(pruned, dump_table(&expected).expect("failed to serialize table"));
        prune_gsub(&mut table);
        assert_eq!(dump_table(&table).expect("failed to serialize table"), pruned);

        let mut table: gpos::Gpos = font.gpos().expect("missing GPOS").to_owned_table();
        let baseline = dump_table(&table).expect("failed to serialize table");
        prune_gpos(&mut table);
        assert_eq!(dump_table(&table).expect("failed to serialize table"), baseline);
    }
}
