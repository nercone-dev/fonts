from .models import Weight, Slope, License, Source, Typeface, Family

version = "2.0"
vendor = "Nercone"

class Paths:
    build    = "build"
    sources  = "build/sources"
    files    = "build/files"
    dist     = "dist"
    licenses = "licenses"

class Licenses:
    SIL_OFL_1_1 = License("SIL Open Font License, Version 1.1", "https://openfontlicense.org", filepath="licenses/OFL.txt", filename="OFL.txt")

class URLs:
    inter      = "https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip"
    meslo      = "https://github.com/andreberg/Meslo-Font/raw/master/dist/v1.2.1/Meslo%20LG%20v1.2.1.zip"
    charter    = "https://practicaltypography.com/fonts/Charter%20210112.zip"
    noto       = "https://github.com/google/fonts/raw/main/ofl/{directory}/{name}%5Bwght%5D.ttf"
    nerd_fonts = "https://github.com/ryanoasis/nerd-fonts/releases/download/v3.4.0/NerdFontsSymbolsOnly.zip"

def noto(style: str, region: str) -> Typeface:
    filename = "Noto{}{}".format(style, region)
    return Typeface("Noto {} {}".format(style, region), [Source("build/sources/noto/{}.ttf".format(filename), URLs.noto.format(directory=filename.lower(), name=filename))], prefix=region.lower() + ".")

class Sources:
    Inter = Typeface("Inter", [
        Source("build/sources/inter/InterVariable.ttf",        URLs.inter, "InterVariable.ttf",        Slope.Upright),
        Source("build/sources/inter/InterVariable-Italic.ttf", URLs.inter, "InterVariable-Italic.ttf", Slope.Italic)
    ])

    Meslo = Typeface("Meslo", [
        Source("build/sources/meslo/MesloLGS-Regular.ttf",    URLs.meslo, "Meslo LG v1.2.1/MesloLGS-Regular.ttf",    Slope.Upright, Weight.Regular),
        Source("build/sources/meslo/MesloLGS-Bold.ttf",       URLs.meslo, "Meslo LG v1.2.1/MesloLGS-Bold.ttf",       Slope.Upright, Weight.Bold),
        Source("build/sources/meslo/MesloLGS-Italic.ttf",     URLs.meslo, "Meslo LG v1.2.1/MesloLGS-Italic.ttf",     Slope.Italic,  Weight.Regular),
        Source("build/sources/meslo/MesloLGS-BoldItalic.ttf", URLs.meslo, "Meslo LG v1.2.1/MesloLGS-BoldItalic.ttf", Slope.Italic,  Weight.Bold)
    ])

    Charter = Typeface("Charter", [
        Source("build/sources/charter/Charter Regular.ttf",     URLs.charter, "Charter 210112/TTF format (best for Windows)/Charter/Charter Regular.ttf",     Slope.Upright, Weight.Regular),
        Source("build/sources/charter/Charter Bold.ttf",        URLs.charter, "Charter 210112/TTF format (best for Windows)/Charter/Charter Bold.ttf",        Slope.Upright, Weight.Bold),
        Source("build/sources/charter/Charter Italic.ttf",      URLs.charter, "Charter 210112/TTF format (best for Windows)/Charter/Charter Italic.ttf",      Slope.Italic,  Weight.Regular),
        Source("build/sources/charter/Charter Bold Italic.ttf", URLs.charter, "Charter 210112/TTF format (best for Windows)/Charter/Charter Bold Italic.ttf", Slope.Italic,  Weight.Bold)
    ])

    NerdFonts = Typeface("Nerd Fonts", [Source("build/sources/nerd-fonts/SymbolsNerdFontMono-Regular.ttf", URLs.nerd_fonts, "SymbolsNerdFontMono-Regular.ttf")], prefix="nf.")

    NotoSansJP = noto("Sans", "JP")
    NotoSansSC = noto("Sans", "SC")
    NotoSansTC = noto("Sans", "TC")
    NotoSansKR = noto("Sans", "KR")

    NotoSerifJP = noto("Serif", "JP")
    NotoSerifSC = noto("Serif", "SC")
    NotoSerifTC = noto("Serif", "TC")
    NotoSerifKR = noto("Serif", "KR")

    NotoSans  = [NotoSansJP,  NotoSansSC,  NotoSansTC,  NotoSansKR]
    NotoSerif = [NotoSerifJP, NotoSerifSC, NotoSerifTC, NotoSerifKR]

regions = ["CJK", "JP", "SC", "TC", "KR"]

def regional(typefaces, region: str):
    if region == "CJK":
        return list(typefaces)
    return [typeface for typeface in typefaces if typeface.name.endswith(region)]

def sans(region: str) -> Family:
    return Family("Nercone Sans {}".format(region), "NerconeSans{}".format(region), Licenses.SIL_OFL_1_1, latin=Sources.Inter, cjk=regional(Sources.NotoSans, region), typeface="Sans", region=region)

def serif(region: str) -> Family:
    return Family("Nercone Serif {}".format(region), "NerconeSerif{}".format(region), Licenses.SIL_OFL_1_1, latin=Sources.Charter, cjk=regional(Sources.NotoSerif, region), typeface="Serif", region=region)

def mono(region: str, nerd_fonts: bool = False) -> Family:
    return Family("Nercone Mono {}{}".format(region, " NF" if nerd_fonts else ""), "NerconeMono{}{}".format(region, "NF" if nerd_fonts else ""), Licenses.SIL_OFL_1_1, latin=Sources.Meslo, cjk=regional(Sources.NotoSans, region), symbols=Sources.NerdFonts if nerd_fonts else None, typeface="Mono", region=region, monospace=True)

class Families:
    # Nercone Sans
    NerconeSansCJK = sans("CJK")
    NerconeSansJP  = sans("JP")
    NerconeSansSC  = sans("SC")
    NerconeSansTC  = sans("TC")
    NerconeSansKR  = sans("KR")

    # Nercone Serif
    NerconeSerifCJK = serif("CJK")
    NerconeSerifJP  = serif("JP")
    NerconeSerifSC  = serif("SC")
    NerconeSerifTC  = serif("TC")
    NerconeSerifKR  = serif("KR")

    # Nercone Mono
    NerconeMonoCJK = mono("CJK")
    NerconeMonoJP  = mono("JP")
    NerconeMonoSC  = mono("SC")
    NerconeMonoTC  = mono("TC")
    NerconeMonoKR  = mono("KR")

    # Nercone Mono NF
    NerconeMonoCJKNF = mono("CJK", nerd_fonts=True)
    NerconeMonoJPNF  = mono("JP",  nerd_fonts=True)
    NerconeMonoSCNF  = mono("SC",  nerd_fonts=True)
    NerconeMonoTCNF  = mono("TC",  nerd_fonts=True)
    NerconeMonoKRNF  = mono("KR",  nerd_fonts=True)

    Sans  = [NerconeSansCJK,  NerconeSansJP,  NerconeSansSC,  NerconeSansTC,  NerconeSansKR]
    Serif = [NerconeSerifCJK, NerconeSerifJP, NerconeSerifSC, NerconeSerifTC, NerconeSerifKR]
    Mono  = [NerconeMonoCJK,  NerconeMonoJP,  NerconeMonoSC,  NerconeMonoTC,  NerconeMonoKR, NerconeMonoCJKNF, NerconeMonoJPNF, NerconeMonoSCNF, NerconeMonoTCNF, NerconeMonoKRNF]

    All = Sans + Serif + Mono

    Collections = {"NerconeSans": Sans, "NerconeSerif": Serif, "NerconeMono": Mono}
