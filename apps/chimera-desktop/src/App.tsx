// Chimera++ 2.0 — root shell.
// Feature routing: Home | Providers | Codex | Appearance | Settings
// Rule (G12): page components must NOT directly read/write files or call platform APIs.
// All data access goes through Tauri invoke() commands in each feature's hooks/.

import { useCallback, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
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
      <div className="app-canvas">
        <div className="app-window">
          <WindowBar active={active} />
          {ready ? (
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
          ) : (
            <div style={{ minHeight: 0, flex: 1, overflow: "hidden" }}>
              <FirstRun onReady={markReady} />
            </div>
          )}
        </div>
      </div>
    </I18nProvider>
  );
}

function WindowBar({ active }: { active: ActiveFeature }) {
  const { t } = useI18n();

  function runWindowAction(action: "close" | "minimize" | "toggleMaximize") {
    try {
      void getCurrentWindow()[action]();
    } catch {
      // Browser-only design verification has no Tauri window handle.
    }
  }

  return (
    <header className="window-bar" data-tauri-drag-region>
      <div className="window-bar-left" data-tauri-drag-region>
        <div className="window-controls" role="group" aria-label={t("shell.windowControls")}>
          <button type="button" className="window-control window-control-close" aria-label={t("shell.closeWindow")} title={t("shell.closeWindow")} onClick={() => runWindowAction("close")} />
          <button type="button" className="window-control window-control-minimize" aria-label={t("shell.minimizeWindow")} title={t("shell.minimizeWindow")} onClick={() => runWindowAction("minimize")} />
          <button type="button" className="window-control window-control-maximize" aria-label={t("shell.maximizeWindow")} title={t("shell.maximizeWindow")} onClick={() => runWindowAction("toggleMaximize")} />
        </div>
        <span className="window-title" data-tauri-drag-region>Chimera++ / {t("shell.workspaceShort")}</span>
      </div>
      <div className="window-bar-actions">
        <span className="window-status"><i aria-hidden="true" />{t("shell.statusReady")}</span>
        <button className="window-more" type="button" aria-label={t("shell.more")}>...</button>
      </div>
    </header>
  );
}
