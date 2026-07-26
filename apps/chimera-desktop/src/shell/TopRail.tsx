import type { ActiveFeature } from "../App";
import { color, type as font, size, radius, hairline, indicator } from "../design/tokens.ts";

const TABS: { id: ActiveFeature; label: string }[] = [
  { id: "home",       label: "Home" },
  { id: "providers",  label: "Providers" },
  { id: "codex",      label: "Codex" },
  { id: "appearance", label: "Appearance" },
  { id: "settings",   label: "Settings" },
];

interface Props {
  active: ActiveFeature;
  onNavigate: (f: ActiveFeature) => void;
}

/**
 * Top navigation rail. Spec: Pencil `TopRail` frame, identical on all 5 screens.
 * height 48 · bg ink1 · bottom border rule · Logo padding [0,20] gap 9
 * · Mark 18x18 r2 accent · AppName 14/600 · tab padding [0,20] 13px
 * · active tab: 2px accent bottom border + primary/600
 * · RailRight gap 12 padding [0,20]: dot 6px green + 12px muted text
 *   + 1x16 separator + 12px dim version
 */
export function TopRail({ active, onNavigate }: Props) {
  return (
    <nav
      role="navigation"
      aria-label="Main navigation"
      style={{
        height: size.rail,
        display: "flex",
        alignItems: "center",
        borderBottom: `${hairline}px solid ${color.rule}`,
        background: color.ink1,
        flexShrink: 0,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 9, padding: "0 20px" }}>
        <div
          style={{
            width: size.mark,
            height: size.mark,
            borderRadius: radius.xs,
            background: color.accent,
            flexShrink: 0,
          }}
        />
        <span style={{ fontFamily: font.family, ...font.appName, color: color.primary }}>
          Chimera++
        </span>
      </div>

      <div style={{ width: hairline, height: 22, background: color.rule }} />

      <div role="tablist" style={{ display: "flex", alignItems: "stretch", height: "100%", flex: 1 }}>
        {TABS.map((tab) => {
          const on = active === tab.id;
          return (
            <button
              key={tab.id}
              role="tab"
              aria-selected={on}
              onClick={() => onNavigate(tab.id)}
              style={{
                padding: "0 20px",
                height: "100%",
                background: "transparent",
                border: "none",
                borderBottom: `${indicator.tabUnderline}px solid ${on ? color.accent : "transparent"}`,
                color: on ? color.primary : color.muted,
                fontFamily: font.family,
                ...(on ? font.uiStrong : font.ui),
                cursor: "pointer",
              }}
            >
              {tab.label}
            </button>
          );
        })}
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 12, padding: "0 20px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <div
            style={{
              width: size.dot,
              height: size.dot,
              borderRadius: "50%",
              background: color.green,
              flexShrink: 0,
            }}
          />
          <span style={{ fontFamily: font.family, ...font.caption, color: color.muted }}>
            All systems ready
          </span>
        </div>
        <div style={{ width: hairline, height: 16, background: color.rule }} />
        <span style={{ fontFamily: font.family, ...font.caption, color: color.dim }}>
          v2.0.0-beta
        </span>
      </div>
    </nav>
  );
}
