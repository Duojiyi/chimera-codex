import assert from "node:assert/strict";
import test from "node:test";

import { formatDownloadProgress, mergeUpdateCheck } from "./updateState.ts";

test("real update check replaces placeholder runtime update state", () => {
  const merged = mergeUpdateCheck(
    { version: "26.721.31836", updateAvailable: false, updateVersion: null, updateMeta: null },
    {
      currentVersion: "26.721.31836",
      latestVersion: "26.721.41059",
      packageVersion: "26.721.4979.0",
      updateAvailable: true,
      source: "mirror",
      installMode: "standard",
      sizeBytes: 744080244,
      releasedAt: "2026-07-24T21:33:02Z",
    },
  );

  assert.equal(merged.version, "26.721.31836");
  assert.equal(merged.updateAvailable, true);
  assert.equal(merged.updateVersion, "26.721.41059");
  assert.match(merged.updateMeta ?? "", /26\.721\.4979\.0/);
});

test("download progress is bounded and stable", () => {
  assert.deepEqual(formatDownloadProgress(0, 100), { percent: 0, label: "0%" });
  assert.deepEqual(formatDownloadProgress(45, 100), { percent: 45, label: "45%" });
  assert.deepEqual(formatDownloadProgress(120, 100), { percent: 100, label: "100%" });
  assert.deepEqual(formatDownloadProgress(1, 0), { percent: 0, label: "0%" });
});
