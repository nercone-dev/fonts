use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

use crate::models::{Format, License, Family};
use crate::constants::Paths;

pub struct Archives;

#[allow(non_upper_case_globals)]
impl Archives {
    pub const xz: &'static str = "tar.xz";
}

pub struct Packager {
    pub name: String,
    pub families: Vec<Family>,
    pub license: License,
    pub source: String,
    pub directory: String,
}

impl Packager {
    pub fn new(name: String, families: Vec<Family>, license: License) -> Packager {
        Packager {
            name,
            families,
            license,
            source: Paths::files.to_string(),
            directory: Paths::dist.to_string(),
        }
    }

    pub fn family(family: Family) -> Packager {
        let (name, license) = (family.filename.clone(), family.license);
        Packager::new(name, vec![family], license)
    }

    pub fn collection(name: String, families: Vec<Family>) -> Packager {
        let license = families[0].license;
        Packager::new(name, families, license)
    }

    pub fn note(&self, message: &str) {
        println!("{}", message);
    }

    pub fn contents(&self) -> BTreeMap<String, String> {
        let mut entries = BTreeMap::new();
        entries.insert(self.license.filename.to_string(), self.license.filepath.to_string());

        for family in &self.families {
            for style in family.styles() {
                for format in Format::all() {
                    let filename = format!("{}-{}.{}", family.filename, style.name(), format.extension());
                    let mut folder = format!("{}/{}", format.group(), format.directory());
                    if !style.variable() {
                        folder += "/Static";
                    }
                    entries.insert(format!("{}/{}", folder, filename),
                                   Path::new(&self.source).join(&filename).to_string_lossy().into_owned());
                }
            }
        }

        entries
    }

    pub fn missing(&self) -> Vec<String> {
        self.contents().values()
            .filter(|path| !Path::new(path).exists())
            .cloned()
            .collect()
    }

    pub fn package(&self) -> String {
        std::fs::create_dir_all(&self.directory)
            .unwrap_or_else(|error| panic!("failed to create {}: {}", self.directory, error));

        let contents = self.contents();
        let absent = self.missing();
        if !absent.is_empty() {
            panic!("{} is not built yet: {} missing", self.name, absent.len());
        }

        let path = Path::new(&self.directory)
            .join(format!("{}.{}", self.name, Archives::xz))
            .to_string_lossy()
            .into_owned();
        self.archive(&path, &contents);
        self.note(&format!("packaged {}", path));
        path
    }

    pub fn archive(&self, path: &str, contents: &BTreeMap<String, String>) {
        let file = File::create(path)
            .unwrap_or_else(|error| panic!("failed to create {}: {}", path, error));
        let encoder = liblzma::write::XzEncoder::new(file, 9);
        let mut builder = tar::Builder::new(encoder);
        for (name, source) in contents {
            builder.append_path_with_name(source, format!("{}/{}", self.name, name))
                .unwrap_or_else(|error| panic!("failed to add {} to {}: {}", name, path, error));
        }
        builder.into_inner()
            .and_then(|encoder| encoder.finish())
            .unwrap_or_else(|error| panic!("failed to write {}: {}", path, error));
    }
}
