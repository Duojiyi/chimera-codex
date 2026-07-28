import type { Provider } from "@/types";
import {
  extractCodexBaseUrl,
  extractCodexModelName,
} from "@/utils/providerConfigUtils";

export type ConnectionState =
  | { kind: "unknown"; message: string }
  | { kind: "checking"; message: string }
  | { kind: "connected"; message: string; modelCount: number }
  | { kind: "error"; message: string };

export interface OperationRecord {
  id: string;
  timestamp: number;
  provider: string;
  action: string;
  result: "success" | "error" | "skipped";
  durationMs?: number;
  detail?: string;
}

export interface CurrentProviderResolution {
  provider: Provider | null;
  source: "live" | "stored" | "external" | "none";
}

const ACTIVITY_KEY = "chimera-plus-plus:activity:v2";
const MAX_ACTIVITY_RECORDS = 200;

function normalizeEndpoint(value: string | null | undefined): string {
  return (value ?? "").trim().replace(/\/+$/, "").toLocaleLowerCase("en-US");
}

function liveConfigText(live: unknown): string {
  if (!live || typeof live !== "object") return "";
  const config = (live as Record<string, unknown>).config;
  return typeof config === "string" ? config : "";
}

export function resolveCurrentProvider(
  providers: Provider[],
  storedId: string,
  live: unknown,
  liveReadSucceeded: boolean,
): CurrentProviderResolution {
  if (!providers.length) return { provider: null, source: "none" };

  const stored = providers.find((provider) => provider.id === storedId) ?? null;
  if (!liveReadSucceeded) {
    return stored
      ? { provider: stored, source: "stored" }
      : { provider: null, source: "external" };
  }

  const config = liveConfigText(live);
  const liveEndpoint = normalizeEndpoint(extractCodexBaseUrl(config));
  const liveModel = extractCodexModelName(config) ?? "";

  const exact = providers.find((provider) => {
    const candidate = String(provider.settingsConfig?.config ?? "");
    const endpoint = normalizeEndpoint(extractCodexBaseUrl(candidate));
    const model = extractCodexModelName(candidate) ?? "";
    return (
      endpoint === liveEndpoint && (!liveModel || !model || model === liveModel)
    );
  });
  if (exact) return { provider: exact, source: "live" };

  if (!liveEndpoint && stored) {
    const storedEndpoint = normalizeEndpoint(
      extractCodexBaseUrl(String(stored.settingsConfig?.config ?? "")),
    );
    if (!storedEndpoint) return { provider: stored, source: "stored" };
  }

  return { provider: null, source: "external" };
}

function isOperationRecord(value: unknown): value is OperationRecord {
  if (!value || typeof value !== "object") return false;
  const record = value as Partial<OperationRecord>;
  return (
    typeof record.id === "string" &&
    typeof record.timestamp === "number" &&
    typeof record.provider === "string" &&
    typeof record.action === "string" &&
    (record.result === "success" ||
      record.result === "error" ||
      record.result === "skipped")
  );
}

export function loadOperationRecords(
  storage: Pick<Storage, "getItem"> = window.localStorage,
): OperationRecord[] {
  try {
    const raw = storage.getItem(ACTIVITY_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed)
      ? parsed.filter(isOperationRecord).slice(0, MAX_ACTIVITY_RECORDS)
      : [];
  } catch {
    return [];
  }
}

export function saveOperationRecords(
  records: OperationRecord[],
  storage: Pick<Storage, "setItem"> = window.localStorage,
): OperationRecord[] {
  const normalized = records
    .filter(isOperationRecord)
    .sort((left, right) => right.timestamp - left.timestamp)
    .slice(0, MAX_ACTIVITY_RECORDS);
  storage.setItem(ACTIVITY_KEY, JSON.stringify(normalized));
  return normalized;
}

export function formatDuration(durationMs?: number): string {
  if (durationMs == null || durationMs < 0) return "-";
  if (durationMs < 1000) return `${Math.round(durationMs)}ms`;
  return `${(durationMs / 1000).toFixed(1)}s`;
}

export function formatVersion(value: string | null | undefined): string {
  return value?.trim() || "未检测到";
}
