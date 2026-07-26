// Design tokens — extracted verbatim from D:/Desktop/chimera-ui.pen
// SOURCE OF TRUTH. Do not hand-edit values; re-extract from the .pen file.
// Feature components must import from here — scripts/verify-design-tokens.mjs
// fails CI if a raw hex literal appears under src/features/ or src/shell/.

/** Surface + text colours (Pencil variable names preserved). */
export const color = {
  /** Page background */
  ink0: "#0C0C0C",
  /** Top rail, side panels, update banner */
  ink1: "#111111",
  /** Selected list row background */
  ink2: "#181818",
  /** Secondary button / toggle-off / select background */
  ink3: "#222222",
  /** Hairline rules and borders */
  rule: "#282828",

  /** Highest-contrast text: hero, active tab, values */
  primary: "#EBEBEB",
  /** Panel titles, spec-sheet values */
  secondary: "#999999",
  /** Body text, inactive tabs, spec-sheet keys */
  muted: "#5E5E5E",
  /** Eyebrow labels, version string */
  dim: "#3A3A3A",

  /** Single accent — logo mark, active tab underline, primary action */
  accent: "#FF4D3D",
  /** Accent at 10% — active tag / applied badge background */
  accentDim: "#FF4D3D1A",

  /** Healthy / running / passed */
  green: "#34C759",
  /** Update available / degraded */
  amber: "#FF9F0A",
  /** Destructive action + auth failure */
  danger: "#FF453A",
  /** Danger button background */
  dangerBg: "#FF453A11",
  /** Danger button border */
  dangerBorder: "#FF453A33",

  /** Explicit transparent (Pencil emits #00000000) */
  transparent: "transparent",
} as const;

/** Type ramp. Each display size carries the lineHeight the design specifies. */
export const type = {
  family: '"Outfit", system-ui, -apple-system, sans-serif',

  /** Home hero provider name */
  hero: { fontSize: 88, fontWeight: 700, lineHeight: 0.9 },
  /** Codex managed-runtime version */
  version: { fontSize: 72, fontWeight: 700, lineHeight: 0.9 },
  /** Codex update-banner version comparison */
  versionCompare: { fontSize: 52, fontWeight: 700 },
  /** Providers detail title */
  detailTitle: { fontSize: 44, fontWeight: 700, lineHeight: 0.92 },
  /** Settings page title */
  pageTitle: { fontSize: 36, fontWeight: 700 },
  /** Appearance skin detail title */
  skinTitle: { fontSize: 30, fontWeight: 700 },
  /** Home hero metric row */
  metric: { fontSize: 26, fontWeight: 600 },
  /** Home hero subtitle, Codex runtime name */
  subtitle: { fontSize: 20, fontWeight: 400 },
  /** Primary action button label */
  actionLabel: { fontSize: 16, fontWeight: 700 },
  /** Codex runtime platform line */
  runtimeName: { fontSize: 16, fontWeight: 400 },
  /** App name in rail, settings page subtitle */
  appName: { fontSize: 14, fontWeight: 600 },
  /** Settings page subtitle, Providers detail body */
  body: { fontSize: 14, fontWeight: 400 },
  /** Nav tab, panel title, settings item key */
  ui: { fontSize: 13, fontWeight: 400 },
  /** Panel titles + active tab (semibold 13) */
  uiStrong: { fontSize: 13, fontWeight: 600 },
  /** Spec-sheet keys/values, status text, version string */
  caption: { fontSize: 12, fontWeight: 400 },
  /** Spec-sheet values (medium) */
  captionStrong: { fontSize: 12, fontWeight: 500 },
  /** Home hero eyebrow */
  eyebrow: { fontSize: 11, fontWeight: 500, letterSpacing: 1.5 },
  /** Section labels — uppercase, tracked */
  sectionLabel: { fontSize: 10, fontWeight: 600, letterSpacing: 1.5 },
} as const;

/** Fixed dimensions the design pins exactly. */
export const size = {
  /** Top rail height */
  rail: 48,
  /** Panel/detail header height */
  panelHead: 52,
  /** Logo mark square */
  mark: 18,
  /** Status dot diameter */
  dot: 6,

  /** Providers list panel */
  providerList: 300,
  /** Providers detail left column */
  providerDetailLeft: 440,
  /** Providers detail right column */
  providerDetailRight: 240,
  /** Providers list row */
  providerRow: 68,

  /** Codex left pane */
  codexLeftPane: 520,
  /** Codex spec-sheet key column */
  codexSpecKey: 140,
  /** Codex spec-sheet row */
  codexSpecRow: 34,
  /** Codex version-history row */
  codexHistoryRow: 44,

  /** Appearance skin list */
  skinList: 260,
  /** Appearance skin row */
  skinRow: 64,
  /** Appearance metadata column */
  skinMeta: 220,

  /** Settings category nav */
  settingsNav: 220,
  /** Settings category row */
  settingsCatRow: 40,
  /** Settings item row */
  settingsItemRow: 44,
  /** Settings item key column */
  settingsItemKey: 280,
  /** Toggle track */
  toggleW: 36,
  toggleH: 20,
  /** Toggle knob */
  toggleKnob: 14,

  /** Home data-strip column */
  dataCol: 426,
  /** Home data-strip row */
  dataRow: 32,
  /** Home hero right pane */
  heroRight: 300,
  /** Home hero zone height (.pen frame X2mpmJ) */
  heroHeight: 360,
  /** Home hero vertical padding */
  heroPadY: 56,
  /** Home hero horizontal padding */
  heroPadX: 48,
  /** Home data-strip column vertical padding */
  dataPadY: 26,
  /** Home data-strip column horizontal padding */
  dataPadX: 32,
  /** Home data-strip key column width */
  dataKeyWidth: 130,
  /** Home hero: gap between eyebrow label and hero title (EyebrowGap spacer) */
  heroEyebrowGap: 12,
} as const;

/** Corner radii the design uses. */
export const radius = {
  /** Logo mark, applied badge */
  xs: 2,
  /** Buttons, selects */
  sm: 3,
  /** Primary action button */
  md: 4,
  /** Active tag pill */
  pill: 20,
} as const;

/** Hairline width for rules and borders. */
export const hairline = 1;

/** Active-state indicator widths. */
export const indicator = {
  /** Active nav tab underline */
  tabUnderline: 2,
  /** Selected list row left edge */
  rowEdge: 2,
} as const;

/** Separator opacities the design varies deliberately. */
export const ruleOpacity = {
  /** Codex spec-sheet / diagnostics separators */
  spec: 0.3,
  /** Codex version-history, Appearance skin list */
  list: 0.4,
  /** Settings item separators */
  settingsItem: 0.35,
} as const;
