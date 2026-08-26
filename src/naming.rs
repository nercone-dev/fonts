use std::collections::BTreeMap;

use write_fonts::tables::fvar::InstanceRecord;
use write_fonts::OffsetMarker;
use write_fonts::tables::name::{Name, NameRecord};
use write_fonts::tables::stat::{AxisRecord, AxisValue, AxisValueFormat1, AxisValueFormat3, AxisValueTableFlags, Stat};
use write_fonts::types::{Fixed, NameId, Tag};

use crate::constants::vendor_id;
use crate::design::{epsilon, Axis};
use crate::font::{tags, Font};
use crate::models::{Family, Style, Weight};

#[allow(non_upper_case_globals)]
pub const windows: (u16, u16, u16) = (3, 1, 0x409);

pub fn title(tag: Tag) -> String {
    match &tag.to_be_bytes() {
        b"wght" => "Weight".to_string(),
        b"wdth" => "Width".to_string(),
        b"opsz" => "Optical Size".to_string(),
        b"ital" => "Italic".to_string(),
        b"slnt" => "Slant".to_string(),
        _ => tag.to_string(),
    }
}

pub struct Names<'a> {
    pub subject: &'a Family,
    pub style: &'a Style,
    pub axis: &'a Axis,
    pub release: &'a str,
    pub notice: &'a str,
}

impl<'a> Names<'a> {
    pub const COPYRIGHT: u16 = 0;
    pub const FAMILY: u16 = 1;
    pub const VARIANT: u16 = 2;
    pub const IDENTIFIER: u16 = 3;
    pub const FULL: u16 = 4;
    pub const VERSION: u16 = 5;
    pub const POSTSCRIPT: u16 = 6;
    pub const DESCRIPTION: u16 = 10;
    pub const LICENSE: u16 = 13;
    pub const LICENSE_URL: u16 = 14;
    pub const TYPOGRAPHIC_FAMILY: u16 = 16;
    pub const TYPOGRAPHIC_VARIANT: u16 = 17;
    pub const VARIATIONS: u16 = 25;

    pub fn new(subject: &'a Family, style: &'a Style, axis: &'a Axis, release: &'a str, notice: &'a str) -> Names<'a> {
        Names { subject, style, axis, release, notice }
    }

    pub fn typographic(&self) -> String {
        match self.style.variable() {
            true => format!("{} Variable", self.subject.name),
            false => self.subject.name.clone(),
        }
    }

    pub fn family(&self) -> String {
        match self.style.distinction() {
            None => self.typographic(),
            Some(weight) => format!("{} {}", self.typographic(), weight),
        }
    }

    pub fn postscript(&self) -> String {
        format!("{}-{}", self.subject.filename, self.style.name())
    }

    pub fn prefix(&self) -> String {
        format!("{}{}", self.subject.filename, self.style.name())
    }

    pub fn records(&self) -> BTreeMap<u16, String> {
        let family = self.subject;
        let style = self.style;

        let typographic = self.typographic();
        let name = self.family();
        let variant = style.ribbi();
        let postscript = self.postscript();

        let mut found = BTreeMap::new();
        found.insert(Names::COPYRIGHT, self.notice.to_string());
        found.insert(Names::FAMILY, name.clone());
        found.insert(Names::VARIANT, variant.clone());
        found.insert(Names::IDENTIFIER, format!("{};{};{}", self.release, vendor_id, postscript));
        found.insert(Names::FULL, match style.bold() || style.italic() {
            false => name.clone(),
            true => format!("{} {}", name, variant),
        });
        found.insert(Names::VERSION, format!("Version {}", self.release));
        found.insert(Names::POSTSCRIPT, postscript);
        found.insert(Names::DESCRIPTION, family.description());
        found.insert(Names::LICENSE, family.license.name.to_string());
        found.insert(Names::LICENSE_URL, family.license.url.to_string());

        if name != typographic || style.label() != variant {
            found.insert(Names::TYPOGRAPHIC_FAMILY, typographic);
            found.insert(Names::TYPOGRAPHIC_VARIANT, style.label());
        }
        found
    }

    pub fn apply(&self, font: &mut Font) {
        let mut found = self.records();
        if font.contains(tags::FVAR) {
            found.insert(Names::VARIATIONS, self.prefix());
        }

        let mut records = Vec::new();
        for (identifier, value) in found {
            if !value.is_empty() {
                records.push(NameRecord::new(windows.0, windows.1, windows.2, NameId::new(identifier), OffsetMarker::new(value)));
            }
        }

        let mut table = Name::new(records);

        if font.contains(tags::FVAR) {
            self.instances(font, &mut table);
        }
        self.axes(font, &mut table);

        table.name_record.sort();
        font.put(tags::NAME, &table);
    }

    pub fn add(table: &mut Name, value: &str) -> NameId {
        let next = table
            .name_record
            .iter()
            .map(|record| record.name_id.to_u16())
            .max()
            .unwrap_or(0)
            .max(255)
            + 1;
        table.name_record.push(NameRecord::new(windows.0, windows.1, windows.2, NameId::new(next), OffsetMarker::new(value.to_string())));
        NameId::new(next)
    }

    pub fn instances(&self, font: &mut Font, table: &mut Name) {
        let fvar = font.read::<read_fonts::tables::fvar::Fvar>().expect("missing fvar");
        let entries: Vec<(Tag, f64)> = fvar
            .axes()
            .expect("failed to parse fvar axes")
            .iter()
            .map(|entry| (entry.axis_tag(), entry.default_value().to_f64()))
            .collect();

        let mut owned: write_fonts::tables::fvar::Fvar = write_fonts::from_obj::ToOwnedTable::to_owned_table(&fvar);
        let arrays = &mut owned.axis_instance_arrays;

        for entry in arrays.axes.iter_mut() {
            entry.axis_name_id = Names::add(table, &title(entry.axis_tag));
        }

        let prefix = self.prefix();
        arrays.instances.clear();
        if let Some((_, origin)) = entries.iter().find(|(tag, _)| *tag == Axis::tag()) {
            for weight in self.axis.weights() {
                let style = Style { weight: Some(weight), slope: self.style.slope };
                let coordinates: Vec<Fixed> = entries
                    .iter()
                    .map(|(tag, default)| {
                        if *tag == Axis::tag() {
                            Fixed::from_f64(weight.value() as f64)
                        } else {
                            Fixed::from_f64(*default)
                        }
                    })
                    .collect();

                let subfamily = Names::add(table, &style.label());
                let postscript = match (weight.value() as f64 - origin).abs() < epsilon {
                    true => NameId::new(Names::POSTSCRIPT),
                    false => Names::add(table, &format!("{}-{}", prefix, weight.name())),
                };

                arrays.instances.push(InstanceRecord {
                    subfamily_name_id: subfamily,
                    flags: 0,
                    coordinates,
                    post_script_name_id: Some(postscript),
                });
            }
        }

        font.put(tags::FVAR, &owned);
    }

    pub fn axes(&self, font: &mut Font, table: &mut Name) {
        let weights = match self.style.variable() {
            true => self.axis.weights(),
            false => vec![self.style.weight.expect("static style has a weight")],
        };
        let italic = self.style.italic();

        let mut records = vec![
            AxisRecord::new(Axis::tag(), Names::add(table, "Weight"), 0),
            AxisRecord::new(Tag::new(b"ital"), Names::add(table, "Italic"), 1),
        ];

        let mut values: Vec<AxisValue> = Vec::new();
        for weight in &weights {
            let name = Names::add(table, weight.name());
            if *weight == Weight::Regular {
                values.push(AxisValue::Format3(AxisValueFormat3::new(
                    0,
                    AxisValueTableFlags::ELIDABLE_AXIS_VALUE_NAME,
                    name,
                    Fixed::from_f64(weight.value() as f64),
                    Fixed::from_f64(Weight::Bold.value() as f64),
                )));
            } else {
                values.push(AxisValue::Format1(AxisValueFormat1::new(0, Default::default(), name, Fixed::from_f64(weight.value() as f64))));
            }
        }

        if italic {
            values.push(AxisValue::Format1(AxisValueFormat1::new(1, Default::default(), Names::add(table, "Italic"), Fixed::from_f64(1.0))));
        } else {
            values.push(AxisValue::Format3(AxisValueFormat3::new(
                1,
                AxisValueTableFlags::ELIDABLE_AXIS_VALUE_NAME,
                Names::add(table, "Roman"),
                Fixed::from_f64(0.0),
                Fixed::from_f64(1.0),
            )));
        }

        if let Some(fvar) = font.read::<read_fonts::tables::fvar::Fvar>() {
            let declared: Vec<Tag> = records.iter().map(|record| record.axis_tag).collect();
            for entry in fvar.axes().expect("failed to parse fvar axes") {
                let tag = entry.axis_tag();
                if !declared.contains(&tag) {
                    records.push(AxisRecord::new(tag, Names::add(table, &title(tag)), records.len() as u16));
                }
            }
        }

        let mut stat = Stat::new(records, values, NameId::new(0));
        stat.elided_fallback_name_id = Some(Names::add(table, "Regular"));
        font.put(tags::STAT, &stat);
    }
}

pub struct Notice;

impl Notice {
    pub fn of(fonts: &[&Font]) -> String {
        let mut notices: Vec<String> = Vec::new();
        for font in fonts {
            let Some(name) = font.read::<read_fonts::tables::name::Name>() else {
                continue;
            };
            let Some(value) = Notice::debug(&name, Names::COPYRIGHT) else {
                continue;
            };
            let value = value.split_whitespace().collect::<Vec<&str>>().join(" ");
            if !value.is_empty() && !notices.contains(&value) {
                notices.push(value);
            }
        }
        notices.join("\n")
    }

    pub fn debug(name: &read_fonts::tables::name::Name, identifier: u16) -> Option<String> {
        for record in name.name_record() {
            if record.name_id().to_u16() != identifier {
                continue;
            }
            let Ok(value) = record.string(name.string_data()) else {
                continue;
            };
            let found: String = value.chars().collect();
            if !found.is_empty() {
                return Some(found);
            }
        }
        None
    }
}
