// Chimera++ 2.0 — root shell.
// Feature routing: Home | Providers | Codex | Appearance | Settings
// Rule (G12): page components must NOT directly read/write files or call platform APIs.
// All data access goes through Tauri invoke() commands in each feature's hooks/.

import { useCallback, useState } from "react";
import { HomeFeature }       from "@/features/home";
import { ProvidersFeature }  from "@/features/providers";
import { CodexFeature }      from "@/features/codex";
import { AppearanceFeature } from "@/features/appearance";
import { SettingsFeature }   from "@/features/settings";
import { TopRail }           from "@/shell/TopRail";
import { FirstRun }          from "@/shell/FirstRun";
import { I18nProvider, useI18n } from "@/i18n";
import { FEATURES, type ActiveFeature } from "@/shell/nav";

/**
 * Resolve the screen to open from `?screen=<name>`.
 *
 * Used by scripts/design-verify to load each screen directly instead of
 * driving clicks. Anything unrecognised falls back to `home`, so a malformed
 * query string can never render a blank window.
 */
function resolveInitialFeature(): ActiveFeature {
  try {
    const raw = new URLSearchParams(window.location.search).get("screen");
    return FEATURES.find((f) => f === raw) ?? "home";
  } catch {
    return "home";
  }
}

export default function App() {
  const [active, setActive] = useState<ActiveFeature>(resolveInitialFeature);
  // Preflight gates the whole app rather than overlaying it: while it is
  // blocked there is no working UI underneath, and leaving the rail reachable
  // would let keyboard and screen-reader users into screens that cannot
  // function. FirstRun calls onReady when the machine is fine — including
  // outside the desktop shell, where there is no backend to ask.
  const [ready, setReady] = useState(false);
  const markReady = useCallback(() => setReady(true), []);

  return (
    <I18nProvider>
      {ready ? (
        <div className="app-canvas">
          <div className="app-window">
            <WindowBar active={active} />
            <div className="app-body">
              <TopRail active={active} onNavigate={setActive} />
              <main className="app-content">
                {active === "home"       && <HomeFeature />}
                {active === "providers"  && <ProvidersFeature />}
                {active === "codex"      && <CodexFeature />}
                {active === "appearance" && <AppearanceFeature />}
                {active === "settings"   && <SettingsFeature />}
              </main>
            </div>
          </div>
        </div>
      ) : (
        <div style={{ height: "100vh", overflow: "hidden" }}>
          <FirstRun onReady={markReady} />
        </div>
      )}
    </I18nProvider>
  );
}

function WindowBar({ active }: { active: ActiveFeature }) {
  const { t } = useI18n();
  return (
    <header className="window-bar">
      <div className="window-bar-left">
        <span className="window-lights" aria-hidden="true"><i /><i /><i /></span>
        <span className="window-title">Chimera++ / {t("shell.workspaceShort")}</span>
      </div>
      <div className="window-bar-actions">
        <span className="window-status"><i aria-hidden="true" />{t("shell.statusReady")}</span>
        <button className="window-more" type="button" aria-label={t("shell.more")}>...</button>
      </div>
    </header>
  );
}
