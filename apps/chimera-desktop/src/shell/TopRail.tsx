import type { ActiveFeature } from "../App";

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

export function TopRail({ active, onNavigate }: Props) {
  return (
    <nav
      role="navigation"
      aria-label="Main navigation"
      style={{
        height: 48, display: "flex", alignItems: "center",
        borderBottom: "1px solid #282828", background: "#111111",
        flexShrink: 0,
      }}
    >
      {/* Logo */}
      <div style={{ display: "flex", alignItems: "center", gap: 9, padding: "0 20px" }}>
        <div style={{ width: 18, height: 18, borderRadius: 2, background: "#FF4D3D" }} />
        <span style={{ fontWeight: 600, fontSize: 14 }}>Chimera++</span>
      </div>

      <div style={{ width: 1, height: 22, background: "#282828" }} />

      {/* Tabs */}
      <div style={{ display: "flex", alignItems: "stretch", height: "100%", flex: 1 }}>
        {TABS.map((tab) => (
          <button
            key={tab.id}
            role="tab"
            aria-selected={active === tab.id}
            onClick={() => onNavigate(tab.id)}
            style={{
              padding: "0 20px", height: "100%",
              background: "transparent", border: "none",
              borderBottom: active === tab.id ? "2px solid #FF4D3D" : "2px solid transparent",
              color: active === tab.id ? "#EBEBEB" : "#5E5E5E",
              fontWeight: active === tab.id ? 600 : 400,
              fontSize: 13, cursor: "pointer",
            }}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Status */}
      <div style={{ display: "flex", alignItems: "center", gap: 7, padding: "0 20px" }}>
        <div style={{ width: 6, height: 6, borderRadius: "50%", background: "#34C759" }} />
        <span style={{ fontSize: 12, color: "#5E5E5E" }}>Ready</span>
      </div>
    </nav>
  );
}
