use read_fonts::tables::glyf::{Anchor, CompositeGlyph, Glyf, Glyph as Outline, SimpleGlyph};
use read_fonts::types::GlyphId;
use read_fonts::TableProvider;

use crate::cff::{self, Glyph, Information};
use crate::font::{tags, Font, Metric};
use crate::models::{Family, Style};
use crate::qu2cu;

#[allow(non_upper_case_globals)]
pub const error: f64 = 1.0 / 2000.0;

#[derive(Clone, Copy)]
pub struct Transform {
    pub xx: f64,
    pub xy: f64,
    pub yx: f64,
    pub yy: f64,
    pub dx: f64,
    pub dy: f64,
}

impl Transform {
    pub fn of(component: &read_fonts::tables::glyf::Component) -> Transform {
        let (dx, dy) = match component.anchor {
            Anchor::Offset { x, y } => (x as f64, y as f64),
            Anchor::Point { .. } => panic!("point-anchored components are not supported"),
        };
        Transform {
            xx: component.transform.xx.to_f32() as f64,
            xy: component.transform.yx.to_f32() as f64,
            yx: component.transform.xy.to_f32() as f64,
            yy: component.transform.yy.to_f32() as f64,
            dx,
            dy,
        }
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.xx * x + self.yx * y + self.dx, self.xy * x + self.yy * y + self.dy)
    }

    pub fn compose(&self, other: &Transform) -> Transform {
        Transform {
            xx: other.xx * self.xx + other.xy * self.yx,
            xy: other.xx * self.xy + other.xy * self.yy,
            yx: other.yx * self.xx + other.yy * self.yx,
            yy: other.yx * self.xy + other.yy * self.yy,
            dx: self.xx * other.dx + self.yx * other.dy + self.dx,
            dy: self.xy * other.dx + self.yy * other.dy + self.dy,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Broken,
    Horizontal,
    Vertical,
}

pub struct Converter {
    pub elements: Vec<kurbo::PathEl>,
    pub tolerance: f64,
    pub current: (f64, f64),
    pub rounded: (i32, i32),
    pub pending: Vec<Vec<(f64, f64)>>,
    pub moved: Option<(f64, f64)>,
    pub chain: Chain,
}

impl Converter {
    pub fn new(tolerance: f64) -> Converter {
        Converter {
            elements: Vec::new(),
            tolerance,
            current: (0.0, 0.0),
            rounded: (0, 0),
            pending: Vec::new(),
            moved: None,
            chain: Chain::Broken,
        }
    }

    pub fn round(point: (f64, f64)) -> (i32, i32) {
        (cff::otround(point.0), cff::otround(point.1))
    }

    pub fn move_to(&mut self, point: (f64, f64)) {
        self.flush();
        self.moved = Some(point);
        self.current = point;
        self.rounded = Converter::round(point);
        self.chain = Chain::Broken;
    }

    pub fn materialize(&mut self) {
        if let Some(point) = self.moved.take() {
            self.elements.push(kurbo::PathEl::MoveTo(kurbo::Point::new(point.0, point.1)));
        }
    }

    pub fn line_to(&mut self, point: (f64, f64)) {
        self.flush();
        self.emit_line(point, true);
    }

    pub fn emit_line(&mut self, point: (f64, f64), mergeable: bool) {
        let rounded = Converter::round(point);
        let delta = (rounded.0 - self.rounded.0, rounded.1 - self.rounded.1);
        self.current = point;
        if delta == (0, 0) {
            self.chain = Chain::Broken;
            return;
        }
        let category = if delta.0 == 0 { Chain::Vertical } else if delta.1 == 0 { Chain::Horizontal } else { Chain::Broken };
        self.rounded = rounded;
        if category != Chain::Broken && category == self.chain {
            let last = self.elements.len() - 1;
            self.elements[last] = kurbo::PathEl::LineTo(kurbo::Point::new(point.0, point.1));
        } else {
            self.materialize();
            self.elements.push(kurbo::PathEl::LineTo(kurbo::Point::new(point.0, point.1)));
        }
        self.chain = if mergeable { category } else { Chain::Broken };
    }

    pub fn emit_curve(&mut self, first: (f64, f64), second: (f64, f64), target: (f64, f64)) {
        let rounded_first = Converter::round(first);
        let rounded_second = Converter::round(second);
        let rounded_target = Converter::round(target);
        if rounded_first == self.rounded && rounded_target == rounded_second {
            self.emit_line(target, false);
            return;
        }
        self.materialize();
        self.elements.push(kurbo::PathEl::CurveTo(
            kurbo::Point::new(first.0, first.1),
            kurbo::Point::new(second.0, second.1),
            kurbo::Point::new(target.0, target.1),
        ));
        self.current = target;
        self.rounded = rounded_target;
        self.chain = Chain::Broken;
    }

    pub fn qcurve_to(&mut self, points: &[(f64, f64)]) {
        let mut spline = Vec::with_capacity(points.len() + 1);
        spline.push(self.current);
        spline.extend_from_slice(points);
        self.pending.push(spline);
        self.current = points[points.len() - 1];
    }

    pub fn qcurve_oncurveless(&mut self, points: &[(f64, f64)]) {
        let first = points[0];
        let last = points[points.len() - 1];
        let start = (0.5 * (first.0 + last.0), 0.5 * (first.1 + last.1));
        self.move_to(start);
        let mut closed = points.to_vec();
        closed.push(start);
        self.qcurve_to(&closed);
    }

    pub fn close(&mut self) {
        self.flush();
        if self.moved.is_none() {
            self.elements.push(kurbo::PathEl::ClosePath);
        }
        self.chain = Chain::Broken;
    }

    pub fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let curves = qu2cu::quadratics_to_curves(&std::mem::take(&mut self.pending), self.tolerance, true);
        for curve in curves {
            self.emit_curve(curve[1], curve[2], curve[3]);
        }
    }

    pub fn finish(mut self) -> kurbo::BezPath {
        self.materialize();
        if let Some(kurbo::PathEl::MoveTo(..)) = self.elements.last() {
            self.elements.push(kurbo::PathEl::ClosePath);
        }
        kurbo::BezPath::from_vec(self.elements)
    }
}

pub struct Drawer<'a> {
    pub glyf: Glyf<'a>,
    pub loca: read_fonts::tables::loca::Loca<'a>,
}

impl Drawer<'_> {
    pub fn draw(&self, index: usize, metric: &Metric, converter: &mut Converter) {
        let identifier = GlyphId::new(index as u32);
        let Some(glyph) = self.loca.get_glyf(identifier, &self.glyf).expect("failed to parse glyph") else {
            return;
        };
        match glyph {
            Outline::Simple(simple) => {
                let offset = metric.bearing as f64 - simple.x_min() as f64;
                self.simple(&simple, offset, None, converter);
            }
            Outline::Composite(composite) => self.composite(&composite, None, converter),
        }
    }

    pub fn simple(&self, glyph: &SimpleGlyph, offset: f64, transform: Option<&Transform>, converter: &mut Converter) {
        let points: Vec<((f64, f64), bool)> = glyph
            .points()
            .map(|point| {
                let x = point.x as f64 + offset;
                let y = point.y as f64;
                let place = match transform {
                    Some(transform) => transform.apply(x, y),
                    None => (x, y),
                };
                (place, point.on_curve)
            })
            .collect();

        let mut start = 0;
        for end in glyph.end_pts_of_contours() {
            let end = end.get() as usize + 1;
            let mut contour = points[start..end].to_vec();
            start = end;
            if contour.is_empty() {
                continue;
            }
            if !contour.iter().any(|(_, on_curve)| *on_curve) {
                let run: Vec<(f64, f64)> = contour.iter().map(|(place, _)| *place).collect();
                converter.qcurve_oncurveless(&run);
            } else {
                let first_on_curve = contour.iter().position(|(_, on_curve)| *on_curve).expect("an on-curve point exists") + 1;
                let length = contour.len();
                contour.rotate_left(first_on_curve % length);
                converter.move_to(contour[contour.len() - 1].0);
                let mut remaining = &contour[..];
                while !remaining.is_empty() {
                    let next_on_curve = remaining.iter().position(|(_, on_curve)| *on_curve).expect("the contour ends on-curve") + 1;
                    if next_on_curve == 1 {
                        if remaining.len() > 1 {
                            converter.line_to(remaining[0].0);
                        }
                    } else {
                        let run: Vec<(f64, f64)> = remaining[..next_on_curve].iter().map(|(place, _)| *place).collect();
                        converter.qcurve_to(&run);
                    }
                    remaining = &remaining[next_on_curve..];
                }
            }
            converter.close();
        }
    }

    pub fn composite(&self, glyph: &CompositeGlyph, transform: Option<&Transform>, converter: &mut Converter) {
        for component in glyph.components() {
            let child = Transform::of(&component);
            let composed = match transform {
                Some(parent) => parent.compose(&child),
                None => child,
            };
            let identifier = GlyphId::new(component.glyph.to_u32());
            match self.loca.get_glyf(identifier, &self.glyf).expect("failed to parse glyph") {
                Some(Outline::Simple(simple)) => self.simple(&simple, 0.0, Some(&composed), converter),
                Some(Outline::Composite(nested)) => self.composite(&nested, Some(&composed), converter),
                None => {}
            }
        }
    }
}

pub struct Outlines;

impl Outlines {
    #[allow(non_upper_case_globals)]
    pub const truetype: [&'static str; 11] = ["glyf", "loca", "gvar", "cvt ", "fpgm", "prep", "cvar", "hdmx", "LTSH", "VDMX", "gasp"];

    pub fn compact(font: &mut Font, family: &Family, style: &Style, version: &str) {
        let data = font.data();
        let reference = read_fonts::FontRef::new(&data).expect("failed to parse font");
        let drawer = Drawer {
            glyf: reference.glyf().expect("missing glyf"),
            loca: reference.loca(None).expect("missing loca"),
        };
        let metrics = font.metrics(tags::HHEA, tags::HMTX);
        let tolerance = font.upem() as f64 * error;

        let count = font.glyph_count();
        let mut glyphs = Vec::with_capacity(count);
        for index in 0..count {
            let mut converter = Converter::new(tolerance);
            drawer.draw(index, &metrics[index], &mut converter);
            glyphs.push(Glyph {
                name: if index == 0 { ".notdef".to_string() } else { format!("glyph{:05}", index) },
                width: metrics[index].advance as f64,
                path: converter.finish(),
            });
        }

        let table = cff::cff(&Outlines::information(font, family, style, version), &glyphs);
        font.set(tags::CFF, table);

        for tag in Outlines::truetype {
            font.remove(write_fonts::types::Tag::new(tag.as_bytes().try_into().expect("tags are four bytes")));
        }

        let glyph_count = count as u16;
        let mut maxp = Vec::with_capacity(6);
        maxp.extend_from_slice(&0x00005000u32.to_be_bytes());
        maxp.extend_from_slice(&glyph_count.to_be_bytes());
        font.set(tags::MAXP, maxp);

        let mut head = font.get(tags::HEAD).expect("missing head").to_vec();
        head[50..52].copy_from_slice(&0i16.to_be_bytes());
        font.set(tags::HEAD, head);
    }

    pub fn information(font: &Font, family: &Family, style: &Style, version: &str) -> Information {
        let upem = font.upem() as f64;

        Information {
            postscript_name: format!("{}-{}", family.filename, style.name()),
            full_name: format!("{} {}", family.name, style.name()),
            family_name: family.name.clone(),
            weight: style.name(),
            version: version.to_string(),
            notice: family.license.name.to_string(),
            is_fixed_pitch: family.monospace,
            italic_angle: 0.0,
            underline_position: -100.0,
            underline_thickness: 50.0,
            font_bbox: [0.0, 0.0, 0.0, 0.0],
            upem,
            std_hw: (upem * 0.04).round_ties_even().max(1.0),
            std_vw: (upem * 0.05).round_ties_even().max(1.0),
            default_width: 0.0,
            nominal_width: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{version, Families};

    #[test]
    fn compact_families() {
        let directory = "/private/tmp/claude-501/-Volumes-Developments-nercone-dev-fonts/d08e5eec-1bbb-4368-8fb4-36df636f3bff/scratchpad/qu2cu-test";
        std::fs::create_dir_all(directory).unwrap();

        for name in ["NerconeSerifJP", "NerconeSansJP"] {
            let path = format!("/Volumes/Developments/nercone-dev/fonts/build/files/{}-Bold.ttf", name);
            let Ok(data) = std::fs::read(&path) else {
                eprintln!("skipping {}: no built TTF", name);
                continue;
            };

            let family = Families::all().into_iter().find(|family| family.filename == name).unwrap();
            let style = crate::models::Style { weight: Some(crate::models::Weight::Bold), slope: crate::models::Slope::Upright };

            let mut font = Font::new(&data);
            Outlines::compact(&mut font, &family, &style, version);
            std::fs::write(format!("{}/{}-Bold.otf", directory, name), font.data()).unwrap();
        }
    }
}
