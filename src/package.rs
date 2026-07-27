use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

use rayon::prelude::*;

use crate::models::{Format, License, Family};
use crate::constants::Paths;

pub struct Archives;

#[allow(non_upper_case_globals)]
impl Archives {
    pub const zip: &'static str = "zip";
    pub const sevenzip: &'static str = "7z";
    pub const gzip: &'static str = "tar.gz";
    pub const xz: &'static str = "tar.xz";

    pub fn all() -> [&'static str; 4] {
        [Archives::zip, Archives::sevenzip, Archives::gzip, Archives::xz]
    }

    pub fn tar(format: &str) -> &'static str {
        match format {
            Archives::gzip => "w:gz",
            Archives::xz => "w:xz",
            _ => panic!("unsupported tar format: {}", format),
        }
    }
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

    pub fn package(&self, formats: Option<&[&str]>) -> Vec<String> {
        std::fs::create_dir_all(&self.directory)
            .unwrap_or_else(|error| panic!("failed to create {}: {}", self.directory, error));

        let contents = self.contents();
        let absent = self.missing();
        if !absent.is_empty() {
            panic!("{} is not built yet: {} missing", self.name, absent.len());
        }

        let all = Archives::all();
        formats.unwrap_or(&all).par_iter().map(|format| {
            let path = Path::new(&self.directory)
                .join(format!("{}.{}", self.name, format))
                .to_string_lossy()
                .into_owned();
            if *format == Archives::zip {
                self.compress(&path, &contents);
            } else if *format == Archives::sevenzip {
                self.collect(&path, &contents);
            } else {
                self.archive(&path, &contents, Archives::tar(format));
            }
            self.note(&format!("packaged {}", path));
            path
        }).collect()
    }

    pub fn compress(&self, path: &str, contents: &BTreeMap<String, String>) {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(9));
        let file = File::create(path)
            .unwrap_or_else(|error| panic!("failed to create {}: {}", path, error));
        let mut archive = zip::ZipWriter::new(file);
        for (name, source) in contents {
            archive.start_file(format!("{}/{}", self.name, name), options)
                .unwrap_or_else(|error| panic!("failed to add {} to {}: {}", name, path, error));
            let mut reader = File::open(source)
                .unwrap_or_else(|error| panic!("failed to open {}: {}", source, error));
            std::io::copy(&mut reader, &mut archive)
                .unwrap_or_else(|error| panic!("failed to add {} to {}: {}", name, path, error));
        }
        archive.finish()
            .unwrap_or_else(|error| panic!("failed to write {}: {}", path, error));
    }

    pub fn archive(&self, path: &str, contents: &BTreeMap<String, String>, mode: &str) {
        let file = File::create(path)
            .unwrap_or_else(|error| panic!("failed to create {}: {}", path, error));
        match mode {
            "w:gz" => {
                let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::new(9));
                let mut builder = tar::Builder::new(encoder);
                for (name, source) in contents {
                    builder.append_path_with_name(source, format!("{}/{}", self.name, name))
                        .unwrap_or_else(|error| panic!("failed to add {} to {}: {}", name, path, error));
                }
                builder.into_inner()
                    .and_then(|encoder| encoder.finish())
                    .unwrap_or_else(|error| panic!("failed to write {}: {}", path, error));
            }
            "w:xz" => {
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
            _ => panic!("unsupported tar mode: {}", mode),
        }
    }

    pub fn collect(&self, path: &str, contents: &BTreeMap<String, String>) {
        if Path::new(path).exists() {
            std::fs::remove_file(path)
                .unwrap_or_else(|error| panic!("failed to remove {}: {}", path, error));
        }

        let mut archive = sevenz_rust2::ArchiveWriter::create(path)
            .unwrap_or_else(|error| panic!("failed to create {}: {}", path, error));
        for (name, source) in contents {
            let entry = sevenz_rust2::ArchiveEntry::new_file(&format!("{}/{}", self.name, name));
            let reader = File::open(source)
                .unwrap_or_else(|error| panic!("failed to open {}: {}", source, error));
            archive.push_archive_entry(entry, Some(reader))
                .unwrap_or_else(|error| panic!("failed to add {} to {}: {}", name, path, error));
        }
        archive.finish()
            .unwrap_or_else(|error| panic!("failed to write {}: {}", path, error));
    }
}
