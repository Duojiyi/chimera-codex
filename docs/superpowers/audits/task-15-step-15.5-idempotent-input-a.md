# Task 15 Step 15.5 Idempotent Tag Input - Audit A

## TDD evidence

- Red: requested `v3.9.0` still selected latest `v3.10.0`; workflow had no input chain.
- Additional Red: empty/missing candidates and unquoted empty input exposed failure-path
  and parameter-binding gaps.
- Green: an optional manual tag is selected only from formal upstream Releases; empty
  input preserves latest selection. Draft, prerelease, non-SemVer and missing tags all
  produce code 4/action error through the production failure path.
- Sync contracts, formatting and Windows/workflow contracts passed (45/45).

## Independent conclusion

PASS. The scheduled behavior is unchanged, the manual validation path is fail-closed,
and the active workflow block passes the selected tag as one quoted argument. Commented
and wrong-step mutation probes fail. No remediation remains.
