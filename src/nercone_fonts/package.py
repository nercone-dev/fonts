import os
import shutil
import tarfile
import zipfile
import subprocess
import tempfile
from typing import Optional, List, Dict

from .models import Format, License, Family
from .constants import Paths

class Archives:
    zip = "zip"
    sevenzip = "7z"
    gzip = "tar.gz"
    xz = "tar.xz"

    all = [zip, sevenzip, gzip, xz]

    tar = {gzip: "w:gz", xz: "w:xz"}

class Packager:
    def __init__(self, name: str, families: List[Family], license: License, source: str = Paths.files, directory: str = Paths.dist, logger=None):
        self.name = name
        self.families = list(families)
        self.license = license
        self.source = source
        self.directory = directory
        self.logger = logger

    def note(self, message: str):
        if self.logger:
            self.logger.info(message)

    def contents(self) -> Dict[str, str]:
        entries = {self.license.filename: self.license.filepath}

        for family in self.families:
            for style in family.styles():
                for format in Format:
                    filename = "{}-{}.{}".format(family.filename, style.name(), format.extension())
                    folder = "{}/{}".format(format.group(), format.directory())
                    if not style.variable():
                        folder += "/Static"
                    entries["{}/{}".format(folder, filename)] = os.path.join(self.source, filename)

        return entries

    def missing(self) -> List[str]:
        return [path for path in self.contents().values() if not os.path.exists(path)]

    def package(self, formats: Optional[List[str]] = None) -> List[str]:
        os.makedirs(self.directory, exist_ok=True)

        contents = self.contents()
        absent = self.missing()
        if absent:
            raise FileNotFoundError("{} is not built yet: {} missing".format(self.name, len(absent)))

        written = []
        for format in (formats or Archives.all):
            path = os.path.join(self.directory, "{}.{}".format(self.name, format))
            if format == Archives.zip:
                self.compress(path, contents)
            elif format == Archives.sevenzip:
                self.collect(path, contents)
            else:
                self.archive(path, contents, Archives.tar[format])
            written.append(path)
            self.note("packaged {}".format(path))

        return written

    def compress(self, path: str, contents: Dict[str, str]):
        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for name, source in sorted(contents.items()):
                archive.write(source, "{}/{}".format(self.name, name))

    def archive(self, path: str, contents: Dict[str, str], mode: str):
        with tarfile.open(path, mode) as archive:
            for name, source in sorted(contents.items()):
                archive.add(source, "{}/{}".format(self.name, name))

    def collect(self, path: str, contents: Dict[str, str]):
        if os.path.exists(path):
            os.remove(path)

        try:
            import py7zr
        except ImportError:
            py7zr = None

        if py7zr is not None:
            with py7zr.SevenZipFile(path, "w") as archive:
                for name, source in sorted(contents.items()):
                    archive.write(source, "{}/{}".format(self.name, name))
            return

        binary = shutil.which("7z") or shutil.which("7zz") or shutil.which("7za")
        if binary is None:
            raise RuntimeError("7z archives need either the py7zr package or the 7z command")

        with tempfile.TemporaryDirectory() as staging:
            for name, source in contents.items():
                target = os.path.join(staging, self.name, name)
                os.makedirs(os.path.dirname(target), exist_ok=True)
                shutil.copyfile(source, target)
            subprocess.run([binary, "a", "-mx=9", "-bso0", "-bsp0", os.path.abspath(path), self.name], cwd=staging, check=True)
