# Task 2b Step 1 — Audit A

**Status:** pass

## Evidence
- Plan: `DEFAULT_MARKET_INDEX_URL` 置空；`fetch_market_manifest` 空 URL 返回空 manifest。
- `script_market.rs` 已实现；测试 `empty_default_market_url_returns_empty_manifest_without_network` 通过。

## Open issues
- 无
