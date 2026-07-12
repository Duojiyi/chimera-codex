# Task 8 Step 4 Audit A — Final publish job (requirements)

> Status: **PASS**
> Date: 2026-07-10
> Scope: Download artifacts → SHA-256/size → draft → upload → publish; anonymous smoke

## Requirements checklist

| Requirement | Evidence | Result |
|---|---|---|
| Download all platform artifacts | Three `download-artifact` steps into `release-assets/` | PASS |
| SHA-256 + size per asset | Node crypto hash + `buf.length` into `latest.json` | PASS |
| `latest.json` schema | `version`, `url`, `body`, `assets[{name,url,sha256,size}]` | PASS |
| Draft on current main SHA | `gh release create --draft --target $GITHUB_SHA` | PASS |
| Upload assets + manifest | `gh release upload` ChimeraCodex-* + `latest.json` | PASS |
| Publish only after upload | `gh release edit --draft=false --prerelease=false` | PASS |
| Failure keeps draft / latest untouched | Draft created first; edit only after upload success | PASS |
| Anonymous smoke | curl `.../latest/download/latest.json` + range GET setup.exe | PASS |
| macOS not notarized note | Release body states ad-hoc / not notarized | PASS |

## Decision

Step 4 requirements met.
