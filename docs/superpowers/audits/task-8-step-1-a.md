# Task 8 Step 1 Audit A — Trigger / idempotent gate (requirements)

> Status: **PASS**
> Date: 2026-07-10
> Scope: `release-assets.yml` push main + workflow_dispatch; Cargo version; tag gate; concurrency

## Requirements checklist

| Requirement | Evidence | Result |
|---|---|---|
| Trigger is `push: main` + `workflow_dispatch` | `on.push.branches=[main]`, `workflow_dispatch` present; no `release: published` | PASS |
| Read Cargo workspace version | `resolve-version` awk on `[workspace.package].version` | PASS |
| Existing `v<version>` tag → idempotent skip | `git ls-remote --tags origin refs/tags/$tag` → `should_publish=false` | PASS |
| Same-version concurrency | `concurrency.group: chimera-release-${{ github.repository }}-main`, `cancel-in-progress: false` | PASS |
| No publish before builds | Build/publish jobs gated on `should_publish == 'true'` | PASS |

## Decision

Step 1 requirements met.
