# Installer transaction idempotency - Independent audit A

Date: 2026-07-13

Independence: reviewed the field symptoms, requirements, TDD evidence, and
observable installer behavior without consulting audit B.

- End-of-enumeration now exits on either an error flag or an empty key name.
- An exact legacy-key match still performs whole-key deletion; a real deletion
  failure remains fatal.
- Missing or non-empty URL protocol keys no longer fail uninstall housekeeping.
- Program files, shortcuts, and owned registry values retain fatal error checks.
- Desktop `Chimera++` still opens the manager; launcher and manager remain in the
  Start Menu.
- Version surfaces agree on `1.2.35-chimera.3`; macOS build number is `5`.

PASS. Cloud compilation and real Windows installation/uninstallation remain
outside this local audit.
