import React, {
  createContext,
  useContext,
  useState,
  useEffect,
  useCallback,
  useRef,
} from "react";
import type { UpdateInfo } from "../lib/updater";
import { checkForUpdate } from "../lib/updater";
import { settingsApi } from "@/lib/api/settings";
import { isTauri } from "@tauri-apps/api/core";

interface UpdateContextValue {
  // 更新状态
  hasUpdate: boolean;
  updateInfo: UpdateInfo | null;
  isChecking: boolean;
  error: string | null;
  lastCheckedAt: number | null;
  /** 已在后台下载完成、可直接安装的版本号；未暂存时为 null。 */
  stagedVersion: string | null;
  /** 后台暂存是否正在进行。 */
  isStaging: boolean;

  // 提示状态
  isDismissed: boolean;
  dismissUpdate: () => void;

  // 操作方法
  checkUpdate: () => Promise<boolean>;
  resetDismiss: () => void;
}

const DEFAULT_UPDATE_CONTEXT: UpdateContextValue = {
  hasUpdate: false,
  updateInfo: null,
  isChecking: false,
  error: null,
  lastCheckedAt: null,
  stagedVersion: null,
  isStaging: false,
  isDismissed: false,
  dismissUpdate: () => undefined,
  checkUpdate: async () => false,
  resetDismiss: () => undefined,
};

const UpdateContext = createContext<UpdateContextValue>(DEFAULT_UPDATE_CONTEXT);

/** How stale a successful check may get before we run another one. */
export const UPDATE_CHECK_INTERVAL_MS = 15 * 60 * 1000;

/**
 * How often to *evaluate* staleness. Deliberately much shorter than the
 * interval itself: when the timer period equals the staleness threshold, a few
 * milliseconds of timer drift make the tick land just before the threshold, it
 * skips, and the next opportunity is a whole period later — so a 15-minute
 * interval silently becomes 30. Polling a coarse condition on a fine timer
 * keeps the real spacing at ~15 min. Each tick is an integer comparison unless
 * a check is actually due, so this costs nothing.
 */
export const UPDATE_CHECK_POLL_MS = 60 * 1000;

export function UpdateProvider({ children }: { children: React.ReactNode }) {
  const DISMISSED_VERSION_KEY = "ccswitch:update:dismissedVersion";
  const LEGACY_DISMISSED_KEY = "dismissedUpdateVersion"; // 兼容旧键
  const LAST_CHECKED_KEY = "ccswitch:update:lastCheckedAt";

  const [hasUpdate, setHasUpdate] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastCheckedAt, setLastCheckedAt] = useState<number | null>(() => {
    const stored = Number(localStorage.getItem(LAST_CHECKED_KEY));
    return Number.isFinite(stored) && stored > 0 ? stored : null;
  });
  const [isDismissed, setIsDismissed] = useState(false);
  const [stagedVersion, setStagedVersion] = useState<string | null>(null);
  const [isStaging, setIsStaging] = useState(false);

  // 从 localStorage 读取已关闭的版本
  useEffect(() => {
    const current = updateInfo?.availableVersion;
    if (!current) return;

    // 读取新键；若不存在，尝试迁移旧键
    let dismissedVersion = localStorage.getItem(DISMISSED_VERSION_KEY);
    if (!dismissedVersion) {
      const legacy = localStorage.getItem(LEGACY_DISMISSED_KEY);
      if (legacy) {
        localStorage.setItem(DISMISSED_VERSION_KEY, legacy);
        localStorage.removeItem(LEGACY_DISMISSED_KEY);
        dismissedVersion = legacy;
      }
    }

    setIsDismissed(dismissedVersion === current);
  }, [updateInfo?.availableVersion]);

  const isCheckingRef = useRef(false);

  const checkUpdate = useCallback(async () => {
    if (!isTauri()) return false;
    if (isCheckingRef.current) return false;
    isCheckingRef.current = true;
    setIsChecking(true);
    setError(null);

    try {
      const result = await checkForUpdate({ timeout: 30000 });

      if (result.status === "available") {
        setHasUpdate(true);
        setUpdateInfo(result.info);

        // 检查是否已经关闭过这个版本的提醒
        let dismissedVersion = localStorage.getItem(DISMISSED_VERSION_KEY);
        if (!dismissedVersion) {
          const legacy = localStorage.getItem(LEGACY_DISMISSED_KEY);
          if (legacy) {
            localStorage.setItem(DISMISSED_VERSION_KEY, legacy);
            localStorage.removeItem(LEGACY_DISMISSED_KEY);
            dismissedVersion = legacy;
          }
        }
        setIsDismissed(dismissedVersion === result.info.availableVersion);
        const checkedAt = Date.now();
        setLastCheckedAt(checkedAt);
        localStorage.setItem(LAST_CHECKED_KEY, String(checkedAt));
        return true; // 有更新
      } else {
        setHasUpdate(false);
        setUpdateInfo(null);
        setIsDismissed(false);
        const checkedAt = Date.now();
        setLastCheckedAt(checkedAt);
        localStorage.setItem(LAST_CHECKED_KEY, String(checkedAt));
        return false; // 已是最新
      }
    } catch (err) {
      console.error("检查更新失败:", err);
      setError(err instanceof Error ? err.message : "检查更新失败");
      setHasUpdate(false);
      throw err; // 抛出错误让调用方处理
    } finally {
      setIsChecking(false);
      isCheckingRef.current = false;
    }
  }, []);

  const dismissUpdate = useCallback(() => {
    setIsDismissed(true);
    if (updateInfo?.availableVersion) {
      localStorage.setItem(DISMISSED_VERSION_KEY, updateInfo.availableVersion);
      // 清理旧键
      localStorage.removeItem(LEGACY_DISMISSED_KEY);
    }
  }, [updateInfo?.availableVersion]);

  const resetDismiss = useCallback(() => {
    setIsDismissed(false);
    localStorage.removeItem(DISMISSED_VERSION_KEY);
    localStorage.removeItem(LEGACY_DISMISSED_KEY);
  }, []);

  /**
   * Last version we kicked off staging for. The dependency array already stops
   * repeat runs for an unchanged version, so this exists for StrictMode's
   * deliberate double-invoke in dev — without it that fires two downloads.
   * (The backend would dedupe the second one, but not before a round trip.)
   */
  const stageAttemptedRef = useRef<string | null>(null);

  // Pre-download in the background as soon as a version is known, so pressing
  // 立即更新 installs immediately instead of waiting on a ~13 MB download.
  //
  // Deliberately not gated on isDismissed: dismissing means "stop nagging me",
  // not "never install". Staging anyway means the install is instant whenever
  // they do choose to. It costs one download per version, not per check.
  useEffect(() => {
    if (!isTauri()) return;
    const version = updateInfo?.availableVersion;
    if (!hasUpdate || !version) return;
    if (stagedVersion === version) return;
    if (stageAttemptedRef.current === version) return;

    stageAttemptedRef.current = version;
    let cancelled = false;
    setIsStaging(true);
    settingsApi
      .stageUpdateDownload()
      .then((staged) => {
        if (cancelled) return;
        setStagedVersion(staged);
      })
      .catch((err) => {
        // A failed pre-download is not user-facing: the install path falls back
        // to downloading normally, so this only costs the head start.
        console.error("预下载更新失败:", err);
      })
      .finally(() => {
        if (!cancelled) setIsStaging(false);
      });

    return () => {
      cancelled = true;
    };
  }, [hasUpdate, updateInfo?.availableVersion, stagedVersion]);

  // Check shortly after launch and periodically while the app remains open.
  useEffect(() => {
    if (!isTauri()) return;
    const checkIfDue = () => {
      if (document.visibilityState !== "visible") return;
      const stored = Number(localStorage.getItem(LAST_CHECKED_KEY));
      const lastSuccessfulCheck =
        Number.isFinite(stored) && stored > 0 ? stored : 0;
      if (Date.now() - lastSuccessfulCheck >= UPDATE_CHECK_INTERVAL_MS) {
        checkUpdate().catch(console.error);
      }
    };
    const timer = setTimeout(() => {
      checkUpdate().catch(console.error);
    }, 1000);
    const interval = window.setInterval(checkIfDue, UPDATE_CHECK_POLL_MS);
    window.addEventListener("focus", checkIfDue);
    document.addEventListener("visibilitychange", checkIfDue);

    return () => {
      clearTimeout(timer);
      window.clearInterval(interval);
      window.removeEventListener("focus", checkIfDue);
      document.removeEventListener("visibilitychange", checkIfDue);
    };
  }, [checkUpdate]);

  const value: UpdateContextValue = {
    hasUpdate,
    updateInfo,
    isChecking,
    error,
    lastCheckedAt,
    stagedVersion,
    isStaging,
    isDismissed,
    dismissUpdate,
    checkUpdate,
    resetDismiss,
  };

  return (
    <UpdateContext.Provider value={value}>{children}</UpdateContext.Provider>
  );
}

export function useUpdate() {
  return useContext(UpdateContext);
}
