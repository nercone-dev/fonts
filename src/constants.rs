use crate::models::{Weight, Slope, License, Source, Typeface, Family};

#[allow(non_upper_case_globals)]
pub const version: &str = "4.0";
#[allow(non_upper_case_globals)]
pub const vendor: &str = "Nercone";

pub struct Paths;

#[allow(non_upper_case_globals)]
impl Paths {
    pub const build:    &'static str = "build";
    pub const sources:  &'static str = "build/sources";
    pub const files:    &'static str = "build/files";
    pub const dist:     &'static str = "dist";
    pub const licenses: &'static str = "licenses";
}

pub struct Licenses;

impl Licenses {
    pub const SIL_OFL_1_1: License = License {
        name: "SIL Open Font License, Version 1.1",
        url: "https://openfontlicense.org",
        filepath: "licenses/OFL.txt",
        filename: "OFL.txt",
    };
}

pub struct URLs;

#[allow(non_upper_case_globals)]
impl URLs {
    pub const inter:      &'static str = "https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip";
    pub const meslo:      &'static str = "https://github.com/andreberg/Meslo-Font/raw/master/dist/v1.2.1/Meslo%20LG%20v1.2.1.zip";
    pub const charter:    &'static str = "https://practicaltypography.com/fonts/Charter%20210112.zip";
    pub const noto:       &'static str = "https://github.com/google/fonts/raw/main/ofl/{directory}/{name}%5Bwght%5D.ttf";
    pub const nerd_fonts: &'static str = "https://github.com/ryanoasis/nerd-fonts/releases/download/v3.4.0/NerdFontsSymbolsOnly.zip";
}

pub fn noto(style: &str, region: &str) -> Typeface {
    let filename = format!("Noto{}{}", style, region);
    Typeface {
        name: format!("Noto {} {}", style, region),
        sources: vec![Source {
            path: format!("build/sources/noto/{}.ttf", filename),
            url: URLs::noto.replace("{directory}", &filename.to_lowercase()).replace("{name}", &filename),
            member: None,
            slope: Slope::Upright,
            weight: None,
        }],
        prefix: format!("{}.", region.to_lowercase()),
    }
}

pub struct Sources;

impl Sources {
    pub fn inter() -> Typeface {
        Typeface {
            name: "Inter".to_string(),
            sources: vec![
                Source { path: "build/sources/inter/InterVariable.ttf".to_string(),        url: URLs::inter.to_string(), member: Some("InterVariable.ttf".to_string()),        slope: Slope::Upright, weight: None },
                Source { path: "build/sources/inter/InterVariable-Italic.ttf".to_string(), url: URLs::inter.to_string(), member: Some("InterVariable-Italic.ttf".to_string()), slope: Slope::Italic,  weight: None },
            ],
            prefix: String::new(),
        }
    }

    pub fn meslo() -> Typeface {
        Typeface {
            name: "Meslo".to_string(),
            sources: vec![
                Source { path: "build/sources/meslo/MesloLGS-Regular.ttf".to_string(),    url: URLs::meslo.to_string(), member: Some("Meslo LG v1.2.1/MesloLGS-Regular.ttf".to_string()),    slope: Slope::Upright, weight: Some(Weight::Regular) },
                Source { path: "build/sources/meslo/MesloLGS-Bold.ttf".to_string(),       url: URLs::meslo.to_string(), member: Some("Meslo LG v1.2.1/MesloLGS-Bold.ttf".to_string()),       slope: Slope::Upright, weight: Some(Weight::Bold) },
                Source { path: "build/sources/meslo/MesloLGS-Italic.ttf".to_string(),     url: URLs::meslo.to_string(), member: Some("Meslo LG v1.2.1/MesloLGS-Italic.ttf".to_string()),     slope: Slope::Italic,  weight: Some(Weight::Regular) },
                Source { path: "build/sources/meslo/MesloLGS-BoldItalic.ttf".to_string(), url: URLs::meslo.to_string(), member: Some("Meslo LG v1.2.1/MesloLGS-BoldItalic.ttf".to_string()), slope: Slope::Italic,  weight: Some(Weight::Bold) },
            ],
            prefix: String::new(),
        }
    }

    pub fn charter() -> Typeface {
        Typeface {
            name: "Charter".to_string(),
            sources: vec![
                Source { path: "build/sources/charter/Charter Regular.ttf".to_string(),     url: URLs::charter.to_string(), member: Some("Charter 210112/TTF format (best for Windows)/Charter/Charter Regular.ttf".to_string()),     slope: Slope::Upright, weight: Some(Weight::Regular) },
                Source { path: "build/sources/charter/Charter Bold.ttf".to_string(),        url: URLs::charter.to_string(), member: Some("Charter 210112/TTF format (best for Windows)/Charter/Charter Bold.ttf".to_string()),        slope: Slope::Upright, weight: Some(Weight::Bold) },
                Source { path: "build/sources/charter/Charter Italic.ttf".to_string(),      url: URLs::charter.to_string(), member: Some("Charter 210112/TTF format (best for Windows)/Charter/Charter Italic.ttf".to_string()),      slope: Slope::Italic,  weight: Some(Weight::Regular) },
                Source { path: "build/sources/charter/Charter Bold Italic.ttf".to_string(), url: URLs::charter.to_string(), member: Some("Charter 210112/TTF format (best for Windows)/Charter/Charter Bold Italic.ttf".to_string()), slope: Slope::Italic,  weight: Some(Weight::Bold) },
            ],
            prefix: String::new(),
        }
    }

    pub fn nerd_fonts(monospace: bool) -> Typeface {
        let filename = if monospace { "SymbolsNerdFontMono-Regular.ttf" } else { "SymbolsNerdFont-Regular.ttf" };
        Typeface {
            name: "Nerd Fonts".to_string(),
            sources: vec![
                Source { path: format!("build/sources/nerd-fonts/{}", filename), url: URLs::nerd_fonts.to_string(), member: Some(filename.to_string()), slope: Slope::Upright, weight: None },
            ],
            prefix: "nf.".to_string(),
        }
    }

    pub fn noto_sans_jp() -> Typeface { noto("Sans", "JP") }
    pub fn noto_sans_sc() -> Typeface { noto("Sans", "SC") }
    pub fn noto_sans_tc() -> Typeface { noto("Sans", "TC") }
    pub fn noto_sans_kr() -> Typeface { noto("Sans", "KR") }

    pub fn noto_serif_jp() -> Typeface { noto("Serif", "JP") }
    pub fn noto_serif_sc() -> Typeface { noto("Serif", "SC") }
    pub fn noto_serif_tc() -> Typeface { noto("Serif", "TC") }
    pub fn noto_serif_kr() -> Typeface { noto("Serif", "KR") }

    pub fn noto_sans() -> Vec<Typeface> {
        vec![Sources::noto_sans_jp(), Sources::noto_sans_sc(), Sources::noto_sans_tc(), Sources::noto_sans_kr()]
    }

    pub fn noto_serif() -> Vec<Typeface> {
        vec![Sources::noto_serif_jp(), Sources::noto_serif_sc(), Sources::noto_serif_tc(), Sources::noto_serif_kr()]
    }
}

#[allow(non_upper_case_globals)]
pub const regions: [&str; 5] = ["CJK", "JP", "SC", "TC", "KR"];

pub fn regional(typefaces: Vec<Typeface>, region: &str) -> Vec<Typeface> {
    if region == "CJK" {
        return typefaces;
    }
    typefaces.into_iter().filter(|typeface| typeface.name.ends_with(region)).collect()
}

pub fn sans(region: &str, nerd_fonts: bool) -> Family {
    Family {
        name: format!("Nercone Sans {}{}", region, if nerd_fonts { " NF" } else { "" }),
        filename: format!("NerconeSans{}{}", region, if nerd_fonts { "NF" } else { "" }),
        license: Licenses::SIL_OFL_1_1,
        latin: Sources::inter(),
        cjk: regional(Sources::noto_sans(), region),
        symbols: if nerd_fonts { Some(Sources::nerd_fonts(false)) } else { None },
        typeface: "Sans".to_string(),
        region: region.to_string(),
        monospace: false,
    }
}

pub fn serif(region: &str, nerd_fonts: bool) -> Family {
    Family {
        name: format!("Nercone Serif {}{}", region, if nerd_fonts { " NF" } else { "" }),
        filename: format!("NerconeSerif{}{}", region, if nerd_fonts { "NF" } else { "" }),
        license: Licenses::SIL_OFL_1_1,
        latin: Sources::charter(),
        cjk: regional(Sources::noto_serif(), region),
        symbols: if nerd_fonts { Some(Sources::nerd_fonts(false)) } else { None },
        typeface: "Serif".to_string(),
        region: region.to_string(),
        monospace: false,
    }
}

pub fn mono(region: &str, nerd_fonts: bool) -> Family {
    Family {
        name: format!("Nercone Mono {}{}", region, if nerd_fonts { " NF" } else { "" }),
        filename: format!("NerconeMono{}{}", region, if nerd_fonts { "NF" } else { "" }),
        license: Licenses::SIL_OFL_1_1,
        latin: Sources::meslo(),
        cjk: regional(Sources::noto_sans(), region),
        symbols: if nerd_fonts { Some(Sources::nerd_fonts(true)) } else { None },
        typeface: "Mono".to_string(),
        region: region.to_string(),
        monospace: true,
    }
}

pub struct Families;

impl Families {
    pub fn sans() -> Vec<Family> {
        regions.iter().map(|region| sans(region, false))
            .chain(regions.iter().map(|region| sans(region, true)))
            .collect()
    }

    pub fn serif() -> Vec<Family> {
        regions.iter().map(|region| serif(region, false))
            .chain(regions.iter().map(|region| serif(region, true)))
            .collect()
    }

    pub fn mono() -> Vec<Family> {
        regions.iter().map(|region| mono(region, false))
            .chain(regions.iter().map(|region| mono(region, true)))
            .collect()
    }

    pub fn all() -> Vec<Family> {
        let mut families = Families::sans();
        families.extend(Families::serif());
        families.extend(Families::mono());
        families
    }

    pub fn collections() -> Vec<(String, Vec<Family>)> {
        vec![
            ("NerconeSans".to_string(), Families::sans()),
            ("NerconeSerif".to_string(), Families::serif()),
            ("NerconeMono".to_string(), Families::mono()),
        ]
    }
}
