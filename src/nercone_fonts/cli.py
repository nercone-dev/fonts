import os
import sys
import argparse
from typing import Optional, List

from modern import Logger, LogLevel

from .models import Format, Family
from .constants import Paths, version, Families
from .build import Builder
from .package import Archives, Packager

logger = Logger("nercone-fonts")

def families(names: Optional[List[str]]) -> List[Family]:
    if not names:
        return Families.All

    chosen = []
    for name in names:
        wanted = name.replace(" ", "").lower()
        matches = [family for family in Families.All if family.filename.lower() == wanted or family.name.replace(" ", "").lower() == wanted]
        if not matches:
            raise SystemExit("unknown family: {}".format(name))
        chosen += matches

    return chosen

def formats(names: Optional[List[str]]) -> List[Format]:
    if not names:
        return list(Format)
    return [Format[name.upper()] for name in names]

def cmd_download(arguments) -> int:
    sources = {source.path: source for family in families(arguments.families) for source in family.sources()}

    for path, source in sorted(sources.items()):
        if source.download():
            logger.info("downloaded {}".format(path))
        else:
            logger.debug("already present: {}".format(path))

    logger.info("{} source fonts in {}".format(len(sources), Paths.sources))
    return 0

def cmd_build(arguments) -> int:
    chosen = families(arguments.families)

    for family in chosen:
        missing = [source.path for source in family.sources() if not source.present()]
        if missing:
            raise SystemExit("{} needs sources that are missing; run `nercone-fonts download` first: {}".format(
                family.name, ", ".join(missing)))

    for family in chosen:
        written = Builder(family, logger=logger, formats=formats(arguments.formats)).build()
        logger.info("{}: {} files".format(family.name, len(written)))

    return 0

def cmd_package(arguments) -> int:
    chosen = families(arguments.families)

    packagers = [Packager(family.filename, [family], family.license, logger=logger) for family in chosen]

    if not arguments.families:
        packagers += [Packager(name, group, group[0].license, logger=logger) for name, group in Families.Collections.items()]

    for packager in packagers:
        packager.package(arguments.archives or Archives.all)

    return 0

def cmd_all(arguments) -> int:
    return cmd_download(arguments) or cmd_build(arguments) or cmd_package(arguments)

def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(prog="nercone-fonts", description="Builds the Nercone composite font families.")
    parser.add_argument("--version", action="version", version=version)
    parser.add_argument("--verbose", action="store_true", help="log every step, including skipped work")

    commands = parser.add_subparsers(dest="command", required=True)

    for name, function, help in (("download", cmd_download, "fetch the source fonts"), ("build", cmd_build, "merge the source fonts into build/files"), ("package", cmd_package, "write dist/ archives from build/files"), ("all", cmd_all, "download, build and package")):
        command = commands.add_parser(name, help=help)
        command.add_argument("families", nargs="*", help="families to act on, all of them by default")
        command.set_defaults(function=function)

        if name in ("build", "all"):
            command.add_argument("--formats", nargs="+", choices=[format.name.lower() for format in Format], help="file formats to write, all of them by default")
        if name in ("package", "all"):
            command.add_argument("--archives", nargs="+", choices=Archives.all, help="archive formats to write, all of them by default")

    arguments = parser.parse_args(argv)
    if arguments.verbose:
        logger.display_level = LogLevel.DEBUG

    for name in ("formats", "archives"):
        if not hasattr(arguments, name):
            setattr(arguments, name, None)

    return arguments.function(arguments)

if __name__ == "__main__":
    sys.exit(main())
