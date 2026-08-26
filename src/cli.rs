use std::sync::atomic::{AtomicUsize, Ordering};

use clap::{Parser, Subcommand};
use rayon::prelude::*;

use crate::build::Builder;
use crate::constants::{version, Families, Paths};
use crate::models::{Family, Format};
use crate::package::Packager;

#[allow(non_upper_case_globals)]
pub const concurrency: usize = 2;

#[derive(Parser)]
#[command(name = "nercone-fonts", about = "Builds the Nercone composite font families.", version = version)]
pub struct Arguments {
    #[arg(long, help = "log every step, including skipped work")]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "fetch the source fonts")]
    Download {
        #[arg(help = "families to act on, all of them by default")]
        families: Vec<String>,
    },
    #[command(about = "merge the source fonts into build/files")]
    Build {
        #[arg(help = "families to act on, all of them by default")]
        families: Vec<String>,
        #[arg(long, num_args = 1.., help = "file formats to write, all of them by default")]
        formats: Vec<String>,
        #[arg(long, default_value_t = concurrency, help = "families to compose at once, each costing a few gigabytes")]
        jobs: usize,
    },
    #[command(about = "write dist/ archives from build/files")]
    Package {
        #[arg(help = "families and collections to act on, all of them by default")]
        families: Vec<String>,
    },
    #[command(about = "download, build and package")]
    All {
        #[arg(help = "families to act on, all of them by default")]
        families: Vec<String>,
        #[arg(long, num_args = 1.., help = "file formats to write, all of them by default")]
        formats: Vec<String>,
        #[arg(long, default_value_t = concurrency, help = "families to compose at once, each costing a few gigabytes")]
        jobs: usize,
    },
}

pub fn families(names: &[String]) -> Vec<Family> {
    if names.is_empty() {
        return Families::all();
    }

    let mut chosen = Vec::new();
    for name in names {
        let wanted = name.replace(' ', "").to_lowercase();
        let matches: Vec<Family> = Families::all()
            .into_iter()
            .filter(|family| family.filename.to_lowercase() == wanted || family.name.replace(' ', "").to_lowercase() == wanted)
            .collect();
        if matches.is_empty() {
            eprintln!("unknown family: {}", name);
            std::process::exit(1);
        }
        chosen.extend(matches);
    }

    chosen
}

pub fn formats(names: &[String]) -> Vec<Format> {
    if names.is_empty() {
        return Format::all().to_vec();
    }
    names
        .iter()
        .map(|name| {
            Format::parse(name).unwrap_or_else(|| {
                eprintln!("unknown format: {}", name);
                std::process::exit(1);
            })
        })
        .collect()
}

pub fn download(names: &[String]) -> i32 {
    let chosen = families(names);
    let sources: Vec<(String, &crate::models::Source)> = Vec::new();
    let list: Vec<Family> = chosen;
    let mut seen = std::collections::BTreeMap::new();
    for family in &list {
        for source in family.sources() {
            seen.entry(source.path.clone()).or_insert_with(|| source.clone());
        }
    }
    drop(sources);

    for (path, source) in &seen {
        if source.download() {
            println!("downloaded {}", path);
        } else {
            println!("already present: {}", path);
        }
    }

    println!("{} source fonts in {}", seen.len(), Paths::sources);
    0
}

pub fn build(names: &[String], wanted: &[String], at_once: usize) -> i32 {
    let chosen = families(names);

    for family in &chosen {
        let missing: Vec<String> = family.sources().iter().filter(|source| !source.present()).map(|source| source.path.clone()).collect();
        if !missing.is_empty() {
            eprintln!(
                "{} needs sources that are missing; run `nercone-fonts download` first: {}",
                family.name,
                missing.join(", ")
            );
            std::process::exit(1);
        }
    }

    let wanted = formats(wanted);
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..at_once.max(1).min(chosen.len()) {
            scope.spawn(|| {
                while let Some(family) = chosen.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let written = Builder::new(family.clone(), Some(wanted.clone()), None).build();
                    println!("{}: {} files", family.name, written.len());
                }
            });
        }
    });

    0
}

pub fn packagers(names: &[String]) -> Vec<Packager> {
    if names.is_empty() {
        let mut packagers: Vec<Packager> = Families::all().into_iter().map(Packager::family).collect();
        packagers.extend(Families::collections().into_iter().map(|(name, group)| Packager::collection(name, group)));
        return packagers;
    }

    let mut packagers = Vec::new();
    for name in names {
        match Families::collection(name) {
            Some((collection, group)) => packagers.push(Packager::collection(collection, group)),
            None => packagers.extend(families(std::slice::from_ref(name)).into_iter().map(Packager::family)),
        }
    }

    packagers
}

pub fn package(names: &[String]) -> i32 {
    packagers(names).par_iter().for_each(|packager| {
        packager.package();
    });

    0
}

pub fn main() -> i32 {
    let arguments = Arguments::parse();

    match &arguments.command {
        Command::Download { families } => download(families),
        Command::Build { families, formats, jobs } => build(families, formats, *jobs),
        Command::Package { families } => package(families),
        Command::All { families, formats, jobs } => {
            let code = download(families);
            if code != 0 {
                return code;
            }
            let code = build(families, formats, *jobs);
            if code != 0 {
                return code;
            }
            package(families)
        }
    }
}
