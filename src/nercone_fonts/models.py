import io
import os
import tarfile
import zipfile
import urllib.request
from enum import Enum
from functools import total_ordering
from typing import Optional, List, Dict
from dataclasses import dataclass, field

user_agent = "Mozilla/5.0 (compatible; +https://github.com/nercone-dev/fonts/)"

@total_ordering
class Weight(Enum):
    Thin       = 100
    ExtraLight = 200
    Light      = 300
    Regular    = 400
    Medium     = 500
    SemiBold   = 600
    Bold       = 700
    ExtraBold  = 800
    Black      = 900

    def __lt__(a: "Weight", b: "Weight") -> bool:
        return a.value < b.value

class Slope(Enum):
    Upright = "Upright"
    Italic  = "Italic"

    def italic(self) -> bool:
        return self is Slope.Italic

    def suffix(self) -> str:
        return "Italic" if self is Slope.Italic else ""

class Format(Enum):
    TTF   = "ttf"
    OTF   = "otf"
    WOFF  = "woff"
    WOFF2 = "woff2"

    def extension(self) -> str:
        return self.value

    def directory(self) -> str:
        return self.name

    def group(self) -> str:
        return "Desktop" if self in (Format.TTF, Format.OTF) else "Web"

    def flavor(self) -> Optional[str]:
        return None if self in (Format.TTF, Format.OTF) else self.value

    def outlines(self) -> str:
        return "cff" if self is Format.OTF else "glyf"

@dataclass(frozen=True)
class Style:
    weight: Optional[Weight] = None
    slope: Slope = Slope.Upright

    def variable(self) -> bool:
        return self.weight is None

    def name(self) -> str:
        return ("Variable" if self.variable() else self.weight.name) + self.slope.suffix()

    def italic(self) -> bool:
        return self.slope.italic()

    def value(self) -> float:
        return float(Weight.Regular.value if self.variable() else self.weight.value)

    def bold(self) -> bool:
        return (not self.variable()) and self.weight >= Weight.Bold

class Archive:
    downloads: Dict[str, bytes] = {}

    @staticmethod
    def fetch(url: str) -> bytes:
        if url not in Archive.downloads:
            request = urllib.request.Request(url, headers={"User-Agent": user_agent})
            with urllib.request.urlopen(request) as response:
                Archive.downloads[url] = response.read()
        return Archive.downloads[url]

    @staticmethod
    def read(url: str, member: Optional[str] = None) -> bytes:
        data = Archive.fetch(url)
        if member is None:
            return data
        if url.endswith(".zip"):
            with zipfile.ZipFile(io.BytesIO(data)) as archive:
                return archive.read(member)
        if url.endswith((".tar.gz", ".tgz", ".tar.xz", ".txz", ".tar.bz2", ".tar")):
            with tarfile.open(fileobj=io.BytesIO(data)) as archive:
                extracted = archive.extractfile(member)
                if extracted is None:
                    raise KeyError(member)
                return extracted.read()
        raise ValueError("unsupported archive: {}".format(url))

    @staticmethod
    def forget():
        Archive.downloads.clear()

@dataclass
class License:
    name: str
    url: str
    filepath: str
    filename: str = "LICENSE"

    def read(self) -> bytes:
        with open(self.filepath, "rb") as f:
            return f.read()

@dataclass
class Source:
    path: str
    url: str
    member: Optional[str] = None
    slope: Slope = Slope.Upright
    weight: Optional[Weight] = None

    def variable(self) -> bool:
        return self.weight is None

    def filename(self) -> str:
        return os.path.basename(self.path)

    def present(self) -> bool:
        return os.path.exists(self.path)

    def download(self) -> bool:
        if self.present():
            return False
        os.makedirs(os.path.dirname(self.path), exist_ok=True)
        data = Archive.read(self.url, self.member)
        with open(self.path, "wb") as f:
            f.write(data)
        return True

    def read(self) -> bytes:
        with open(self.path, "rb") as f:
            return f.read()

@dataclass
class Typeface:
    name: str
    sources: List[Source] = field(default_factory=lambda: [])
    prefix: str = ""

    def variable(self) -> bool:
        return all(source.variable() for source in self.sources)

    def slopes(self) -> List[Slope]:
        return [slope for slope in Slope if any(source.slope is slope for source in self.sources)]

    def source(self, slope: Slope, weight: Optional[Weight] = None) -> Source:
        candidates = [source for source in self.sources if source.slope is slope]
        if not candidates:
            candidates = [source for source in self.sources if source.slope is Slope.Upright]

        variable = [source for source in candidates if source.variable()]
        if variable:
            return variable[0]

        if weight is None:
            weight = Weight.Regular
        return min(candidates, key=lambda source: abs(source.weight.value - weight.value))

    def weights(self) -> List[Weight]:
        if self.variable():
            return list(Weight)
        return sorted({source.weight for source in self.sources})

@dataclass
class Family:
    name: str
    filename: str
    license: License

    latin: Typeface
    cjk: List[Typeface] = field(default_factory=lambda: [])
    symbols: Optional[Typeface] = None

    typeface: str = "Sans"
    region: str = "CJK"
    monospace: bool = False

    def typefaces(self) -> List[Typeface]:
        return [self.latin] + list(self.cjk) + ([self.symbols] if self.symbols else [])

    def sources(self) -> List[Source]:
        return [source for typeface in self.typefaces() for source in typeface.sources]

    def styles(self) -> List[Style]:
        return [Style(weight, slope)
                for slope in Slope
                for weight in (None, Weight.Regular, Weight.Bold)]

    def credits(self) -> List[str]:
        names = [self.latin.name]

        if len(self.cjk) > 1:
            prefix = os.path.commonprefix([typeface.name for typeface in self.cjk])
            prefix = prefix[:prefix.rfind(" ") + 1]
            names.append(prefix + "/".join(typeface.name[len(prefix):] for typeface in self.cjk))
        else:
            names.extend(typeface.name for typeface in self.cjk)

        if self.symbols:
            names.append(self.symbols.name)

        return names

    def description(self) -> str:
        credits = self.credits()
        joined = credits[0] if len(credits) == 1 else "{} and {}".format(", ".join(credits[:-1]), credits[-1])
        return "{} is a composite font created by Nercone, combining {}.".format(self.name, joined)
