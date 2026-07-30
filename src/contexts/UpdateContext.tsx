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
import { isTauri } from "@tauri-apps/api/core";

interface UpdateContextValue {
  // 更新状态
  hasUpdate: boolean;
  updateInfo: UpdateInfo | null;
  isChecking: boolean;
  error: string | null;
  lastCheckedAt: number | null;

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
  isDismissed: false,
  dismissUpdate: () => undefined,
  checkUpdate: async () => false,
  resetDismiss: () => undefined,
};

const UpdateContext = createContext<UpdateContextValue>(DEFAULT_UPDATE_CONTEXT);

const UPDATE_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;

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
    const interval = window.setInterval(checkIfDue, UPDATE_CHECK_INTERVAL_MS);
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
