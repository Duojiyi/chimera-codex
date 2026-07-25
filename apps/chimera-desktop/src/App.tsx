// Chimera++ 2.0 — root shell.
// Feature routing: Home | Providers | Codex | Appearance | Settings
// Rule (G12): page components must NOT directly read/write files or call platform APIs.
// All data access goes through Tauri invoke() commands in each feature's hooks/.

import { useState } from "react";
import { HomeFeature }       from "@/features/home";
import { ProvidersFeature }  from "@/features/providers";
import { CodexFeature }      from "@/features/codex";
import { AppearanceFeature } from "@/features/appearance";
import { SettingsFeature }   from "@/features/settings";
import { TopRail }           from "@/shell/TopRail";

export type ActiveFeature = "home" | "providers" | "codex" | "appearance" | "settings";

export default function App() {
  const [active, setActive] = useState<ActiveFeature>("home");

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100vh", overflow: "hidden" }}>
      <TopRail active={active} onNavigate={setActive} />
      <main style={{ flex: 1, overflow: "hidden" }}>
        {active === "home"       && <HomeFeature />}
        {active === "providers"  && <ProvidersFeature />}
        {active === "codex"      && <CodexFeature />}
        {active === "appearance" && <AppearanceFeature />}
        {active === "settings"   && <SettingsFeature />}
      </main>
    </div>
  );
}
