export interface UpdateCheck {
  currentVersion: string | null;
  latestVersion: string;
  packageVersion: string;
  updateAvailable: boolean;
  source: "auto" | "mirror";
  installMode: "standard" | "portable";
  sizeBytes: number;
  releasedAt: string | null;
}

export interface RuntimeUpdateState {
  version: string | null;
  updateAvailable: boolean;
  updateVersion: string | null;
  updateMeta: string | null;
}

export function mergeUpdateCheck<T extends RuntimeUpdateState>(
  runtime: T,
  update: UpdateCheck,
): T {
  const sizeMb = Math.round(update.sizeBytes / 1024 / 1024);
  return {
    ...runtime,
    version: update.currentVersion ?? runtime.version,
    updateAvailable: update.updateAvailable,
    updateVersion: update.latestVersion,
    updateMeta: `${update.packageVersion} · ${sizeMb} MB · ${update.source}`,
  };
}

export function formatDownloadProgress(
  downloaded: number,
  total: number,
): { percent: number; label: string } {
  const percent = total > 0
    ? Math.max(0, Math.min(100, Math.round((downloaded / total) * 100)))
    : 0;
  return { percent, label: `${percent}%` };
}
