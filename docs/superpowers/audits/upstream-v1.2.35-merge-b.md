# Upstream v1.2.35 Merge - Independent Audit B

## Scope

Independent diff, conflict-resolution, security, packaging and regression review. The
final conclusion was reached without relying on audit A.

## Findings and remediation

- Initial FAIL: NSIS was fixed but core repair still recreated the primary shortcut
  against the launcher. Remediated and locked by the entrypoint-plan test.
- Second FAIL: upstream latency code restored `CodexPlusPlus/` in the User-Agent.
  Remediated through the branding truth source, an HTTP case-insensitive header-name
  test with exact value comparison, and the production no-promotion scanner.
- Final tree contains no unresolved merge stages, sponsor/QR resurrection or upstream
  customer README. Chimera icon blobs remain distinct from upstream and pass the
  deterministic icon gate.
- Cargo/npm/Tauri versions and locks agree on `1.2.35-chimera.1`; build number is 3.
  The local Tauri `Cargo.toml` CRLF phantom is not staged.

## Conclusion

PASS AFTER REMEDIATION. No code or release-automation blocker remains. Windows upgrade
shortcut behavior, Explorer icon caching and unsigned macOS installation still require
real-machine Task 16 verification and are not claimed here.
