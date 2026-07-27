import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const appSource = readFileSync(new URL('../App.tsx', import.meta.url), 'utf8');
const styles = readFileSync(new URL('../styles.css', import.meta.url), 'utf8');
const tauriConfig = JSON.parse(
  readFileSync(new URL('../../src-tauri/tauri.conf.json', import.meta.url), 'utf8'),
) as { app: { windows: Array<{ decorations?: boolean }> } };

test('the custom title bar owns real close, minimize, and maximize controls', () => {
  assert.match(appSource, /getCurrentWindow/);
  assert.match(appSource, /data-tauri-drag-region/);
  assert.match(appSource, /window-control-close/);
  assert.match(appSource, /window-control-minimize/);
  assert.match(appSource, /window-control-maximize/);
  assert.equal(tauriConfig.app.windows[0]?.decorations, false);
});

test('the shipped window is full bleed instead of a framed mockup', () => {
  assert.match(styles, /\.app-canvas\s*{[^}]*padding:\s*0/s);
  assert.match(styles, /\.app-window\s*{[^}]*border-radius:\s*0/s);
});

test('window controls have accessible targets around familiar 12px traffic-light dots', () => {
  assert.match(styles, /\.window-control\s*{[^}]*width:\s*28px[^}]*height:\s*28px/s);
  assert.match(styles, /\.window-control::before\s*{[^}]*width:\s*12px[^}]*height:\s*12px[^}]*border-radius:\s*50%/s);
});
