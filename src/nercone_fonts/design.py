import itertools
from functools import partial
from typing import Optional, Iterable, List, Dict, Tuple

from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.TupleVariation import TupleVariation
from fontTools.ttLib.tables._g_l_y_f import GlyphCoordinates
from fontTools.varLib.models import VariationModel, supportScalar
from fontTools.varLib.varStore import OnlineVarStoreBuilder

from .models import Weight

tolerance = 0.5
epsilon = 1e-9

def interpolate(value: float, pairs: List[Tuple[float, float]]) -> float:
    if value <= pairs[0][0]:
        return pairs[0][1]
    if value >= pairs[-1][0]:
        return pairs[-1][1]

    for (left, lower), (right, upper) in zip(pairs, pairs[1:]):
        if left <= value <= right:
            if right == left:
                return lower
            return lower + (upper - lower) * (value - left) / (right - left)

    return pairs[-1][1]

class Axis:
    tag = "wght"

    def __init__(self, minimum: float = 100.0, default: float = 400.0, maximum: float = 900.0):
        self.minimum = float(minimum)
        self.default = float(default)
        self.maximum = float(maximum)

    def clamp(self, value: float) -> float:
        return max(self.minimum, min(self.maximum, value))

    def normalize(self, value: float) -> float:
        value = self.clamp(value)
        if value < self.default:
            return (value - self.default) / (self.default - self.minimum) if self.default > self.minimum else 0.0
        if value > self.default:
            return (value - self.default) / (self.maximum - self.default) if self.maximum > self.default else 0.0
        return 0.0

    def denormalize(self, coordinate: float) -> float:
        coordinate = max(-1.0, min(1.0, coordinate))
        if coordinate < 0:
            return self.default + coordinate * (self.default - self.minimum)
        if coordinate > 0:
            return self.default + coordinate * (self.maximum - self.default)
        return self.default

    def weights(self) -> List[Weight]:
        return [weight for weight in Weight if self.minimum <= weight.value <= self.maximum]

    def matches(self, other: "Axis") -> bool:
        return abs(self.minimum - other.minimum) < epsilon and abs(self.default - other.default) < epsilon and abs(self.maximum - other.maximum) < epsilon

    @staticmethod
    def of(fonts: List[TTFont], default: float = 400.0) -> "Axis":
        ranges = [(entry.minValue, entry.maxValue) for font in fonts if "fvar" in font for entry in font["fvar"].axes if entry.axisTag == Axis.tag]

        if not ranges:
            return Axis(default, default, default)

        return Axis(max(minimum for minimum, _ in ranges), default, min(maximum for _, maximum in ranges))

class Space:
    tag = Axis.tag
    identity = [(-1.0, -1.0), (0.0, 0.0), (1.0, 1.0)]

    def __init__(self, axis: Axis, segments: Optional[List[Tuple[float, float]]] = None):
        self.axis = axis
        self.segments = sorted(segments) if segments else list(Space.identity)

    @staticmethod
    def read(font: TTFont) -> Optional["Space"]:
        if "fvar" not in font:
            return None

        for entry in font["fvar"].axes:
            if entry.axisTag != Space.tag:
                continue
            segments = None
            if "avar" in font and Space.tag in font["avar"].segments:
                segments = sorted(font["avar"].segments[Space.tag].items()) or None
            return Space(Axis(entry.minValue, entry.defaultValue, entry.maxValue), segments)

        return None

    def inverse(self) -> List[Tuple[float, float]]:
        return sorted((mapped, plain) for plain, mapped in self.segments)

    def normalize(self, weight: float) -> float:
        return interpolate(self.axis.normalize(weight), self.segments)

    def denormalize(self, coordinate: float) -> float:
        return self.axis.denormalize(interpolate(coordinate, self.inverse()))

    def linear(self) -> bool:
        return all(abs(plain - mapped) < epsilon for plain, mapped in self.segments)

    def matches(self, other: "Space") -> bool:
        return self.axis.matches(other.axis) and len(self.segments) == len(other.segments) and all(abs(a - c) < epsilon and abs(b - d) < epsilon for (a, b), (c, d) in zip(self.segments, other.segments))

    def breakpoints(self) -> List[float]:
        return sorted({self.axis.denormalize(plain) for plain, _ in self.segments} | {self.axis.minimum, self.axis.default, self.axis.maximum})

    def avar(self) -> Optional[object]:
        if self.linear():
            return None

        table = newTable("avar")
        table.segments = {Space.tag: {plain: mapped for plain, mapped in self.segments}}
        return table

class Mapping:
    def __init__(self, font: TTFont, space: Space):
        self.font = font
        self.space = space
        self.source = Space.read(font)

    def coordinate(self, weight: float) -> float:
        return self.source.normalize(weight)

    def weight(self, coordinate: float) -> float:
        return self.source.denormalize(coordinate)

    def settled(self) -> bool:
        return self.source is None or self.source.matches(self.space)

    def axes(self) -> List[str]:
        if "fvar" not in self.font:
            return [Space.tag]
        return [entry.axisTag for entry in self.font["fvar"].axes]

    def supports(self) -> Iterable[Tuple[float, float, float]]:
        if "gvar" in self.font:
            for variations in self.font["gvar"].variations.values():
                for variation in variations:
                    if Space.tag in variation.axes:
                        yield variation.axes[Space.tag]

        tags = self.axes()
        for store in Mapping.stores(self.font):
            for region in store.VarRegionList.Region:
                for tag, entry in zip(tags, region.VarRegionAxis):
                    if tag == Space.tag and entry.PeakCoord:
                        yield (entry.StartCoord, entry.PeakCoord, entry.EndCoord)

    def levels(self) -> Dict[str, List[float]]:
        found = {tag: {0.0} for tag in self.axes() if tag != Space.tag}
        if not found:
            return found

        if "gvar" in self.font:
            for variations in self.font["gvar"].variations.values():
                for variation in variations:
                    for tag, support in variation.axes.items():
                        if tag in found:
                            found[tag].update(support)

        tags = self.axes()
        for store in Mapping.stores(self.font):
            for region in store.VarRegionList.Region:
                for tag, entry in zip(tags, region.VarRegionAxis):
                    if tag in found and entry.PeakCoord:
                        found[tag].update((entry.StartCoord, entry.PeakCoord, entry.EndCoord))

        return {tag: sorted(values) for tag, values in found.items()}

    def breakpoints(self) -> List[float]:
        if self.source is None:
            return []

        found = set(self.source.breakpoints())
        for support in self.supports():
            found.update(self.weight(coordinate) for coordinate in support)

        return sorted(found)

    def apply(self, masters: List[float]):
        for tag in ("HVAR", "VVAR", "MVAR"):
            if tag in self.font:
                del self.font[tag]

        if self.source is not None and not self.settled():
            levels = self.levels()
            locations, coordinates = [], []
            for chosen in itertools.product(*levels.values()):
                extra = {tag: value for tag, value in zip(levels, chosen) if value}
                for weight in masters:
                    location = dict(extra)
                    if self.space.normalize(weight):
                        location[Space.tag] = self.space.normalize(weight)
                    locations.append(location)

                    coordinate = dict(extra)
                    coordinate[Space.tag] = self.coordinate(weight)
                    coordinates.append(coordinate)

            model = VariationModel(locations, axisOrder=self.axes())

            if "gvar" in self.font:
                self.outlines(model, coordinates)
            self.deltas(model, coordinates)

        self.declare()

    def outlines(self, model: VariationModel, coordinates: List[Dict[str, float]]):
        glyf, gvar = self.font["glyf"], self.font["gvar"]
        horizontal = self.font["hmtx"].metrics
        vertical = self.font["vmtx"].metrics if "vmtx" in self.font else None
        rounding = partial(GlyphCoordinates.__round__, round=round)

        for name, variations in list(gvar.variations.items()):
            if not variations:
                continue

            points, control = glyf._getCoordinatesAndControls(name, horizontal, vertical)
            for variation in variations:
                variation.calcInferredDeltas(points, control.endPts)

            samples = []
            for coordinate in coordinates:
                total = GlyphCoordinates(points)
                for variation in variations:
                    scalar = supportScalar(coordinate, variation.axes)
                    if scalar:
                        total += GlyphCoordinates(variation.coordinates) * scalar
                samples.append(total)

            deltas = model.getDeltas(samples, round=rounding)

            rebuilt = []
            for delta, support in zip(deltas[1:], model.supports[1:]):
                if all(value == 0 for value in delta.array):
                    continue
                variation = TupleVariation(support, delta)
                variation.optimize(deltas[0], control.endPts, tolerance=tolerance)
                rebuilt.append(variation)

            gvar.variations[name] = rebuilt

    def deltas(self, model: VariationModel, coordinates: List[Dict[str, float]]):
        if "GDEF" not in self.font:
            return

        definitions = self.font["GDEF"].table
        store = getattr(definitions, "VarStore", None)
        if store is None:
            return

        builder = OnlineVarStoreBuilder(self.axes())
        builder.setModel(model)

        indices = {0xFFFFFFFF: 0xFFFFFFFF}
        for outer, data in enumerate(store.VarData):
            supports = [self.support(store.VarRegionList.Region[index]) for index in data.VarRegionIndex]
            for inner, item in enumerate(data.Item):
                values = [sum(delta * supportScalar(coordinate, support) for delta, support in zip(item, supports)) for coordinate in coordinates]
                indices[(outer << 16) | inner] = builder.storeMasters(values)[1]

        definitions.VarStore = builder.finish()
        definitions.remap_device_varidxes(indices)
        if "GPOS" in self.font:
            self.font["GPOS"].table.remap_device_varidxes(indices)

    def declare(self):
        if "fvar" not in self.font:
            return

        axis = self.space.axis
        for entry in self.font["fvar"].axes:
            if entry.axisTag == Space.tag:
                entry.minValue, entry.defaultValue, entry.maxValue = axis.minimum, axis.default, axis.maximum
        self.font["fvar"].instances[:] = []

        segments = dict(self.font["avar"].segments) if "avar" in self.font else {}
        table = self.space.avar()
        if table is None:
            segments.pop(Space.tag, None)
        else:
            segments[Space.tag] = table.segments[Space.tag]

        curved = any(any(abs(plain - mapped) > epsilon for plain, mapped in mapping.items()) for mapping in segments.values())
        if curved:
            table = newTable("avar")
            table.segments = {entry.axisTag: segments.get(entry.axisTag) or dict(Space.identity) for entry in self.font["fvar"].axes}
            self.font["avar"] = table
        elif "avar" in self.font:
            del self.font["avar"]

    def support(self, region) -> Dict[str, Tuple[float, float, float]]:
        return {tag: (entry.StartCoord, entry.PeakCoord, entry.EndCoord) for tag, entry in zip(self.axes(), region.VarRegionAxis) if entry.PeakCoord}

    @staticmethod
    def stores(font: TTFont) -> List[object]:
        found = []

        if "GDEF" in font and getattr(font["GDEF"].table, "VarStore", None):
            found.append(font["GDEF"].table.VarStore)
        for tag in ("HVAR", "VVAR", "MVAR"):
            if tag in font and getattr(font[tag].table, "VarStore", None):
                found.append(font[tag].table.VarStore)

        return found
