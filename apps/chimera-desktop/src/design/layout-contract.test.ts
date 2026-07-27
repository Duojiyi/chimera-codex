import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const settingsSource = readFileSync(new URL('../features/settings/index.tsx', import.meta.url), 'utf8');
const homeSource = readFileSync(new URL('../features/home/index.tsx', import.meta.url), 'utf8');
const codexSource = readFileSync(new URL('../features/codex/index.tsx', import.meta.url), 'utf8');
const providersSource = readFileSync(new URL('../features/providers/index.tsx', import.meta.url), 'utf8');

test('settings content consumes the available width without an empty third column', () => {
  assert.match(settingsSource, /className="settings-feature"/);
  assert.match(settingsSource, /className="settings-content"/);
});

test('long runtime versions and paths are constrained inside their cards', () => {
  assert.match(homeSource, /className="truncate-safe home-stat-value"/);
  assert.match(homeSource, /className="truncate-safe home-runtime-version"[^>]*title={version}/);
  assert.match(codexSource, /className="truncate-safe codex-version"[^>]*title={versionLabel}/);
  assert.match(codexSource, /className="wrap-safe codex-spec-value"/);
});

test('destructive buttons use AA-safe text on the pale danger background', () => {
  for (const source of [settingsSource, providersSource]) {
    assert.doesNotMatch(source, /background:\s*color\.dangerBg[^}]*color:\s*color\.danger(?:[,\s}])/s);
  }
});
