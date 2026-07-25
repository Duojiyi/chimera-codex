# ADR-004: No Local Protocol Proxy in MVP

**Status:** Accepted
**Date:** 2026-07-26

## Context

Codex CLI consumes the OpenAI Responses API. Several AI vendors (e.g., Anthropic, Google, Mistral) expose only a Chat Completions-compatible endpoint rather than a Responses API endpoint. One way to bridge this gap would be to run a local proxy process that listens on a loopback port, translates Chat Completions requests to Responses API format (or vice versa), and forwards them to the vendor.

This approach introduces meaningful costs:

- A resident proxy process adds operational complexity: it must be started, kept alive, and stopped alongside the application.
- The proxy becomes an additional attack surface. Any process listening on a local port can be targeted by other local processes or malicious browser extensions.
- The API key must be held by the proxy in memory and potentially written to a config file it manages, creating additional secret exposure paths.
- Debugging failures becomes harder because the request chain now has an extra hop with its own error modes.
- Users may incorrectly assume the proxy provides encryption or key isolation guarantees that it does not actually provide.

## Decision

v2.0.0 ships with no local listening proxy of any kind.

Vendor support in v2.0.0 is limited to vendors that natively implement the OpenAI Responses API. The API key for the active vendor is written directly into `config.toml` in the format Codex expects. This means the active key may exist in plain text on disk in a user-readable location.

The UI and documentation must state this plainly. No claim of end-to-end encryption, key isolation, or secure storage may be made for the v2.0.0 key handling path. Users must be informed that their key is stored as plain text and advised to apply appropriate filesystem permissions.

## Consequences

**Vendor compatibility** is narrowed to Responses API-native providers. Vendors that offer only Chat Completions endpoints cannot be used in v2.0.0.

**No hidden resident process.** There is no background proxy daemon started by the application. The process footprint is limited to Codex itself.

**Honest security posture.** Documentation and UI must accurately describe key storage. Users can make informed decisions about where they run the tool and how they protect their config file.

**Deferred complexity.** Protocol translation logic does not need to be designed, tested, or maintained for the MVP release.

## Future Work

A Chat Completions to Responses API translation layer can be added as a discrete, opt-in capability in a future version. That work should carry its own threat model document covering proxy authentication, key handling within the proxy process, and the trust boundary between the proxy and the rest of the application. It should not be retrofitted onto the v2.0.0 architecture without that analysis.
