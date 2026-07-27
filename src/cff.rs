use kurbo::{BezPath, PathEl, Point};

pub const STANDARD_STRINGS: [&str; 391] = [
    ".notdef", "space", "exclam", "quotedbl", "numbersign", "dollar", "percent", "ampersand", "quoteright",
    "parenleft", "parenright", "asterisk", "plus", "comma", "hyphen", "period", "slash", "zero", "one", "two",
    "three", "four", "five", "six", "seven", "eight", "nine", "colon", "semicolon", "less", "equal", "greater",
    "question", "at", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z", "bracketleft", "backslash", "bracketright", "asciicircum", "underscore",
    "quoteleft", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t",
    "u", "v", "w", "x", "y", "z", "braceleft", "bar", "braceright", "asciitilde", "exclamdown", "cent", "sterling",
    "fraction", "yen", "florin", "section", "currency", "quotesingle", "quotedblleft", "guillemotleft",
    "guilsinglleft", "guilsinglright", "fi", "fl", "endash", "dagger", "daggerdbl", "periodcentered", "paragraph",
    "bullet", "quotesinglbase", "quotedblbase", "quotedblright", "guillemotright", "ellipsis", "perthousand",
    "questiondown", "grave", "acute", "circumflex", "tilde", "macron", "breve", "dotaccent", "dieresis", "ring",
    "cedilla", "hungarumlaut", "ogonek", "caron", "emdash", "AE", "ordfeminine", "Lslash", "Oslash", "OE",
    "ordmasculine", "ae", "dotlessi", "lslash", "oslash", "oe", "germandbls", "onesuperior", "logicalnot", "mu",
    "trademark", "Eth", "onehalf", "plusminus", "Thorn", "onequarter", "divide", "brokenbar", "degree", "thorn",
    "threequarters", "twosuperior", "registered", "minus", "eth", "multiply", "threesuperior", "copyright", "Aacute",
    "Acircumflex", "Adieresis", "Agrave", "Aring", "Atilde", "Ccedilla", "Eacute", "Ecircumflex", "Edieresis",
    "Egrave", "Iacute", "Icircumflex", "Idieresis", "Igrave", "Ntilde", "Oacute", "Ocircumflex", "Odieresis",
    "Ograve", "Otilde", "Scaron", "Uacute", "Ucircumflex", "Udieresis", "Ugrave", "Yacute", "Ydieresis", "Zcaron",
    "aacute", "acircumflex", "adieresis", "agrave", "aring", "atilde", "ccedilla", "eacute", "ecircumflex",
    "edieresis", "egrave", "iacute", "icircumflex", "idieresis", "igrave", "ntilde", "oacute", "ocircumflex",
    "odieresis", "ograve", "otilde", "scaron", "uacute", "ucircumflex", "udieresis", "ugrave", "yacute", "ydieresis",
    "zcaron", "exclamsmall", "Hungarumlautsmall", "dollaroldstyle", "dollarsuperior", "ampersandsmall", "Acutesmall",
    "parenleftsuperior", "parenrightsuperior", "twodotenleader", "onedotenleader", "zerooldstyle", "oneoldstyle",
    "twooldstyle", "threeoldstyle", "fouroldstyle", "fiveoldstyle", "sixoldstyle", "sevenoldstyle", "eightoldstyle",
    "nineoldstyle", "commasuperior", "threequartersemdash", "periodsuperior", "questionsmall", "asuperior",
    "bsuperior", "centsuperior", "dsuperior", "esuperior", "isuperior", "lsuperior", "msuperior", "nsuperior",
    "osuperior", "rsuperior", "ssuperior", "tsuperior", "ff", "ffi", "ffl", "parenleftinferior", "parenrightinferior",
    "Circumflexsmall", "hyphensuperior", "Gravesmall", "Asmall", "Bsmall", "Csmall", "Dsmall", "Esmall", "Fsmall",
    "Gsmall", "Hsmall", "Ismall", "Jsmall", "Ksmall", "Lsmall", "Msmall", "Nsmall", "Osmall", "Psmall", "Qsmall",
    "Rsmall", "Ssmall", "Tsmall", "Usmall", "Vsmall", "Wsmall", "Xsmall", "Ysmall", "Zsmall", "colonmonetary",
    "onefitted", "rupiah", "Tildesmall", "exclamdownsmall", "centoldstyle", "Lslashsmall", "Scaronsmall",
    "Zcaronsmall", "Dieresissmall", "Brevesmall", "Caronsmall", "Dotaccentsmall", "Macronsmall", "figuredash",
    "hypheninferior", "Ogoneksmall", "Ringsmall", "Cedillasmall", "questiondownsmall", "oneeighth", "threeeighths",
    "fiveeighths", "seveneighths", "onethird", "twothirds", "zerosuperior", "foursuperior", "fivesuperior",
    "sixsuperior", "sevensuperior", "eightsuperior", "ninesuperior", "zeroinferior", "oneinferior", "twoinferior",
    "threeinferior", "fourinferior", "fiveinferior", "sixinferior", "seveninferior", "eightinferior", "nineinferior",
    "centinferior", "dollarinferior", "periodinferior", "commainferior", "Agravesmall", "Aacutesmall",
    "Acircumflexsmall", "Atildesmall", "Adieresissmall", "Aringsmall", "AEsmall", "Ccedillasmall", "Egravesmall",
    "Eacutesmall", "Ecircumflexsmall", "Edieresissmall", "Igravesmall", "Iacutesmall", "Icircumflexsmall",
    "Idieresissmall", "Ethsmall", "Ntildesmall", "Ogravesmall", "Oacutesmall", "Ocircumflexsmall", "Otildesmall",
    "Odieresissmall", "OEsmall", "Oslashsmall", "Ugravesmall", "Uacutesmall", "Ucircumflexsmall", "Udieresissmall",
    "Yacutesmall", "Thornsmall", "Ydieresissmall", "001.000", "001.001", "001.002", "001.003", "Black", "Bold",
    "Book", "Light", "Medium", "Regular", "Roman", "Semibold",
];

pub struct Information {
    pub postscript_name: String,
    pub full_name: String,
    pub family_name: String,
    pub weight: String,
    pub version: String,
    pub notice: String,
    pub is_fixed_pitch: bool,
    pub italic_angle: f64,
    pub underline_position: f64,
    pub underline_thickness: f64,
    pub font_bbox: [f64; 4],
    pub upem: f64,
    pub std_hw: f64,
    pub std_vw: f64,
    pub default_width: f64,
    pub nominal_width: f64,
}

impl Information {
    pub fn top(&self, strings: &mut Strings, charset: usize, charstrings: usize, size: usize, private: usize) -> Dict {
        let mut dict = Dict { data: Vec::new() };
        dict.integer(strings.sid(&self.version) as i32);
        dict.operator(0);
        dict.integer(strings.sid(&self.notice) as i32);
        dict.operator(1);
        dict.integer(strings.sid(&self.full_name) as i32);
        dict.operator(2);
        dict.integer(strings.sid(&self.family_name) as i32);
        dict.operator(3);
        dict.integer(strings.sid(&self.weight) as i32);
        dict.operator(4);

        if self.is_fixed_pitch {
            dict.integer(1);
            dict.operator(0x0c01);
        }

        if self.italic_angle != 0.0 {
            dict.number(self.italic_angle);
            dict.operator(0x0c02);
        }

        if self.underline_position != -100.0 {
            dict.number(self.underline_position);
            dict.operator(0x0c03);
        }

        if self.underline_thickness != 50.0 {
            dict.number(self.underline_thickness);
            dict.operator(0x0c04);
        }

        if self.upem != 1000.0 {
            let scale = 1.0 / self.upem;
            for value in [scale, 0.0, 0.0, scale, 0.0, 0.0] {
                dict.number(value);
            }
            dict.operator(0x0c07);
        }

        if self.font_bbox != [0.0, 0.0, 0.0, 0.0] {
            for value in self.font_bbox {
                dict.number(value);
            }
            dict.operator(5);
        }

        dict.integer(charset as i32);
        dict.operator(15);
        dict.integer(size as i32);
        dict.integer(private as i32);
        dict.operator(18);
        dict.integer(charstrings as i32);
        dict.operator(17);
        dict
    }

    pub fn private(&self) -> Dict {
        let mut dict = Dict { data: Vec::new() };
        dict.operator(6);
        dict.operator(7);
        dict.operator(8);
        dict.operator(9);
        dict.number(self.std_hw);
        dict.operator(10);
        dict.number(self.std_vw);
        dict.operator(11);
        dict.operator(0x0c0c);
        dict.operator(0x0c0d);

        if self.default_width != 0.0 {
            dict.number(self.default_width);
            dict.operator(20);
        }

        if self.nominal_width != 0.0 {
            dict.number(self.nominal_width);
            dict.operator(21);
        }

        dict
    }
}

pub struct Glyph {
    pub name: String,
    pub width: f64,
    pub path: BezPath,
}

impl Glyph {
    pub fn charstring(&self, information: &Information) -> Vec<u8> {
        let mut charstring = CharString { data: Vec::new(), x: 0, y: 0 };
        let width = otround(self.width);

        if width as f64 != information.default_width {
            charstring.number(width - otround(information.nominal_width));
        }

        for element in self.path.elements() {
            match element {
                PathEl::MoveTo(target) => {
                    charstring.point(*target);
                    charstring.operator(21);
                }
                PathEl::LineTo(target) => {
                    charstring.point(*target);
                    charstring.operator(5);
                }
                PathEl::CurveTo(first, second, target) => {
                    charstring.point(*first);
                    charstring.point(*second);
                    charstring.point(*target);
                    charstring.operator(8);
                }
                PathEl::QuadTo(..) => panic!("quadratic segments must be converted to cubic before CFF serialization"),
                PathEl::ClosePath => {}
            }
        }

        charstring.operator(14);
        charstring.data
    }
}

pub fn otround(value: f64) -> i32 {
    (value + 0.5).floor() as i32
}

pub struct CharString {
    pub data: Vec<u8>,
    pub x: i32,
    pub y: i32,
}

impl CharString {
    pub fn number(&mut self, value: i32) {
        if (-107..=107).contains(&value) {
            self.data.push((value + 139) as u8);
        } else if (108..=1131).contains(&value) {
            let value = value - 108;
            self.data.push((value >> 8) as u8 + 247);
            self.data.push(value as u8);
        } else if (-1131..=-108).contains(&value) {
            let value = -value - 108;
            self.data.push((value >> 8) as u8 + 251);
            self.data.push(value as u8);
        } else if (-32768..=32767).contains(&value) {
            self.data.push(28);
            self.data.extend((value as i16).to_be_bytes());
        } else {
            self.data.push(255);
            self.data.extend(((value as i64 * 65536).clamp(i32::MIN as i64, i32::MAX as i64) as i32).to_be_bytes());
        }
    }

    pub fn point(&mut self, target: Point) {
        let x = otround(target.x);
        let y = otround(target.y);
        self.number(x - self.x);
        self.number(y - self.y);
        self.x = x;
        self.y = y;
    }

    pub fn operator(&mut self, opcode: u8) {
        self.data.push(opcode);
    }
}

pub struct Dict {
    pub data: Vec<u8>,
}

impl Dict {
    pub fn number(&mut self, value: f64) {
        if value.fract() == 0.0 && value >= i32::MIN as f64 && value <= i32::MAX as f64 {
            self.integer(value as i32);
        } else {
            self.real(value);
        }
    }

    pub fn integer(&mut self, value: i32) {
        if (-107..=107).contains(&value) {
            self.data.push((value + 139) as u8);
        } else if (108..=1131).contains(&value) {
            let value = value - 108;
            self.data.push((value >> 8) as u8 + 247);
            self.data.push(value as u8);
        } else if (-1131..=-108).contains(&value) {
            let value = -value - 108;
            self.data.push((value >> 8) as u8 + 251);
            self.data.push(value as u8);
        } else if (-32768..=32767).contains(&value) {
            self.data.push(28);
            self.data.extend((value as i16).to_be_bytes());
        } else {
            self.data.push(29);
            self.data.extend(value.to_be_bytes());
        }
    }

    pub fn real(&mut self, value: f64) {
        let formatted = format!("{:.7e}", value);
        let (mantissa, exponent) = formatted.split_once('e').unwrap();
        let exponent: i32 = exponent.parse().unwrap();
        let digits: String = mantissa.chars().filter(|character| character.is_ascii_digit()).collect();
        let digits = digits.trim_end_matches('0');
        let digits = if digits.is_empty() { "0" } else { digits };
        let power = if digits == "0" { 0 } else { exponent - (digits.len() as i32 - 1) };
        let mut nibbles = Vec::new();

        if mantissa.starts_with('-') && digits != "0" {
            nibbles.push(0x0e);
        }

        for character in digits.chars() {
            nibbles.push(character as u8 - b'0');
        }

        if power != 0 {
            nibbles.push(if power > 0 { 0x0b } else { 0x0c });
            for character in power.abs().to_string().chars() {
                nibbles.push(character as u8 - b'0');
            }
        }

        nibbles.push(0x0f);

        if nibbles.len() % 2 == 1 {
            nibbles.push(0x0f);
        }

        self.data.push(30);
        for pair in nibbles.chunks(2) {
            self.data.push(pair[0] << 4 | pair[1]);
        }
    }

    pub fn operator(&mut self, opcode: u16) {
        if opcode > 0xff {
            self.data.push((opcode >> 8) as u8);
        }
        self.data.push(opcode as u8);
    }
}

pub struct Strings {
    pub custom: Vec<String>,
}

impl Strings {
    pub fn sid(&mut self, text: &str) -> u16 {
        if let Some(position) = STANDARD_STRINGS.iter().position(|string| *string == text) {
            return position as u16;
        }

        if let Some(position) = self.custom.iter().position(|string| string == text) {
            return STANDARD_STRINGS.len() as u16 + position as u16;
        }

        self.custom.push(text.to_string());
        STANDARD_STRINGS.len() as u16 + self.custom.len() as u16 - 1
    }
}

pub struct Index {
    pub items: Vec<Vec<u8>>,
}

impl Index {
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = (self.items.len() as u16).to_be_bytes().to_vec();

        if self.items.is_empty() {
            return data;
        }

        let total: usize = self.items.iter().map(|item| item.len()).sum();
        let size = Index::offset_size(total + 1);
        data.push(size as u8);
        let mut offset = 1usize;
        data.extend(&offset.to_be_bytes()[8 - size..]);

        for item in &self.items {
            offset += item.len();
            data.extend(&offset.to_be_bytes()[8 - size..]);
        }

        for item in &self.items {
            data.extend(item);
        }

        data
    }

    pub fn offset_size(offset: usize) -> usize {
        if offset < 0x100 {
            1
        } else if offset < 0x10000 {
            2
        } else if offset < 0x1000000 {
            3
        } else {
            4
        }
    }
}

pub fn cff(information: &Information, glyphs: &[Glyph]) -> Vec<u8> {
    let mut strings = Strings { custom: Vec::new() };
    let mut charset = vec![0u8];

    for glyph in &glyphs[1..] {
        charset.extend(strings.sid(&glyph.name).to_be_bytes());
    }

    let charstrings = Index { items: glyphs.iter().map(|glyph| glyph.charstring(information)).collect() }.serialize();
    let private = information.private().data;
    let names = Index { items: vec![information.postscript_name.clone().into_bytes()] }.serialize();
    let mut top = information.top(&mut strings, 0, 0, private.len(), 0);
    let customs = Index { items: strings.custom.iter().map(|string| string.clone().into_bytes()).collect() }.serialize();
    let subroutines = Index { items: Vec::new() }.serialize();
    let mut offsets = (0usize, 0usize, 0usize);

    loop {
        let index = Index { items: vec![top.data.clone()] }.serialize();
        let start = 4 + names.len() + index.len() + customs.len() + subroutines.len();
        let next = (start, start + charset.len(), start + charset.len() + charstrings.len());

        if next == offsets {
            let mut data = vec![1, 0, 4, 4];
            data.extend(&names);
            data.extend(&index);
            data.extend(&customs);
            data.extend(&subroutines);
            data.extend(&charset);
            data.extend(&charstrings);
            data.extend(&private);
            return data;
        }

        offsets = next;
        top = information.top(&mut strings, offsets.0, offsets.1, private.len(), offsets.2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn contour(points: &[(f64, f64)]) -> BezPath {
        let mut path = BezPath::new();
        path.move_to(points[0]);
        for point in &points[1..] {
            path.line_to(*point);
        }
        path.close_path();
        path
    }

    #[test]
    fn serialize() {
        let information = Information {
            postscript_name: "NerconeTest-Regular".to_string(),
            full_name: "Nercone Test Regular".to_string(),
            family_name: "Nercone Test".to_string(),
            weight: "Regular".to_string(),
            version: "1.234".to_string(),
            notice: "SIL Open Font License, Version 1.1".to_string(),
            is_fixed_pitch: false,
            italic_angle: 0.0,
            underline_position: -100.0,
            underline_thickness: 50.0,
            font_bbox: [50.0, -200.0, 900.0, 1400.0],
            upem: 2048.0,
            std_hw: 82.0,
            std_vw: 102.0,
            default_width: 500.0,
            nominal_width: 100.0,
        };

        let mut curved = BezPath::new();
        curved.move_to((100.0, 0.0));
        curved.curve_to((200.0, 300.7), (400.5, 300.0), (500.0, 0.0));
        curved.close_path();
        curved.move_to((150.0, 50.0));
        curved.line_to((450.0, 50.0));

        let glyphs = [
            Glyph { name: ".notdef".to_string(), width: 600.0, path: contour(&[(50.0, 0.0), (550.0, 0.0), (550.0, 1400.0), (50.0, 1400.0)]) },
            Glyph { name: "space".to_string(), width: 500.0, path: BezPath::new() },
            Glyph { name: "A".to_string(), width: 1024.0, path: contour(&[(100.0, 100.0), (900.0, 100.0), (900.0, 900.0), (100.0, 900.0)]) },
            Glyph { name: "uni3042".to_string(), width: 1200.0, path: curved },
        ];

        let data = cff(&information, &glyphs);
        assert_eq!(&data[..4], &[1, 0, 4, 4]);

        let directory = "/private/tmp/claude-501/-Volumes-Developments-nercone-dev-fonts/d08e5eec-1bbb-4368-8fb4-36df636f3bff/scratchpad/cff-test";
        std::fs::create_dir_all(directory).unwrap();
        std::fs::write(format!("{}/test.cff", directory), &data).unwrap();
    }
}
