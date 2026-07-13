# Windows legacy cleanup idempotency - Independent audit A

Date: 2026-07-13

Independence: reviewed the field symptom, requested behavior, TDD contract, and
observable installer paths without consulting audit B.

## Findings

- Legacy key absent: enumeration ends at `install_complete`; installation is
  idempotently successful.
- Legacy key present and deletable: the found branch deletes it and completes.
- Legacy key deletion fails: `install_legacy_cleanup_failed` remains fatal.
- The regression test covers the complete ordered probe and delete behavior.
- Cargo/npm/Tauri versions are `1.2.35-chimera.2`; macOS build number is `4`.

The first PowerShell 5.1 branding check was investigated and identified as a
no-BOM UTF-8 decoding false positive. Re-running with the workflow runtime,
PowerShell 7, passes. No generated-file drift remains.

## Conclusion

PASS. No local-scope blocker remains. Cloud builds and Task 16 real-machine
installation remain outside this audit.
