use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;

#[allow(non_upper_case_globals)]
pub const user_agent: &str = "Mozilla/5.0 (compatible; +https://github.com/nercone-dev/fonts/)";

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Weight {
    Thin       = 100,
    ExtraLight = 200,
    Light      = 300,
    Regular    = 400,
    Medium     = 500,
    SemiBold   = 600,
    Bold       = 700,
    ExtraBold  = 800,
    Black      = 900,
}

impl Weight {
    pub fn value(self) -> u16 {
        self as u16
    }

    pub fn name(self) -> &'static str {
        match self {
            Weight::Thin       => "Thin",
            Weight::ExtraLight => "ExtraLight",
            Weight::Light      => "Light",
            Weight::Regular    => "Regular",
            Weight::Medium     => "Medium",
            Weight::SemiBold   => "SemiBold",
            Weight::Bold       => "Bold",
            Weight::ExtraBold  => "ExtraBold",
            Weight::Black      => "Black",
        }
    }

    pub fn all() -> [Weight; 9] {
        [Weight::Thin, Weight::ExtraLight, Weight::Light, Weight::Regular, Weight::Medium,
         Weight::SemiBold, Weight::Bold, Weight::ExtraBold, Weight::Black]
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Slope {
    Upright,
    Italic,
}

impl Slope {
    pub fn italic(self) -> bool {
        self == Slope::Italic
    }

    pub fn suffix(self) -> &'static str {
        if self.italic() { "Italic" } else { "" }
    }

    pub fn all() -> [Slope; 2] {
        [Slope::Upright, Slope::Italic]
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Format {
    Ttf,
    Otf,
    Woff,
    Woff2,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Ttf   => "ttf",
            Format::Otf   => "otf",
            Format::Woff  => "woff",
            Format::Woff2 => "woff2",
        }
    }

    pub fn directory(self) -> &'static str {
        match self {
            Format::Ttf   => "TTF",
            Format::Otf   => "OTF",
            Format::Woff  => "WOFF",
            Format::Woff2 => "WOFF2",
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            Format::Ttf | Format::Otf => "Desktop",
            Format::Woff | Format::Woff2 => "Web",
        }
    }

    pub fn flavor(self) -> Option<&'static str> {
        match self {
            Format::Ttf | Format::Otf => None,
            Format::Woff | Format::Woff2 => Some(self.extension()),
        }
    }

    pub fn outlines(self) -> &'static str {
        if self == Format::Otf { "cff" } else { "glyf" }
    }

    pub fn all() -> [Format; 4] {
        [Format::Ttf, Format::Otf, Format::Woff, Format::Woff2]
    }

    pub fn parse(name: &str) -> Option<Format> {
        match name.to_lowercase().as_str() {
            "ttf"   => Some(Format::Ttf),
            "otf"   => Some(Format::Otf),
            "woff"  => Some(Format::Woff),
            "woff2" => Some(Format::Woff2),
            _       => None,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Style {
    pub weight: Option<Weight>,
    pub slope: Slope,
}

impl Style {
    pub fn variable(self) -> bool {
        self.weight.is_none()
    }

    pub fn name(self) -> String {
        let weight = match self.weight {
            None => "Variable",
            Some(weight) => weight.name(),
        };
        format!("{}{}", weight, self.slope.suffix())
    }

    pub fn italic(self) -> bool {
        self.slope.italic()
    }

    pub fn value(self) -> f64 {
        self.weight.unwrap_or(Weight::Regular).value() as f64
    }

    pub fn bold(self) -> bool {
        match self.weight {
            None => false,
            Some(weight) => weight >= Weight::Bold,
        }
    }
}

#[allow(non_upper_case_globals)]
pub static downloads: Mutex<Option<HashMap<String, Vec<u8>>>> = Mutex::new(None);

pub struct Archive;

impl Archive {
    pub fn fetch(url: &str) -> Vec<u8> {
        let mut guard = downloads.lock().unwrap();
        let cache = guard.get_or_insert_with(HashMap::new);
        if !cache.contains_key(url) {
            let response = ureq::get(url)
                .set("User-Agent", user_agent)
                .call()
                .unwrap_or_else(|error| panic!("failed to fetch {}: {}", url, error));
            let mut data = Vec::new();
            response.into_reader().read_to_end(&mut data)
                .unwrap_or_else(|error| panic!("failed to read {}: {}", url, error));
            cache.insert(url.to_string(), data);
        }
        cache[url].clone()
    }

    pub fn read(url: &str, member: Option<&str>) -> Vec<u8> {
        let data = Archive::fetch(url);
        let member = match member {
            None => return data,
            Some(member) => member,
        };
        if url.ends_with(".zip") {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))
                .unwrap_or_else(|error| panic!("failed to open {}: {}", url, error));
            let mut file = archive.by_name(member)
                .unwrap_or_else(|error| panic!("{} not found in {}: {}", member, url, error));
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .unwrap_or_else(|error| panic!("failed to read {} from {}: {}", member, url, error));
            return contents;
        }
        if [".tar.gz", ".tgz", ".tar.xz", ".txz", ".tar.bz2", ".tar"].iter().any(|extension| url.ends_with(extension)) {
            let reader = std::io::Cursor::new(data);
            let decoder: Box<dyn Read> = if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
                Box::new(flate2::read::GzDecoder::new(reader))
            } else if url.ends_with(".tar.xz") || url.ends_with(".txz") {
                Box::new(liblzma::read::XzDecoder::new(reader))
            } else if url.ends_with(".tar") {
                Box::new(reader)
            } else {
                panic!("unsupported archive: {}", url);
            };
            let mut archive = tar::Archive::new(decoder);
            for entry in archive.entries().unwrap_or_else(|error| panic!("failed to open {}: {}", url, error)) {
                let mut entry = entry.unwrap_or_else(|error| panic!("failed to read {}: {}", url, error));
                if entry.path().map(|path| path == Path::new(member)).unwrap_or(false) {
                    let mut contents = Vec::new();
                    entry.read_to_end(&mut contents)
                        .unwrap_or_else(|error| panic!("failed to read {} from {}: {}", member, url, error));
                    return contents;
                }
            }
            panic!("{} not found in {}", member, url);
        }
        panic!("unsupported archive: {}", url);
    }

    pub fn forget() {
        if let Some(cache) = downloads.lock().unwrap().as_mut() {
            cache.clear();
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct License {
    pub name: &'static str,
    pub url: &'static str,
    pub filepath: &'static str,
    pub filename: &'static str,
}

impl License {
    pub fn read(&self) -> Vec<u8> {
        std::fs::read(self.filepath)
            .unwrap_or_else(|error| panic!("failed to read {}: {}", self.filepath, error))
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Source {
    pub path: String,
    pub url: String,
    pub member: Option<String>,
    pub slope: Slope,
    pub weight: Option<Weight>,
}

impl Source {
    pub fn variable(&self) -> bool {
        self.weight.is_none()
    }

    pub fn filename(&self) -> String {
        Path::new(&self.path).file_name().unwrap().to_string_lossy().into_owned()
    }

    pub fn present(&self) -> bool {
        Path::new(&self.path).exists()
    }

    pub fn download(&self) -> bool {
        if self.present() {
            return false;
        }
        if let Some(parent) = Path::new(&self.path).parent() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|error| panic!("failed to create {}: {}", parent.display(), error));
        }
        let data = Archive::read(&self.url, self.member.as_deref());
        std::fs::write(&self.path, data)
            .unwrap_or_else(|error| panic!("failed to write {}: {}", self.path, error));
        true
    }

    pub fn read(&self) -> Vec<u8> {
        std::fs::read(&self.path)
            .unwrap_or_else(|error| panic!("failed to read {}: {}", self.path, error))
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Typeface {
    pub name: String,
    pub sources: Vec<Source>,
    pub prefix: String,
}

impl Typeface {
    pub fn variable(&self) -> bool {
        self.sources.iter().all(|source| source.variable())
    }

    pub fn slopes(&self) -> Vec<Slope> {
        Slope::all().into_iter()
            .filter(|slope| self.sources.iter().any(|source| source.slope == *slope))
            .collect()
    }

    pub fn source(&self, slope: Slope, weight: Option<Weight>) -> &Source {
        let mut candidates: Vec<&Source> = self.sources.iter().filter(|source| source.slope == slope).collect();
        if candidates.is_empty() {
            candidates = self.sources.iter().filter(|source| source.slope == Slope::Upright).collect();
        }

        if let Some(variable) = candidates.iter().find(|source| source.variable()) {
            return variable;
        }

        let weight = weight.unwrap_or(Weight::Regular);
        candidates.into_iter()
            .min_by_key(|source| (source.weight.unwrap().value() as i32 - weight.value() as i32).abs())
            .unwrap()
    }

    pub fn weights(&self) -> Vec<Weight> {
        if self.variable() {
            return Weight::all().to_vec();
        }
        let mut weights: Vec<Weight> = self.sources.iter().filter_map(|source| source.weight).collect();
        weights.sort();
        weights.dedup();
        weights
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Family {
    pub name: String,
    pub filename: String,
    pub license: License,

    pub latin: Typeface,
    pub cjk: Vec<Typeface>,
    pub symbols: Option<Typeface>,

    pub typeface: String,
    pub region: String,
    pub monospace: bool,
}

impl Family {
    pub fn typefaces(&self) -> Vec<&Typeface> {
        let mut typefaces = vec![&self.latin];
        typefaces.extend(self.cjk.iter());
        typefaces.extend(self.symbols.iter());
        typefaces
    }

    pub fn sources(&self) -> Vec<&Source> {
        self.typefaces().into_iter().flat_map(|typeface| typeface.sources.iter()).collect()
    }

    pub fn styles(&self) -> Vec<Style> {
        let mut styles = Vec::new();
        for slope in Slope::all() {
            for weight in [None, Some(Weight::Regular), Some(Weight::Bold)] {
                styles.push(Style { weight, slope });
            }
        }
        styles
    }

    pub fn credits(&self) -> Vec<String> {
        let mut names = vec![self.latin.name.clone()];

        if self.cjk.len() > 1 {
            let mut prefix = self.cjk[0].name.clone();
            for typeface in &self.cjk[1..] {
                let common = prefix.chars().zip(typeface.name.chars())
                    .take_while(|(a, b)| a == b)
                    .map(|(a, _)| a)
                    .collect::<String>();
                prefix = common;
            }
            prefix = match prefix.rfind(' ') {
                None => String::new(),
                Some(index) => prefix[..index + 1].to_string(),
            };
            let suffixes = self.cjk.iter()
                .map(|typeface| typeface.name[prefix.len()..].to_string())
                .collect::<Vec<String>>();
            names.push(format!("{}{}", prefix, suffixes.join("/")));
        } else {
            names.extend(self.cjk.iter().map(|typeface| typeface.name.clone()));
        }

        if let Some(symbols) = &self.symbols {
            names.push(symbols.name.clone());
        }

        names
    }

    pub fn description(&self) -> String {
        let credits = self.credits();
        let joined = if credits.len() == 1 {
            credits[0].clone()
        } else {
            format!("{} and {}", credits[..credits.len() - 1].join(", "), credits[credits.len() - 1])
        };
        format!("{} is a composite font created by Nercone, combining {}.", self.name, joined)
    }
}
