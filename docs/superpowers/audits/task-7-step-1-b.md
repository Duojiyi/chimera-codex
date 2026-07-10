# Task 7 Step 1 Audit B — README rewrite (diff / boundary)

> Status: **PASS**
> Date: 2026-07-10
> Scope: Diff and residual-string boundary for root READMEs

## Diff surface

- Replaced full upstream marketing README with Chimera distribution docs.
- Retained functional topics: relay injection, data paths, FAQ, development commands.
- Intentionally keeps legacy `Codex++` / `Codex++ 管理工具` / `Codex++ Manager` only inside migration instructions.

## Boundary checks

| Risk | Check | Result |
|---|---|---|
| Promo URL leak | No `jojocode.com`, `Ad-List`, tip QR paths in README bodies | PASS |
| Brand placeholder | No `chimera-org`, `TBD`, `example owner` | PASS |
| Provider id confusion | Documents `CodexPlusPlus` as compatibility provider id only | PASS |
| Secret leak | No live keys | PASS |

## Decision

README diff is scoped to documentation; migration legacy names are documented, not product branding.
