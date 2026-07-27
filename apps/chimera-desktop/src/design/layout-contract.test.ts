import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const settingsSource = readFileSync(new URL('../features/settings/index.tsx', import.meta.url), 'utf8');
const homeSource = readFileSync(new URL('../features/home/index.tsx', import.meta.url), 'utf8');
const codexSource = readFileSync(new URL('../features/codex/index.tsx', import.meta.url), 'utf8');

test('settings content consumes the available width without an empty third column', () => {
  assert.match(settingsSource, /className="settings-feature"/);
  assert.match(settingsSource, /className="settings-content"/);
});

test('long runtime versions and paths are constrained inside their cards', () => {
  assert.match(homeSource, /className="truncate-safe home-stat-value"/);
  assert.match(homeSource, /className="wrap-safe home-runtime-version"/);
  assert.match(codexSource, /className="wrap-safe codex-version"/);
  assert.match(codexSource, /className="wrap-safe codex-spec-value"/);
});
