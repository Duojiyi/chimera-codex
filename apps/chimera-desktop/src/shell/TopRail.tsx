import type { ActiveFeature } from "./nav";
import { color, type as font, radius } from "../design/tokens.ts";
import { useI18n, type TranslationKey } from "../i18n/index.tsx";

// Module-level constants hold i18n KEYS, never translated text — translation
// happens at render so a language switch re-renders without reloading the
// webview. scripts/verify-i18n.mjs enforces this.
const TABS: { id: ActiveFeature; labelKey: TranslationKey }[] = [
  { id: "home",       labelKey: "nav.home" },
  { id: "providers",  labelKey: "nav.providers" },
  { id: "codex",      labelKey: "nav.codex" },
  { id: "appearance", labelKey: "nav.appearance" },
  { id: "settings",   labelKey: "nav.settings" },
];

interface Props {
  active: ActiveFeature;
  onNavigate: (f: ActiveFeature) => void;
}

/**
 * Left workspace navigation. The parent window bar owns the window chrome;
 * this rail is the full-height 232px workspace sidebar from the Pencil spec.
 */
export function TopRail({ active, onNavigate }: Props) {
  const { t } = useI18n();
  return (
    <nav
      role="navigation"
      aria-label={t("nav.ariaLabel")}
      style={{ width: 232, display: "flex", flexDirection: "column", background: color.sidebar, flexShrink: 0, padding: "22px 16px", boxSizing: "border-box" }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "0 4px", marginBottom: 24 }}>
        <div
          style={{
            width: 30,
            height: 30,
            borderRadius: radius.xs,
            background: color.brandMark,
            flexShrink: 0,
          }}><span aria-hidden="true" style={{ display: "block", width: 16, height: 16, margin: 7, borderRadius: "50%", background: color.brandCore }} /></div>
        <span style={{ fontFamily: font.family, ...font.appName, color: color.primary }}>
          Chimera++
        </span>
      </div>

      <span className="rail-label">{t("shell.workspaceShort")}</span>
      <div role="tablist" aria-label={t("shell.tabsAriaLabel")} style={{ display: "flex", flexDirection: "column", gap: 4, flex: 1 }}>
        {TABS.map((tab) => {
          const on = active === tab.id;
          return (
            <button
              key={tab.id}
              role="tab"
              aria-selected={on}
              onClick={() => onNavigate(tab.id)}
              style={{
                width: "100%",
                minHeight: 42,
                padding: "0 12px",
                background: on ? color.ink3 : "transparent",
                border: "none",
                borderRadius: radius.sm,
                color: on ? color.primary : color.secondary,
                fontFamily: font.family,
                ...(on ? font.uiStrong : font.ui),
                textAlign: "left",
                cursor: "pointer",
              }}
            >
              {t(tab.labelKey)}
            </button>
          );
        })}
      </div>

      <div className="rail-promo">
        <strong>{t("shell.promoTitle")}</strong>
        <span>{t("shell.promoCopy")}</span>
        <i aria-hidden="true" />
      </div>
      <div className="rail-account">
        <span className="rail-avatar" aria-hidden="true" />
        <span><strong>{t("shell.localWorkspace")}</strong><small>{t("shell.loggedIn")}</small></span>
        <span className="rail-chevron" aria-hidden="true">⌄</span>
      </div>
    </nav>
  );
}
