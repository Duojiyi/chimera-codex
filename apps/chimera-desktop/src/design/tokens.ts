// Design tokens — Soft Bento desktop shell baseline.
// Keep visual decisions here so the implementation can be checked against the
// Pencil frame without scattering one-off values through feature components.
// Feature components must import from here — scripts/verify-design-tokens.mjs
// fails CI if a raw hex literal appears under src/features/ or src/shell/.

/** Surface + text colours (Pencil variable names preserved). */
export const color = {
  /** Pencil outer canvas and app desktop window surfaces. */
  ink0: "#A8CFD2",
  ink1: "#E8F3F0",
  ink2: "#F7F9F8",
  ink3: "#FFFFFF",
  rule: "#D6E4E2",
  primary: "#182526",
  secondary: "#375456",
  muted: "#3A595C",
  dim: "#345154",
  accent: "#192626",
  accentDim: "#EDF8F1",
  sidebar: "#E8F3F0",
  window: "#FBFEFC",
  promo: "#B5D8D8",
  promoCircle: "#FFE23D",
  brandMark: "#C6E8E3",
  brandCore: "#FFD93D",
  accountAvatar: "#D7E7FA",
  cardAlt: "#F5FBEA",

  /** Healthy / running / passed */
  green: "#35A96B",
  amber: "#E8B657",
  danger: "#C85252",
  /** AA text counterparts for the status swatches above. */
  greenText: "#207A4A",
  amberText: "#8C5A03",
  dangerText: "#A23434",
  dangerBg: "#FCEBE8",
  dangerBorder: "#EBC5C0",

  /** Explicit transparent (Pencil emits #00000000) */
  transparent: "transparent",
} as const;

/** Type ramp. Each display size carries the lineHeight the design specifies. */
export const type = {
  family: '"Outfit", system-ui, -apple-system, sans-serif',

  /** Home hero provider name */
  hero: { fontSize: 42, fontWeight: 700, lineHeight: 1.05 },
  /** Codex managed-runtime version */
  version: { fontSize: 48, fontWeight: 700, lineHeight: 1 },
  /** Codex update-banner version comparison */
  versionCompare: { fontSize: 36, fontWeight: 700 },
  /** Providers detail title */
  detailTitle: { fontSize: 28, fontWeight: 700, lineHeight: 1.05 },
  /** Settings page title */
  pageTitle: { fontSize: 28, fontWeight: 700 },
  /** Appearance skin detail title */
  skinTitle: { fontSize: 24, fontWeight: 700 },
  /** Home hero metric row */
  metric: { fontSize: 22, fontWeight: 600 },
  /** Home hero subtitle, Codex runtime name */
  subtitle: { fontSize: 16, fontWeight: 400 },
  /** Primary action button label */
  actionLabel: { fontSize: 13, fontWeight: 700 },
  /** Codex runtime platform line */
  runtimeName: { fontSize: 14, fontWeight: 400 },
  /** App name in rail, settings page subtitle */
  appName: { fontSize: 15, fontWeight: 700 },
  /** Settings page subtitle, Providers detail body */
  body: { fontSize: 13, fontWeight: 400 },
  /** Nav tab, panel title, settings item key */
  ui: { fontSize: 13, fontWeight: 500 },
  /** Panel titles + active tab (semibold 13) */
  uiStrong: { fontSize: 13, fontWeight: 700 },
  /** Spec-sheet keys/values, status text, version string */
  caption: { fontSize: 12, fontWeight: 400 },
  /** Spec-sheet values (medium) */
  captionStrong: { fontSize: 12, fontWeight: 500 },
  /** Home hero eyebrow */
  eyebrow: { fontSize: 11, fontWeight: 500, letterSpacing: 1.5 },
  /** Section labels — uppercase, tracked */
  sectionLabel: { fontSize: 10, fontWeight: 700, letterSpacing: 1.5 },
} as const;

/** Fixed dimensions the design pins exactly. */
export const size = {
  /** Top rail height */
  rail: 58,
  /** Panel/detail header height */
  panelHead: 52,
  /** Logo mark square */
  mark: 18,
  /** Status dot diameter */
  dot: 6,

  /** Providers list panel */
  providerList: 280,
  /** Providers detail left column */
  providerDetailLeft: 420,
  /** Providers detail right column */
  providerDetailRight: 240,
  /** Providers list row */
  providerRow: 68,

  /** Codex left pane */
  codexLeftPane: 440,
  /** Codex spec-sheet key column */
  codexSpecKey: 140,
  /** Codex spec-sheet row */
  codexSpecRow: 34,
  /** Codex version-history row */
  codexHistoryRow: 44,

  /** Appearance skin list */
  skinList: 280,
  /** Appearance skin row */
  skinRow: 64,
  /** Appearance metadata column */
  skinMeta: 220,

  /** Settings category nav */
  settingsNav: 232,
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
  heroHeight: 280,
  /** Home hero vertical padding */
  heroPadY: 32,
  /** Home hero horizontal padding */
  heroPadX: 36,
  /** Home data-strip column vertical padding */
  dataPadY: 20,
  /** Home data-strip column horizontal padding */
  dataPadX: 24,
  /** Home data-strip key column width */
  dataKeyWidth: 130,
  /** Home hero: gap between eyebrow label and hero title (EyebrowGap spacer) */
  heroEyebrowGap: 12,
} as const;

/** Corner radii the design uses. */
export const radius = {
  /** Logo mark, applied badge */
  xs: 6,
  /** Buttons, selects */
  sm: 8,
  /** Primary action button */
  md: 12,
  /** Active tag pill */
  pill: 999,
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
