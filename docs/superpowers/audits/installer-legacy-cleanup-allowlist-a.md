# Installer legacy cleanup allowlist remediation - Independent audit A

Date: 2026-07-13

Independence: reviewed cloud run `29230762055`, the ad-removal requirement, and
observable scanner behavior without consulting audit B.

- The cloud Red consists of eight unmatched current lines and eight unused old
  entries after the NSIS hotfix inserted ten lines.
- Exactly eight `lineNumber` values move by ten. Path, pattern, exact line, and
  reason are unchanged.
- All entries remain limited to transactional snapshot, cleanup, and rollback
  of legacy `Codex++ 管理工具` desktop and Start Menu shortcuts.
- The same PowerShell 7 scanner used by CI returns
  `verify-no-upstream-ads: OK`.

PASS. The 16 fail-closed findings are closed without broadening the allowlist.
