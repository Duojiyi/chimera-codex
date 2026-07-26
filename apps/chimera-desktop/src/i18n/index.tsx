// i18n runtime.
//
// Chinese is the default and the authoring language of the dictionary; English
// is a switchable alternative. Both dictionaries are complete by construction:
// en.ts is typed Record<TranslationKey, string>, so a key present in zh.ts and
// absent in en.ts is a compile error.
//
// Design note — why this does NOT reload the webview on switch:
// The 1.x manager resolves language once at import and reloads the window when
// it changes, because its Chinese literals live in module-level constants
// (route tables, preset labels) that evaluate a single time. v2 avoids that by
// keeping only translation *keys* in module-level constants and calling t() at
// render time, so a React state change is sufficient and the switch is instant.
// scripts/verify-i18n.mjs enforces the rule that makes this safe.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import { zh, type TranslationKey } from "./zh.ts";
import { en } from "./en.ts";

/**
 * Tell the backend which language the tray menu should use.
 *
 * The tray is built during Tauri setup, before any webview exists to ask, so it
 * starts in Chinese and is corrected from here. Best-effort by design: outside
 * the desktop shell (the design-verify harness runs in a plain browser) there
 * is no tray and no invoke, and the UI must not break because of it.
 */
function syncTrayLanguage(lang: Language): void {
  const invoke = (
    globalThis as { __TAURI__?: { core?: { invoke?: (c: string, a?: unknown) => Promise<unknown> } } }
  ).__TAURI__?.core?.invoke;
  void invoke?.("set_tray_language", { lang })?.catch(() => {
    // A tray that failed to relabel is cosmetic; never surface it.
  });
}

export type { TranslationKey };
export type Language = "zh" | "en";

const STORAGE_KEY = "chimera-lang";
const DICTS: Record<Language, Record<TranslationKey, string>> = { zh, en };

/**
 * Chinese is the product default; only an explicit "en" overrides it.
 *
 * A `?lang=` query parameter takes precedence over the stored choice so the
 * design-verification harness can capture both languages without mutating
 * local storage. Anything other than the two known codes is ignored, so a
 * malformed URL always lands on the Chinese default.
 */
function resolveInitialLanguage(): Language {
  try {
    const fromUrl = new URLSearchParams(window.location.search).get("lang");
    if (fromUrl === "en" || fromUrl === "zh") return fromUrl;
  } catch {
    // Non-browser context — fall through to storage.
  }
  try {
    return window.localStorage.getItem(STORAGE_KEY) === "en" ? "en" : "zh";
  } catch {
    // Private mode / storage disabled — fall back to the default.
    return "zh";
  }
}

interface I18nValue {
  lang: Language;
  /** Translate a key. */
  t: (key: TranslationKey) => string;
  /**
   * Translate and interpolate `{0}`, `{1}`, … positionally.
   * Kept separate from `t` so the plain path stays a single object lookup.
   */
  tf: (key: TranslationKey, args: (string | number)[]) => string;
  setLang: (next: Language) => void;
}

const I18nContext = createContext<I18nValue | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Language>(resolveInitialLanguage);

  const setLang = useCallback((next: Language) => {
    setLangState(next);
    try {
      window.localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Persistence is best-effort; the in-session switch still applies.
    }
    // Keep the document language in sync for assistive tech and font shaping.
    if (typeof document !== "undefined") {
      document.documentElement.lang = next === "zh" ? "zh-Hans" : "en";
    }
    syncTrayLanguage(next);
  }, []);

  // The initial language can come from storage or ?lang=, neither of which the
  // backend saw when it built the tray. Reconcile once on mount.
  useEffect(() => {
    if (typeof document !== "undefined") {
      document.documentElement.lang = lang === "zh" ? "zh-Hans" : "en";
    }
    syncTrayLanguage(lang);
    // Mount only: later changes go through setLang.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const value = useMemo<I18nValue>(() => {
    const dict = DICTS[lang];
    const t = (key: TranslationKey) => dict[key];
    const tf = (key: TranslationKey, args: (string | number)[]) =>
      dict[key].replace(/\{(\d+)\}/g, (whole, i) => {
        const arg = args[Number(i)];
        return arg === undefined ? whole : String(arg);
      });
    return { lang, t, tf, setLang };
  }, [lang, setLang]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

/** Access the active language and translators. Must be inside I18nProvider. */
export function useI18n(): I18nValue {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    throw new Error("useI18n must be used inside <I18nProvider>");
  }
  return ctx;
}

/**
 * Translate outside React (module-level defaults, error mappers).
 * Reads the persisted choice directly. Prefer useI18n() in components so the
 * value re-renders on switch.
 */
export function translateStatic(key: TranslationKey): string {
  return DICTS[resolveInitialLanguage()][key];
}
