/**
 * RouteGlobe — the rotating dotted globe in the 供应商 hero.
 *
 * Adapted from the Originkit `Globe` component. Four changes matter for
 * shipping it inside a Tauri desktop app:
 *
 *   1. Land geometry is bundled (`ne-110m-land.json`, 84 KB) instead of fetched
 *      from raw.githubusercontent.com on mount. The original's fetch leaves the
 *      hero permanently blank on a firewalled or offline machine.
 *   2. `three` is imported dynamically, so it lands in its own chunk instead of
 *      blocking first paint of the providers view.
 *   3. WebGL is feature-detected, with a CSS fallback disc instead of a throw
 *      (GPU-less VMs, older WebView2, and jsdom in the test suite).
 *   4. Rotation is delta-time based, so it turns at the same rate on a 60 Hz and
 *      a 144 Hz display, and pauses while the window is hidden or off-screen.
 *
 * d3-geo is not needed: the equirectangular projection it supplied is a linear
 * lng/lat -> x/y map, inlined below as `lngToX`/`latToY`.
 *
 * ne-110m-land.json is Natural Earth 110m physical land (public domain), from
 * martynafford/natural-earth-geojson, with `properties` dropped and coordinates
 * rounded to 2dp (~1.1 km — far finer than a 300px globe resolves): 207 -> 84 KB.
 * It is listed in .prettierignore, so regenerate rather than reformat it.
 */
import { useEffect, useRef, useState, type CSSProperties } from "react";
import type { Mesh, Object3D } from "three";

type Ring = number[][];

export type LandCollection = {
  features: Array<{
    geometry?: { type: string; coordinates: unknown } | null;
  }>;
};

/** Flatten every ring of every land polygon into one list. Exported for test. */
export function flattenRings(land: LandCollection): Ring[] {
  const rings: Ring[] = [];
  for (const feature of land.features) {
    const geometry = feature.geometry;
    if (!geometry) continue;
    if (geometry.type === "Polygon") {
      for (const ring of geometry.coordinates as Ring[]) rings.push(ring);
    } else if (geometry.type === "MultiPolygon") {
      for (const polygon of geometry.coordinates as Ring[][]) {
        for (const ring of polygon) rings.push(ring);
      }
    }
  }
  return rings;
}

const MASK_W = 1024;
const MASK_H = 512;
const TAU = Math.PI * 2;
// The equirectangular projection, inlined (this is all d3-geo was doing here).
export const lngToX = (lng: number) => ((lng + 180) / 360) * MASK_W;
export const latToY = (lat: number) => ((90 - lat) / 180) * MASK_H;

let maskCache: Uint8Array | null = null;

/**
 * Rasterise land into a 1024x512 bit mask, once per session. All rings of a
 * feature go into one path so Canvas2D's nonzero winding rule carves lake holes
 * out of their enclosing polygon. 1024x512 is ~0.35 deg/px, well under the
 * ~1.2 deg dot spacing, so the mask is never the limiting factor.
 */
function getLandMask(rings: Ring[]): Uint8Array | null {
  if (maskCache) return maskCache;
  const canvas = document.createElement("canvas");
  canvas.width = MASK_W;
  canvas.height = MASK_H;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return null;
  ctx.fillStyle = "#000";
  ctx.fillRect(0, 0, MASK_W, MASK_H);
  ctx.fillStyle = "#fff";
  ctx.beginPath();
  for (const ring of rings) {
    if (ring.length < 2) continue;
    ctx.moveTo(lngToX(ring[0][0]), latToY(ring[0][1]));
    for (let i = 1; i < ring.length; i += 1) {
      ctx.lineTo(lngToX(ring[i][0]), latToY(ring[i][1]));
    }
    ctx.closePath();
  }
  ctx.fill();
  const pixels = ctx.getImageData(0, 0, MASK_W, MASK_H).data;
  const mask = new Uint8Array(MASK_W * MASK_H);
  for (let i = 0; i < mask.length; i += 1) {
    mask[i] = pixels[i * 4] > 128 ? 1 : 0;
  }
  maskCache = mask;
  return mask;
}

/** Sample the mask. Longitude wraps, latitude clamps at the poles. */
function isOnLand(mask: Uint8Array, lng: number, lat: number): boolean {
  const x = ((Math.round(lngToX(lng)) % MASK_W) + MASK_W) % MASK_W;
  const y = clamp(Math.round(latToY(lat)), 0, MASK_H - 1);
  return mask[y * MASK_W + x] === 1;
}

const clamp = (v: number, lo: number, hi: number) =>
  Math.min(hi, Math.max(lo, v));

const mapLinear = (
  v: number,
  inMin: number,
  inMax: number,
  outMin: number,
  outMax: number,
) =>
  inMax === inMin
    ? outMin
    : outMin + ((v - inMin) / (inMax - inMin)) * (outMax - outMin);

/** Dot grid tightness: UI 1..10 -> 1.92..0.64 degrees between dots. */
export const densityToStep = (ui: number) =>
  mapLinear(clamp(ui, 1, 10), 1, 10, 1.92, 0.64);

/**
 * Y-rotation that brings a longitude round to face the camera. Negated because
 * the globe turns under a fixed camera: to look at 105E the globe rotates -105.
 */
export const spinForLongitude = (lng: number) => (-lng * Math.PI) / 180;

/** Position on a sphere of radius r. lng 0 faces the camera at rotation 0. */
export function latLngToVec(lat: number, lng: number, r: number) {
  const a = lat * (Math.PI / 180);
  const b = lng * (Math.PI / 180);
  const cosA = Math.cos(a);
  return {
    x: cosA * Math.sin(b) * r,
    y: Math.sin(a) * r,
    z: cosA * Math.cos(b) * r,
  };
}
/**
 * Probe for WebGL without constructing a renderer. Testing for the constructors
 * first keeps this silent under jsdom, which defines neither — so the existing
 * tests that render the providers view get the CSS fallback rather than a thrown
 * context error. Real GPU-less environments take the same path.
 */
function hasWebGL(): boolean {
  if (typeof window === "undefined" || typeof document === "undefined") {
    return false;
  }
  if (
    !("WebGL2RenderingContext" in window) &&
    !("WebGLRenderingContext" in window)
  ) {
    return false;
  }
  try {
    const probe = document.createElement("canvas");
    const gl = probe.getContext("webgl2") ?? probe.getContext("webgl");
    if (!gl) return false;
    // Release the probe context immediately; browsers cap concurrent contexts.
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    return true;
  } catch {
    return false;
  }
}

function reduceMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/**
 * Subscribe to prefers-reduced-motion changes. Reading it once at setup means a
 * user toggling the OS setting sees no effect until a reload — and on Windows
 * that setting gets flipped by things other than the user (see REDUCED_SLOW_FACTOR).
 * Returns a no-op unsubscribe where matchMedia is unavailable.
 */
function watchReduceMotion(fn: (reduced: boolean) => void): () => void {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function")
    return () => {};
  const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
  const onChange = (e: MediaQueryListEvent) => fn(e.matches);
  mq.addEventListener("change", onChange);
  return () => mq.removeEventListener("change", onChange);
}

/**
 * What to do when the OS asks for reduced motion.
 *
 * - `slow`   spin at REDUCED_SLOW_FACTOR of `speed` (default)
 * - `freeze` stop dead
 * - `spin`   ignore the preference entirely
 */
export type ReducedMotionPolicy = "slow" | "freeze" | "spin";

/**
 * Fraction of `speed` used under `reducedMotion: "slow"`.
 *
 * Why not freeze: on Windows this media query is driven by
 * SystemParametersInfo(SPI_GETCLIENTAREAANIMATION), which is off by default on
 * Server SKUs and in RDP sessions — for performance, not because anyone asked
 * for an accessibility accommodation. Freezing turns those (very common) cases
 * into what looks like a broken globe. A 298px disc turning once every ~12s is
 * far from the large-area parallax that WCAG 2.3.3 is aimed at, so throttling
 * honours the intent while still showing the thing is alive. `freeze` remains
 * available for callers who want the strict reading.
 */
export const REDUCED_SLOW_FACTOR = 0.25;

/** Spin rate in rad/s, signed by direction, after the reduced-motion policy. */
export function resolveSpinRate(
  speed: number,
  direction: "left" | "right",
  policy: ReducedMotionPolicy,
  reduced: boolean,
): number {
  const factor =
    !reduced || policy === "spin"
      ? 1
      : policy === "freeze"
        ? 0
        : REDUCED_SLOW_FACTOR;
  const magnitude = (speed * factor * Math.PI) / 180;
  // Sign only a non-zero magnitude: `0 * -1` is -0, which compares equal under
  // `!==` but not under Object.is, and gives -Infinity if anything ever divides
  // by it. Return a plain 0 for "stopped".
  if (magnitude === 0) return 0;
  return direction === "left" ? -magnitude : magnitude;
}

/** Recursively free GPU resources for a subtree. dispose() is idempotent, so
 *  shared geometry/material hit more than once is harmless. */
function disposeTree(root: Object3D) {
  root.traverse((node) => {
    const mesh = node as Partial<Mesh>;
    mesh.geometry?.dispose();
    const material = mesh.material;
    if (Array.isArray(material)) material.forEach((m) => m.dispose());
    else material?.dispose();
  });
}

/**
 * A globe colour scheme. `land` is a ramp sampled by absolute latitude, so the
 * continents graduate from equator to pole rather than sitting flat in one hue.
 */
export interface GlobePalette {
  /** Sphere (sea) colour. */
  ocean: string;
  /** Dot colours from equator (index 0) to pole (last). At least one. */
  land: string[];
  /** Halo bled outside the silhouette. */
  glow: string;
  /** Limb shading multiplied over the sphere edge, for roundness. */
  shade: string;
}

/** #rgb / #rrggbb -> [0..255, 0..255, 0..255]. */
function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  const full =
    h.length === 3
      ? h
          .split("")
          .map((c) => c + c)
          .join("")
      : h;
  const n = Number.parseInt(full, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

const toHex = (n: number) =>
  Math.round(clamp(n, 0, 255))
    .toString(16)
    .padStart(2, "0");

/**
 * "r g b" for CSS. The space-separated form is what lets a stylesheet apply its
 * own alpha — `rgb(var(--x) / 0.3)` — which `var(--hex)` cannot do.
 */
export const rgbTriple = (hex: string) => hexToRgb(hex).join(" ");

/**
 * Sample a ramp at `t` in 0..1, interpolating in sRGB. sRGB (not linear) is
 * deliberate: these stops are picked by eye against the panel, and lerping in
 * the space they were chosen in is what keeps the midpoints looking right.
 */
export function rampColor(stops: string[], t: number): string {
  if (stops.length === 0) return "#000000";
  if (stops.length === 1) return stops[0];
  const x = clamp(t, 0, 1) * (stops.length - 1);
  const i = Math.min(Math.floor(x), stops.length - 2);
  const f = x - i;
  const a = hexToRgb(stops[i]);
  const b = hexToRgb(stops[i + 1]);
  return `#${toHex(a[0] + (b[0] - a[0]) * f)}${toHex(a[1] + (b[1] - a[1]) * f)}${toHex(a[2] + (b[2] - a[2]) * f)}`;
}

/** Land colour for a latitude: 0 deg -> ramp start, +-90 deg -> ramp end. */
export const landColorAt = (palette: GlobePalette, lat: number) =>
  rampColor(palette.land, Math.min(Math.abs(lat), 90) / 90);

/**
 * Presets, chosen on measured separation rather than by eye alone:
 *
 *   - Ocean vs the #fafbfc panel decides whether the globe reads as a solid
 *     object at all. The first pass used a pale #8cc0e0 sea, only ΔE 1.9 from
 *     the panel, which is exactly why it looked washed out.
 *   - Land vs ocean decides whether ~2px dots stay legible. WCAG contrast is
 *     the wrong test here — warm land on cool sea measures 1.04:1 while being
 *     obviously distinct, because the separation is hue, not luminance. ΔE (Lab)
 *     is what tracks it, and these ramps hold ΔE ~60-110 across the whole ramp.
 *   - Pole stops stay off pure white: a near-white dot at the limb dissolves
 *     into the panel behind it.
 */
export const GLOBE_PALETTES: Record<string, GlobePalette> = {
  // Default. Deep azure sea (ΔE 4.2 from the panel), continents warming from
  // the brand orange at the equator through amber to pale sand at the poles.
  aurora: {
    ocean: "#3b7fa8",
    land: ["#ff6a3d", "#ff9540", "#ffbe4d", "#ffe07a", "#fff4d0"],
    glow: "#5aa3cc",
    shade: "#0d3a52",
  },
  // Naturalistic: tropical green through arid tan to polar ice.
  earth: {
    ocean: "#4b93bd",
    land: ["#2f9c62", "#63ae47", "#c2b94c", "#e3c795", "#eaf2f8"],
    glow: "#6fb0d4",
    shade: "#123c56",
  },
  // Highest contrast: deep teal sea, hot sunset land.
  sunset: {
    ocean: "#2f6f86",
    land: ["#ff4d2e", "#ff7a33", "#ffa93c", "#ffd75e", "#fff3c4"],
    glow: "#3d8ba6",
    shade: "#06222f",
  },
  // The previous near-monochrome look, kept as an escape hatch.
  mono: {
    ocean: "#edf1f4",
    land: ["#6b7885", "#7c8894", "#95a0aa"],
    glow: "#c9d3da",
    shade: "#68787f",
  },
};

export interface GlobeMarker {
  lat: number;
  lng: number;
}

export interface RouteGlobeProps {
  /** Degrees per second of spin. 0 freezes it. */
  speed?: number;
  /** "right" matches Earth's real west-to-east rotation. */
  direction?: "left" | "right";
  /** Follow damping, 0 (snap) .. 10 (very loose). Governs drag inertia. */
  smoothing?: number;
  /** Dot grid tightness, 1 (sparse) .. 10 (dense). */
  density?: number;
  /** Dot diameter as a fraction of globe diameter. */
  dotScale?: number;
  /** Fraction of the box's shorter axis the globe should span. */
  fill?: number;
  /** Pause the spin while the pointer is over the sphere. */
  stopOnHover?: boolean;
  /** Allow drag-to-spin. */
  draggable?: boolean;
  /** Drag sensitivity, 0 .. 10. */
  dragSpeed?: number;
  initialLatitude?: number;
  initialLongitude?: number;
  /** A preset name from GLOBE_PALETTES, or a full palette of your own. */
  palette?: keyof typeof GLOBE_PALETTES | GlobePalette;
  /** Response to prefers-reduced-motion. See ReducedMotionPolicy. */
  reducedMotion?: ReducedMotionPolicy;
  markers?: GlobeMarker[];
  markerColor?: string;
  /** Marker diameter as a fraction of globe diameter. */
  markerScale?: number;
  className?: string;
}

const NO_MARKERS: GlobeMarker[] = [];
const FOV = 50;
const RADIUS = 1;
/** Just short of the pole — at exactly 90 deg the spin axis points at the camera. */
const MAX_TILT = (85 * Math.PI) / 180;
/** Settle threshold, radians. */
const EPS = 1e-4;
/** Fling inertia: fraction of drag rate carried over, per second. */
const INERTIA = 18;
/** Below this the fling is spent, rad/s. */
const VEL_EPS = 0.6;

/**
 * Camera distance such that the globe spans exactly `fill` of the box's shorter
 * axis.
 *
 * The subtlety: a sphere's outline is the *tangent* circle, not the equator, so
 * it projects larger than R/d suggests — by ~11% at these distances. Framing off
 * the equator (`R / (fill * tan(halfFov))`) therefore overfills, and at
 * fill >= 0.9 the poles get clipped flat by the frustum.
 *
 * Solving the tangent case instead: the silhouette subtends asin(R/d) at the
 * camera, and its NDC extent is tan(asin(R/d)) / (tan(halfFov) * min(1, aspect))
 * = R / (sqrt(d^2 - R^2) * tan(halfFov) * min(1, aspect)). Setting that to
 * `fill` and solving for d gives the closed form below.
 */
export function cameraDistance(fill: number, w: number, h: number): number {
  const halfFov = (FOV / 2) * (Math.PI / 180);
  const aspect = h > 0 ? w / h : 1;
  // Below 1:1 the horizontal FOV is the tighter constraint.
  const k = clamp(fill, 0.1, 1) * Math.tan(halfFov) * Math.min(1, aspect);
  return RADIUS * Math.sqrt(1 + 1 / (k * k));
}

export default function RouteGlobe({
  // 120 deg/s = one turn every 3s.
  speed = 120,
  direction = "right",
  smoothing = 8,
  density = 6,
  // Slightly fatter than a hairline: at 3s/turn the dots travel further than
  // their own width each frame, and larger dots read as motion rather than
  // strobe. Also gives the per-dot colour enough area to actually show.
  dotScale = 0.0075,
  fill = 0.92,
  stopOnHover = true,
  draggable = true,
  dragSpeed = 5,
  initialLatitude = 20,
  // 105E / 20N — East Asia centred, tilted to show the northern hemisphere.
  initialLongitude = 105,
  palette = "aurora",
  // Throttle rather than freeze: see REDUCED_SLOW_FACTOR for why a frozen globe
  // is the wrong default on Windows specifically.
  reducedMotion = "slow",
  markers = NO_MARKERS,
  markerColor = "#ff5a36",
  markerScale = 0.028,
  className,
}: RouteGlobeProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  // The CSS disc shows until the first WebGL frame lands, so the hero is never
  // an empty box while the three chunk loads — and it simply stays if WebGL is
  // unavailable or the import fails.
  const [ready, setReady] = useState(false);

  const colors: GlobePalette =
    typeof palette === "string"
      ? (GLOBE_PALETTES[palette] ?? GLOBE_PALETTES.aurora)
      : palette;
  // Object/array props change identity every render, so key the effect on
  // content and read live values through refs.
  const paletteKey = `${colors.ocean}|${colors.land.join(",")}|${colors.glow}|${colors.shade}`;
  const colorsRef = useRef(colors);
  colorsRef.current = colors;

  const markerKey = markers.map((m) => `${m.lat},${m.lng}`).join("|");
  const markersRef = useRef(markers);
  markersRef.current = markers;

  useEffect(() => {
    const host = hostRef.current;
    if (!host || !hasWebGL()) return;

    let cancelled = false;
    const cleanups: Array<() => void> = [];

    void (async () => {
      let THREE: typeof import("three");
      let rings: Ring[];
      try {
        // Both dynamic, so neither the renderer nor the 84 KB of land geometry
        // sits in the entry chunk — first paint of the providers view doesn't
        // wait on either.
        const [three, land] = await Promise.all([
          import("three"),
          import("@/assets/chimera/ne-110m-land.json"),
        ]);
        THREE = three;
        rings = flattenRings(land.default as unknown as LandCollection);
      } catch {
        return;
      }
      // Only one await above, so this cannot interleave with the cleanup below:
      // either cleanup already ran (cancelled -> nothing allocated), or every
      // cleanup below is registered before it can run.
      if (cancelled) return;

      const {
        Scene,
        PerspectiveCamera,
        WebGLRenderer,
        SphereGeometry,
        MeshBasicMaterial,
        Color,
        Mesh,
        Group,
        InstancedMesh,
        Matrix4,
        Raycaster,
        Vector2,
        SRGBColorSpace,
      } = THREE;

      const readSize = () => ({
        w: host.clientWidth || host.offsetWidth || 320,
        h: host.clientHeight || host.offsetHeight || 320,
      });

      const framedDistance = (w: number, h: number) =>
        cameraDistance(fill, w, h);

      const { w: w0, h: h0 } = readSize();
      const scene = new Scene();
      const camera = new PerspectiveCamera(FOV, w0 / h0, 0.1, 100);
      camera.position.set(0, 0, framedDistance(w0, h0));
      camera.lookAt(0, 0, 0);

      const renderer = new WebGLRenderer({ antialias: true, alpha: true });
      renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
      renderer.setSize(w0, h0, false);
      renderer.outputColorSpace = SRGBColorSpace;
      const canvas = renderer.domElement;
      canvas.className = "route-globe-canvas";
      if (draggable) canvas.style.cursor = "grab";
      host.appendChild(canvas);

      // ---- scene graph -------------------------------------------------
      // rotation.y is longitude (spin); rotation.x is latitude (tilt). Facing
      // longitude L toward the camera means rotating by -L.
      const spin0 = spinForLongitude(initialLongitude);
      const tilt0 = clamp(
        (initialLatitude * Math.PI) / 180,
        -MAX_TILT,
        MAX_TILT,
      );
      const globe = new Group();
      globe.rotation.set(tilt0, spin0, 0);
      scene.add(globe);

      const palette = colorsRef.current;

      // The ocean sphere is opaque on purpose: it's what occludes the dots on
      // the far side, so this reads as a solid globe rather than a transparent
      // wireframe ball.
      const ocean = new Mesh(
        new SphereGeometry(RADIUS, 64, 48),
        new MeshBasicMaterial({ color: new Color(palette.ocean) }),
      );
      globe.add(ocean);

      // ---- land dots ---------------------------------------------------
      const mask = getLandMask(rings);
      const step = densityToStep(density);
      const coords: Array<[number, number]> = [];
      if (mask) {
        for (let lat = -90; lat <= 90; lat += step) {
          // Widen longitude spacing toward the poles, otherwise dots bunch up
          // where the meridians converge.
          const c = Math.cos((Math.abs(lat) * Math.PI) / 180);
          const lngStep = c > 0.01 ? step / Math.max(0.3, c) : 360;
          for (let lng = -180; lng < 180; lng += lngStep) {
            if (isOnLand(mask, lng, lat)) coords.push([lng, lat]);
          }
        }
      }

      if (coords.length) {
        const dots = new InstancedMesh(
          new SphereGeometry(RADIUS * dotScale, 4, 3),
          // White base: instanceColor multiplies into diffuse, so anything other
          // than white would tint the whole ramp. Note we must NOT set
          // vertexColors here — three drives USE_COLOR off instanceColor for the
          // fragment stage on its own, and the flag would make the vertex shader
          // read a `color` attribute the geometry doesn't have.
          new MeshBasicMaterial({ color: 0xffffff }),
          coords.length,
        );
        // One Color per latitude band (~150 of them) rather than per dot.
        const bandCache = new Map<string, InstanceType<typeof Color>>();
        const bandColor = (lat: number) => {
          const hex = landColorAt(palette, lat);
          let c = bandCache.get(hex);
          if (!c) {
            c = new Color(hex);
            bandCache.set(hex, c);
          }
          return c;
        };
        const m = new Matrix4();
        for (let i = 0; i < coords.length; i += 1) {
          const p = latLngToVec(coords[i][1], coords[i][0], RADIUS);
          dots.setColorAt(i, bandColor(coords[i][1]));
          m.makeTranslation(p.x, p.y, p.z);
          dots.setMatrixAt(i, m);
        }
        dots.instanceMatrix.needsUpdate = true;
        // setColorAt allocates instanceColor on first call; flag it either way so
        // the buffer is uploaded.
        if (dots.instanceColor) dots.instanceColor.needsUpdate = true;
        globe.add(dots);
      }

      // ---- markers -----------------------------------------------------
      const marks = markersRef.current;
      if (marks.length) {
        const markGeo = new SphereGeometry(RADIUS * markerScale, 16, 12);
        const markMat = new MeshBasicMaterial({
          color: new Color(markerColor),
        });
        for (const mk of marks) {
          if (!Number.isFinite(mk?.lat) || !Number.isFinite(mk?.lng)) continue;
          // Lift off the surface slightly so markers never z-fight the ocean.
          const p = latLngToVec(mk.lat, mk.lng, RADIUS * 1.01);
          const mesh = new Mesh(markGeo, markMat);
          mesh.position.set(p.x, p.y, p.z);
          globe.add(mesh);
        }
      }

      // ---- animation ---------------------------------------------------
      // Rotation is degrees/second against a measured delta, so the pace is
      // identical on 60 Hz and 144 Hz. The original advanced a fixed amount per
      // frame, which spun 2.4x faster on a 144 Hz panel.
      // Mutable: the OS preference can flip while we're running, and the watcher
      // below reassigns this rather than tearing the whole scene down.
      let rate = resolveSpinRate(
        speed,
        direction,
        reducedMotion,
        reduceMotion(),
      );
      const sm = clamp(smoothing, 0, 10);
      const lerpK = sm <= 0 ? 1 : mapLinear(sm, 0, 10, 0.4, 0.06);
      const decay = mapLinear(sm, 0, 10, 0.7, 0.94);

      const spin = { now: spin0, target: spin0 };
      const tilt = { now: tilt0, target: tilt0 };
      const vel = { x: 0, y: 0 };
      let raf: number | null = null;
      let last = performance.now();
      let dragging = false;
      let hovering = false;
      let onScreen = true;
      let tabVisible = !document.hidden;

      const awake = () => onScreen && tabVisible;
      const start = () => {
        if (raf === null && awake()) raf = requestAnimationFrame(frame);
      };
      // Called when resuming from a pause, so a long gap doesn't jump the globe.
      const resume = () => {
        last = performance.now();
        start();
      };

      function frame(t: number) {
        raf = null;
        // Clamp dt as a second guard against long gaps (background throttling).
        const dt = Math.min((t - last) / 1000, 0.1);
        last = t;

        const spinning = rate !== 0 && !dragging && !(stopOnHover && hovering);
        if (spinning) spin.target += rate * dt;

        if (!dragging) {
          if (Math.abs(vel.x) > VEL_EPS || Math.abs(vel.y) > VEL_EPS) {
            // Inertia is rad/second, decayed per 60Hz-frame-equivalent.
            spin.target += vel.x * dt;
            tilt.target = clamp(tilt.target + vel.y * dt, -MAX_TILT, MAX_TILT);
            const d = Math.pow(decay, dt * 60);
            vel.x *= d;
            vel.y *= d;
          } else {
            vel.x = 0;
            vel.y = 0;
          }
        }

        const k = lerpK >= 1 ? 1 : 1 - Math.pow(1 - lerpK, dt * 60);
        spin.now += (spin.target - spin.now) * k;
        tilt.now += (tilt.target - tilt.now) * k;

        // Keep the ever-accumulating spin bounded. Shift target with it so the
        // lerp gap is untouched.
        if (spin.now > TAU) {
          spin.now -= TAU;
          spin.target -= TAU;
        } else if (spin.now < -TAU) {
          spin.now += TAU;
          spin.target += TAU;
        }

        globe.rotation.y = spin.now;
        globe.rotation.x = tilt.now;
        renderer.render(scene, camera);

        const settling =
          Math.abs(spin.target - spin.now) > EPS ||
          Math.abs(tilt.target - tilt.now) > EPS;
        const coasting = Math.abs(vel.x) > VEL_EPS || Math.abs(vel.y) > VEL_EPS;
        if (spinning || dragging || settling || coasting) start();
      }

      // ---- drag to spin ------------------------------------------------
      // Pointer capture rather than document listeners: the drag keeps tracking
      // if the cursor leaves the canvas, and releases correctly either way.
      const sens = mapLinear(clamp(dragSpeed, 0, 10), 0, 10, 0.001, 0.02);
      let lastX = 0;
      let lastY = 0;
      let activePointer: number | null = null;

      const onPointerDown = (e: PointerEvent) => {
        if (activePointer !== null) return;
        activePointer = e.pointerId;
        canvas.setPointerCapture(e.pointerId);
        dragging = true;
        vel.x = 0;
        vel.y = 0;
        lastX = e.clientX;
        lastY = e.clientY;
        canvas.style.cursor = "grabbing";
        resume();
      };

      const onPointerMove = (e: PointerEvent) => {
        if (activePointer !== e.pointerId) return;
        const dx = (e.clientX - lastX) * sens;
        const dy = (e.clientY - lastY) * sens;
        lastX = e.clientX;
        lastY = e.clientY;
        spin.target += dx;
        tilt.target = clamp(tilt.target + dy, -MAX_TILT, MAX_TILT);
        // Convert per-move delta into a rad/second throw for the coast.
        vel.x = dx * INERTIA;
        vel.y = dy * INERTIA;
        start();
      };

      const onPointerUp = (e: PointerEvent) => {
        if (activePointer !== e.pointerId) return;
        activePointer = null;
        dragging = false;
        canvas.style.cursor = "grab";
        start();
      };

      if (draggable) {
        canvas.addEventListener("pointerdown", onPointerDown);
        canvas.addEventListener("pointermove", onPointerMove);
        canvas.addEventListener("pointerup", onPointerUp);
        canvas.addEventListener("pointercancel", onPointerUp);
      }

      // ---- hover pause -------------------------------------------------
      // Raycast the sphere rather than using the box bounds, so the empty
      // corners of the stage don't stop the spin.
      const ray = new Raycaster();
      const ndc = new Vector2();
      const onHover = (e: PointerEvent) => {
        const r = canvas.getBoundingClientRect();
        if (!r.width || !r.height) return;
        ndc.x = ((e.clientX - r.left) / r.width) * 2 - 1;
        ndc.y = -((e.clientY - r.top) / r.height) * 2 + 1;
        ray.setFromCamera(ndc, camera);
        const wasHovering = hovering;
        hovering = ray.intersectObject(ocean).length > 0;
        if (wasHovering && !hovering) resume();
      };
      const onLeave = () => {
        if (!hovering) return;
        hovering = false;
        resume();
      };
      if (stopOnHover) {
        canvas.addEventListener("pointermove", onHover);
        canvas.addEventListener("pointerleave", onLeave);
      }

      // ---- pause while hidden or off-screen ----------------------------
      // A decorative globe has no business burning GPU behind a hidden window.
      const onVisibility = () => {
        tabVisible = !document.hidden;
        if (tabVisible) resume();
      };
      document.addEventListener("visibilitychange", onVisibility);

      // Toggling the OS animation setting takes effect immediately, rather than
      // waiting for a reload.
      const unwatchMotion = watchReduceMotion((reduced) => {
        rate = resolveSpinRate(speed, direction, reducedMotion, reduced);
        resume();
      });

      const io = new IntersectionObserver(
        (entries) => {
          onScreen = entries.some((entry) => entry.isIntersecting);
          if (onScreen) resume();
        },
        { threshold: 0 },
      );
      io.observe(host);

      const ro = new ResizeObserver(() => {
        const { w, h } = readSize();
        camera.aspect = w / h;
        camera.position.z = framedDistance(w, h);
        camera.updateProjectionMatrix();
        renderer.setSize(w, h, false);
        start();
      });
      ro.observe(host);

      // Paint once before revealing, so the fallback never swaps to a blank
      // canvas for a frame.
      renderer.render(scene, camera);
      setReady(true);
      start();

      cleanups.push(() => {
        if (raf !== null) cancelAnimationFrame(raf);
        canvas.removeEventListener("pointerdown", onPointerDown);
        canvas.removeEventListener("pointermove", onPointerMove);
        canvas.removeEventListener("pointerup", onPointerUp);
        canvas.removeEventListener("pointercancel", onPointerUp);
        canvas.removeEventListener("pointermove", onHover);
        canvas.removeEventListener("pointerleave", onLeave);
        document.removeEventListener("visibilitychange", onVisibility);
        unwatchMotion();
        io.disconnect();
        ro.disconnect();
        disposeTree(scene);
        renderer.dispose();
        // Without this the WebGL context lingers until GC; browsers cap how many
        // live contexts a document may hold.
        renderer.forceContextLoss();
        canvas.remove();
      });
    })();

    return () => {
      cancelled = true;
      for (const fn of cleanups) fn();
    };
  }, [
    speed,
    direction,
    smoothing,
    density,
    dotScale,
    fill,
    stopOnHover,
    draggable,
    dragSpeed,
    initialLatitude,
    initialLongitude,
    paletteKey,
    reducedMotion,
    markerColor,
    markerScale,
    markerKey,
  ]);

  return (
    <div
      ref={hostRef}
      className={`route-globe${className ? ` ${className}` : ""}`}
      // Fed to CSS so the halo, limb shading and fallback disc all track the
      // palette and the exact silhouette size without duplicating the values.
      style={
        {
          "--globe-ocean": colors.ocean,
          "--globe-land-mid": landColorAt(colors, 45),
          // rgb triples: the stylesheet applies its own alpha to these.
          "--globe-glow-rgb": rgbTriple(colors.glow),
          "--globe-shade-rgb": rgbTriple(colors.shade),
          // Drives the CSS disc size, so the halo and limb shading land exactly
          // on the WebGL silhouette (diameter = fill x shorter axis).
          // String, not number: a bare number must reach CSS unitless for
          // calc(var(--globe-fill) * 100cqh) to parse.
          "--globe-fill": String(clamp(fill, 0.1, 1)),
        } as CSSProperties
      }
    >
      {/* Halo bleeding past the silhouette. Behind the canvas, so the sphere
          covers the part that overlaps and only the spill shows. */}
      <span className="route-globe-glow" aria-hidden="true" />
      {/* Visible until WebGL paints; permanent if WebGL is unavailable. */}
      <span
        className={`route-globe-fallback${ready ? " is-hidden" : ""}`}
        aria-hidden="true"
      />
      {/* Limb shading, above the canvas. The silhouette is a circle at every
          orientation, so a fixed disc lines up no matter how the globe turns —
          and screen-fixed shading is what reads as a light source, which a
          texture rotating with the sphere cannot do. */}
      <span className="route-globe-shade" aria-hidden="true" />
    </div>
  );
}
