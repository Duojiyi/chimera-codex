# Installer legacy cleanup allowlist remediation - Independent audit B

Date: 2026-07-13

Independence: reviewed only the diff, exact allowlist matching, boundaries, and
regression surface without consulting audit A.

- Diff size is exactly eight deleted and eight added lines; only `lineNumber`
  changes.
- All eight affected numbers increase by ten, matching the preceding NSIS
  insertion. The four earlier NSIS entries remain unchanged.
- All twelve NSIS allowlist entries match the current source path, line number,
  pattern, and complete line with zero mismatches.
- No path, pattern, source content, or reason was widened; the scanner passes.

PASS. No unrelated change or advertising-removal regression is hidden by this
remediation.
