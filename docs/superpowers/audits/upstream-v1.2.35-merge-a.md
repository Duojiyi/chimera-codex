# Upstream v1.2.35 Merge - Independent Audit A

## Scope

Requirements and observable-behavior review of the `v1.2.35` upstream merge, including
the reported Windows desktop entry defect.

## TDD and regression evidence

- NSIS shortcut Red failed because `Chimera++.lnk` targeted the silent launcher; Green
  targets `codex-plus-plus-manager.exe` and its icon.
- Repair-entry Red exposed that core `install_shortcuts` still targeted the launcher;
  Green uses explicit manager primary target/icon fields.
- Upstream latency User-Agent failed the no-upstream-brand gate; a wire-level test and
  implementation now require `ChimeraPlusPlus/<version>`.
- Installer regression 30/30, Windows/workflow contracts 45/45, environment detection
  4/4, latency 2/2, TypeScript, formatting and branding gates passed.

## Conclusion

PASS. The merge accepts the upstream environment diagnostics while retaining Chimera
branding, no-promotion/About/GitHub boundaries, customer configuration and updater
behavior. Version is `1.2.35-chimera.1`; macOS build number is 3. Real installation
and Windows icon-cache behavior remain Task 16 acceptance work.
