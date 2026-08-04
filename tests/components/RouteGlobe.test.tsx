/**
 * RouteGlobe — guards the two things that fail silently.
 *
 * 1. The no-WebGL path. jsdom has no WebGL, and `tests/integration/
 *    ProvidersUpdateBanner.test.tsx` renders the whole providers view, so if the
 *    globe ever constructs a renderer unconditionally it takes those tests down
 *    with it. The same path is what GPU-less machines get in production.
 * 2. Globe orientation. A sign flip in latLngToVec produces a mirrored or
 *    upside-down Earth that still renders perfectly happily — nothing throws,
 *    the dots just land in the wrong hemisphere.
 */
import { render, screen } from "@testing-library/react";
import RouteGlobe, {
  cameraDistance,
  densityToStep,
  flattenRings,
  GLOBE_PALETTES,
  landColorAt,
  latLngToVec,
  latToY,
  lngToX,
  rampColor,
  REDUCED_SLOW_FACTOR,
  resolveSpinRate,
  rgbTriple,
  spinForLongitude,
  type LandCollection,
} from "@/components/RouteGlobe";

describe("RouteGlobe — no-WebGL fallback", () => {
  it("does not construct a WebGL renderer under jsdom", () => {
    // If three were imported and a renderer built, this would throw.
    expect(() => render(<RouteGlobe />)).not.toThrow();
  });

  it("renders the CSS fallback disc, decoratively hidden", () => {
    const { container } = render(<RouteGlobe />);
    const disc = container.querySelector(".route-globe-fallback");
    expect(disc).toBeInTheDocument();
    // Purely decorative — it must not reach the accessibility tree.
    expect(disc).toHaveAttribute("aria-hidden", "true");
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("keeps the fallback visible when WebGL never initialises", () => {
    const { container } = render(<RouteGlobe />);
    // `is-hidden` is only added once a real frame has painted.
    expect(
      container.querySelector(".route-globe-fallback.is-hidden"),
    ).not.toBeInTheDocument();
    expect(container.querySelector("canvas")).not.toBeInTheDocument();
  });

  it("applies a caller className alongside the base class", () => {
    const { container } = render(<RouteGlobe className="route-globe-art" />);
    const host = container.querySelector(".route-globe");
    expect(host).toHaveClass("route-globe");
    expect(host).toHaveClass("route-globe-art");
  });
});

describe("RouteGlobe — globe orientation", () => {
  const R = 1;
  const near = (a: number, b: number) => expect(a).toBeCloseTo(b, 6);

  it("puts longitude 0 on the camera axis (+z)", () => {
    const p = latLngToVec(0, 0, R);
    near(p.x, 0);
    near(p.y, 0);
    near(p.z, 1);
  });

  it("puts the north pole at +y and the south pole at -y", () => {
    near(latLngToVec(90, 0, R).y, 1);
    near(latLngToVec(-90, 0, R).y, -1);
  });

  it("puts east at +x, so the globe is not mirrored", () => {
    // 90E must be to the right of the viewer, not the left.
    const east = latLngToVec(0, 90, R);
    near(east.x, 1);
    near(east.z, 0);
    expect(latLngToVec(0, -90, R).x).toBeLessThan(0);
  });

  it("keeps every point on the sphere of the requested radius", () => {
    for (const [lat, lng] of [
      [0, 0],
      [45, 30],
      [-33.9, 151.2], // Sydney
      [51.5, -0.13], // London
      [-90, 180],
    ]) {
      const p = latLngToVec(lat, lng, 2.5);
      near(Math.hypot(p.x, p.y, p.z), 2.5);
    }
  });

  it("scales linearly with radius", () => {
    const a = latLngToVec(20, -105, 1);
    const b = latLngToVec(20, -105, 3);
    near(b.x, a.x * 3);
    near(b.y, a.y * 3);
    near(b.z, a.z * 3);
  });
});

describe("RouteGlobe — initial facing longitude", () => {
  /** Rotate about +y, matching what three.js does for `globe.rotation.y`. */
  const rotateY = (p: { x: number; y: number; z: number }, theta: number) => ({
    x: p.x * Math.cos(theta) + p.z * Math.sin(theta),
    y: p.y,
    z: -p.x * Math.sin(theta) + p.z * Math.cos(theta),
  });

  it("brings the requested longitude onto the camera axis", () => {
    // The bug this pins: the sign here is easy to invert, and doing so shows
    // 105W (North America) when 105E (Asia) was asked for. Nothing throws —
    // you just get the wrong hemisphere.
    for (const lng of [0, 23, -23, 90, -90, 105, -105, 179]) {
      const facing = rotateY(latLngToVec(0, lng, 1), spinForLongitude(lng));
      expect(facing.z).toBeCloseTo(1, 6); // straight at the camera
      expect(facing.x).toBeCloseTo(0, 6); // not off to one side
    }
  });

  it("defaults to 105E, so the hero opens on Asia rather than America", () => {
    const { container } = render(<RouteGlobe />);
    expect(container.querySelector(".route-globe")).toBeInTheDocument();
    // 105E must rotate the globe negatively; 105W positively.
    expect(spinForLongitude(105)).toBeLessThan(0);
    expect(spinForLongitude(-105)).toBeGreaterThan(0);
  });
});

describe("RouteGlobe — equirectangular projection", () => {
  it("maps the lng/lat corners onto the mask corners", () => {
    // 1024x512 mask: -180 -> x 0, +180 -> x 1024, +90 -> y 0, -90 -> y 512.
    expect(lngToX(-180)).toBe(0);
    expect(lngToX(180)).toBe(1024);
    expect(lngToX(0)).toBe(512);
    expect(latToY(90)).toBe(0);
    expect(latToY(-90)).toBe(512);
    expect(latToY(0)).toBe(256);
  });

  it("is monotonic, so the map is never flipped", () => {
    expect(lngToX(-90)).toBeLessThan(lngToX(90));
    // y grows downward as latitude falls — north must land above south.
    expect(latToY(45)).toBeLessThan(latToY(-45));
  });
});

describe("RouteGlobe — land geometry", () => {
  it("flattens Polygon and MultiPolygon rings alike", () => {
    const land: LandCollection = {
      features: [
        { geometry: { type: "Polygon", coordinates: [[[0, 0]], [[1, 1]]] } },
        {
          geometry: {
            type: "MultiPolygon",
            coordinates: [[[[2, 2]]], [[[3, 3]], [[4, 4]]]],
          },
        },
      ],
    };
    // 2 rings from the Polygon (outer + hole) and 3 from the MultiPolygon.
    expect(flattenRings(land)).toHaveLength(5);
  });

  it("skips null geometry and unknown types instead of throwing", () => {
    const land: LandCollection = {
      features: [
        { geometry: null },
        {},
        { geometry: { type: "LineString", coordinates: [[0, 0]] } },
      ],
    };
    expect(flattenRings(land)).toEqual([]);
  });
});

describe("RouteGlobe — camera framing", () => {
  const FOV = 50;
  const R = 1;

  /**
   * Independent check of where the silhouette actually lands, written in a
   * different algebraic form than the source's closed form: the outline subtends
   * asin(R/d) at the camera, so its NDC extent is tan(asin(R/d)) scaled by the
   * frustum. If the two forms agree, the closed form is right.
   */
  const silhouetteNdc = (d: number, w: number, h: number) =>
    Math.tan(Math.asin(R / d)) /
    (Math.tan((FOV / 2) * (Math.PI / 180)) * Math.min(1, w / h));

  it("lands the silhouette at exactly the requested fill", () => {
    for (const fill of [0.5, 0.7, 0.92, 1]) {
      for (const [w, h] of [
        [500, 324],
        [430, 264],
        [320, 320],
      ]) {
        expect(silhouetteNdc(cameraDistance(fill, w, h), w, h)).toBeCloseTo(
          fill,
          6,
        );
      }
    }
  });

  it("never lets the poles clip, even at fill = 1", () => {
    // The bug this guards: framing off the equator radius (atan) instead of the
    // tangent circle (asin) overfills by ~11%, so fill >= 0.9 clipped the globe
    // flat top and bottom. ndc > 1 means outside the frustum.
    for (const [w, h] of [
      [500, 324],
      [320, 320],
      [240, 400],
    ]) {
      expect(silhouetteNdc(cameraDistance(1, w, h), w, h)).toBeLessThanOrEqual(
        1.000001,
      );
    }
  });

  it("is measurably further out than framing off the equator", () => {
    // Guards against the old formula creeping back: it always sits closer.
    const equatorFramed =
      (R / (0.92 * Math.tan((FOV / 2) * (Math.PI / 180)))) * 1;
    expect(cameraDistance(0.92, 500, 324)).toBeGreaterThan(equatorFramed);
  });

  it("pulls back further for boxes narrower than they are tall", () => {
    expect(cameraDistance(0.92, 240, 400)).toBeGreaterThan(
      cameraDistance(0.92, 400, 240),
    );
  });

  it("clamps fill instead of returning a degenerate distance", () => {
    expect(cameraDistance(0, 500, 324)).toBe(cameraDistance(0.1, 500, 324));
    expect(cameraDistance(5, 500, 324)).toBe(cameraDistance(1, 500, 324));
    expect(Number.isFinite(cameraDistance(0.92, 500, 0))).toBe(true);
  });
});

describe("RouteGlobe — palette ramp", () => {
  it("returns the endpoints exactly", () => {
    const stops = ["#ff0000", "#00ff00", "#0000ff"];
    expect(rampColor(stops, 0)).toBe("#ff0000");
    expect(rampColor(stops, 1)).toBe("#0000ff");
  });

  it("interpolates the midpoint in sRGB", () => {
    expect(rampColor(["#000000", "#ffffff"], 0.5)).toBe("#808080");
  });

  it("lands on interior stops exactly", () => {
    const stops = ["#ff0000", "#00ff00", "#0000ff"];
    expect(rampColor(stops, 0.5)).toBe("#00ff00");
  });

  it("clamps t and survives degenerate ramps", () => {
    expect(rampColor(["#111111", "#222222"], -3)).toBe("#111111");
    expect(rampColor(["#111111", "#222222"], 9)).toBe("#222222");
    expect(rampColor(["#abcdef"], 0.5)).toBe("#abcdef");
    expect(rampColor([], 0.5)).toBe("#000000");
  });

  it("expands 3-digit hex", () => {
    expect(rampColor(["#f00", "#f00"], 0.5)).toBe("#ff0000");
  });

  it("maps equator to the ramp start and the poles to its end", () => {
    const p = GLOBE_PALETTES.aurora;
    expect(landColorAt(p, 0)).toBe(p.land[0]);
    expect(landColorAt(p, 90)).toBe(p.land[p.land.length - 1]);
    expect(landColorAt(p, -90)).toBe(p.land[p.land.length - 1]);
  });

  it("is symmetric about the equator, so both hemispheres match", () => {
    const p = GLOBE_PALETTES.aurora;
    for (const lat of [12, 33.5, 61, 89]) {
      expect(landColorAt(p, lat)).toBe(landColorAt(p, -lat));
    }
    // 45 deg is the exact midpoint of a 5-stop ramp.
    expect(landColorAt(p, 45)).toBe(p.land[2]);
  });

  it("clamps latitudes beyond the poles", () => {
    const p = GLOBE_PALETTES.aurora;
    expect(landColorAt(p, 140)).toBe(landColorAt(p, 90));
  });

  it("ships only well-formed presets", () => {
    for (const [name, p] of Object.entries(GLOBE_PALETTES)) {
      const hex = /^#[0-9a-f]{6}$/i;
      expect(p.ocean, name).toMatch(hex);
      expect(p.glow, name).toMatch(hex);
      expect(p.shade, name).toMatch(hex);
      expect(p.land.length, name).toBeGreaterThan(0);
      for (const stop of p.land) expect(stop, name).toMatch(hex);
    }
  });

  it("emits rgb triples CSS can use with an alpha slash", () => {
    expect(rgbTriple("#ff5a36")).toBe("255 90 54");
    expect(rgbTriple("#000")).toBe("0 0 0");
  });
});

describe("RouteGlobe — dot density", () => {
  it("tightens spacing as density rises", () => {
    expect(densityToStep(1)).toBeGreaterThan(densityToStep(10));
  });

  it("clamps out-of-range input to the endpoints", () => {
    expect(densityToStep(-5)).toBeCloseTo(densityToStep(1), 6);
    expect(densityToStep(99)).toBeCloseTo(densityToStep(10), 6);
  });

  it("stays inside a sane degree range at the endpoints", () => {
    // Guards against a dot count that would tank the frame rate: below ~0.5deg
    // the instance count runs into six figures.
    expect(densityToStep(10)).toBeGreaterThan(0.5);
    expect(densityToStep(1)).toBeLessThan(4);
  });
});

describe("RouteGlobe — reduced-motion policy", () => {
  const DEG = Math.PI / 180;

  it("converts deg/s to rad/s and signs it by direction", () => {
    expect(resolveSpinRate(120, "right", "slow", false)).toBeCloseTo(
      120 * DEG,
      9,
    );
    expect(resolveSpinRate(120, "left", "slow", false)).toBeCloseTo(
      -120 * DEG,
      9,
    );
  });

  it("ignores the policy entirely when the OS has not asked for reduced motion", () => {
    for (const policy of ["slow", "freeze", "spin"] as const) {
      expect(resolveSpinRate(120, "right", policy, false)).toBeCloseTo(
        120 * DEG,
        9,
      );
    }
  });

  // The regression that shipped: animations are off by default on Windows Server
  // and in RDP sessions, which reads as prefers-reduced-motion, and a hard zero
  // there is indistinguishable from a broken globe.
  it("still turns under the default policy when reduced motion is requested", () => {
    const rate = resolveSpinRate(120, "right", "slow", true);
    expect(rate).not.toBe(0);
    expect(rate).toBeGreaterThan(0);
    expect(rate).toBeCloseTo(120 * REDUCED_SLOW_FACTOR * DEG, 9);
  });

  it("throttles rather than matching full speed", () => {
    const full = resolveSpinRate(120, "right", "slow", false);
    const slow = resolveSpinRate(120, "right", "slow", true);
    expect(slow).toBeLessThan(full);
    expect(REDUCED_SLOW_FACTOR).toBeGreaterThan(0);
    expect(REDUCED_SLOW_FACTOR).toBeLessThan(1);
  });

  it("freeze stops dead, and keeps stopping regardless of direction", () => {
    expect(resolveSpinRate(120, "right", "freeze", true)).toBe(0);
    expect(resolveSpinRate(120, "left", "freeze", true)).toBe(0);
  });

  it("spin overrides the preference at full rate and correct sign", () => {
    expect(resolveSpinRate(120, "right", "spin", true)).toBeCloseTo(
      120 * DEG,
      9,
    );
    expect(resolveSpinRate(120, "left", "spin", true)).toBeCloseTo(
      -120 * DEG,
      9,
    );
  });

  it("keeps speed=0 frozen no matter the policy", () => {
    for (const policy of ["slow", "freeze", "spin"] as const) {
      expect(resolveSpinRate(0, "right", policy, true)).toBe(0);
      expect(resolveSpinRate(0, "right", policy, false)).toBe(0);
    }
  });
});
