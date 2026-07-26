//! Step 9.3 — remove secrets from anything that leaves the machine.
//!
//! Diagnostics exist so a user can send us something useful when Chimera
//! misbehaves, which makes them the one artifact designed to leave the
//! machine, and this the last place a credential can escape.
//!
//! Two properties shape the implementation, and both are tested:
//!
//! - **Idempotent.** The diagnostics path redacts twice — once when building
//!   the bundle and once on the preview the user approves. If the second pass
//!   changed anything the first already handled, what they approved would not
//!   be what gets sent.
//! - **Structure-preserving.** `C:\Users\<redacted>\AppData\...` still tells a
//!   support conversation where the file was. Replacing the whole path with a
//!   marker makes diagnostics useless, which makes people stop sending them —
//!   a slower route to the same place as leaking.
//!
//! Hand-written scanning rather than a regex crate: this runs on text that may
//! be megabytes of log, the patterns are simple, and adding a regex dependency
//! to the crate that decides what code runs next is not a trade worth making.

/// What a redacted value is replaced with. Fixed-width and obviously not data,
/// so nobody mistakes it for a truncated secret.
const MARK: &str = "[redacted]";

/// Characters that can appear inside a token. A secret ends at the first one
/// that cannot.
///
/// `+` is included for email local parts (`jane.doe+chimera@…`). Without it the
/// token before the `@` is only the part after the plus, and everything before
/// it — the actual name — survives redaction. No credential shape this module
/// recognises contains `+`: base64url has no `+`, and neither do `sk-`/`ghp_`
/// tokens, so widening the token alphabet cannot make those rules over-match.
fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '+'
}

/// Prefixes that make the rest of a token a credential by construction.
const TOKEN_PREFIXES: [&str; 4] = ["sk-", "ghp_", "gho_", "github_pat_"];

/// Minimum length before a prefixed token counts. Short enough to catch real
/// keys, long enough that a variable literally named `sk-` in a log does not
/// trip it.
const MIN_TOKEN_LEN: usize = 20;

/// Is this token a JWT? Three base64url segments separated by dots.
fn looks_like_jwt(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    parts.len() == 3
        && parts[0].starts_with("eyJ")
        && parts.iter().all(|p| {
            p.len() >= 8
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
}

fn looks_like_prefixed_token(token: &str) -> bool {
    TOKEN_PREFIXES.iter().any(|p| token.starts_with(p)) && token.len() >= MIN_TOKEN_LEN
}

/// Split into (token, separator) runs so replacement can rebuild the text with
/// its punctuation intact.
fn tokens(input: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let bytes: Vec<(usize, char)> = input.char_indices().collect();
    let mut i = 0;
    while i < bytes.len() {
        if is_token_char(bytes[i].1) {
            let start = bytes[i].0;
            let mut j = i;
            while j < bytes.len() && is_token_char(bytes[j].1) {
                j += 1;
            }
            let end = if j < bytes.len() {
                bytes[j].0
            } else {
                input.len()
            };
            spans.push((start, end));
            i = j;
        } else {
            i += 1;
        }
    }
    spans
}

/// Replace the user name inside a home-directory path, keeping the rest.
///
/// Handles `C:\Users\<name>\...`, `/Users/<name>/...` and `/home/<name>/...`.
/// The name is what identifies a person; the directories after it are what
/// make a support conversation possible.
fn redact_home_paths(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let lower = input.to_ascii_lowercase();
    let markers: [(&str, char); 3] = [("users\\", '\\'), ("users/", '/'), ("home/", '/')];

    let mut cursor = 0;
    while cursor < input.len() {
        // Find the earliest marker at or after the cursor.
        let hit = markers
            .iter()
            .filter_map(|(m, sep)| lower[cursor..].find(m).map(|i| (cursor + i, m.len(), *sep)))
            .min_by_key(|(i, _, _)| *i);

        let Some((at, marker_len, sep)) = hit else {
            break;
        };
        let name_start = at + marker_len;
        let name_end = input[name_start..]
            .find(sep)
            .map(|i| name_start + i)
            .unwrap_or(input.len());

        // An empty segment is not a username, and neither is one that already
        // holds the marker — that is what makes this idempotent.
        let name = &input[name_start..name_end];
        out.push_str(&input[cursor..name_start]);
        if name.is_empty() || name == MARK {
            out.push_str(name);
        } else {
            out.push_str(MARK);
        }
        cursor = name_end;
    }
    out.push_str(&input[cursor..]);
    out
}

/// Strip `user:pass@` from a URL, keeping the host.
fn redact_url_userinfo(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(rel) = input[cursor..].find("://") {
        let scheme_end = cursor + rel + 3;
        out.push_str(&input[cursor..scheme_end]);
        // Authority runs to the next '/', '?', '#' or whitespace.
        let authority_end = input[scheme_end..]
            .find(|c: char| c == '/' || c == '?' || c == '#' || c.is_whitespace())
            .map(|i| scheme_end + i)
            .unwrap_or(input.len());
        let authority = &input[scheme_end..authority_end];
        match authority.rfind('@') {
            Some(at) => {
                out.push_str(MARK);
                out.push('@');
                out.push_str(&authority[at + 1..]);
            }
            None => out.push_str(authority),
        }
        cursor = authority_end;
    }
    out.push_str(&input[cursor..]);
    out
}

fn redact_emails(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    for (start, end) in tokens(input) {
        let token = &input[start..end];
        // An email is a token, an '@', and another token containing a dot.
        let is_email_local = input[end..].starts_with('@');
        if !is_email_local {
            continue;
        }
        let domain_start = end + 1;
        let domain_end = tokens(&input[domain_start..])
            .first()
            .map(|(s, e)| (domain_start + s, domain_start + e));
        let Some((ds, de)) = domain_end else { continue };
        if ds != domain_start || !input[ds..de].contains('.') {
            continue;
        }
        if token == MARK {
            continue; // already redacted
        }
        out.push_str(&input[cursor..start]);
        out.push_str(MARK);
        cursor = de;
    }
    out.push_str(&input[cursor..]);
    out
}

fn redact_tokens(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    for (start, end) in tokens(input) {
        let token = &input[start..end];
        if !looks_like_prefixed_token(token) && !looks_like_jwt(token) {
            continue;
        }
        out.push_str(&input[cursor..start]);
        out.push_str(MARK);
        cursor = end;
    }
    out.push_str(&input[cursor..]);
    out
}

/// Remove every secret this knows how to recognise.
///
/// Order matters: URL userinfo first, because a password inside a URL is not a
/// token span on its own; then emails, whose `@` would otherwise be consumed;
/// then tokens; then home paths, which are the only rule that keeps part of
/// what it matched.
pub fn redact(input: &str) -> String {
    let s = redact_url_userinfo(input);
    let s = redact_emails(&s);
    let s = redact_tokens(&s);
    redact_home_paths(&s)
}

/// Would `redact` change this text?
///
/// The canary check in [`crate::diagnostics`] uses it, so it must agree with
/// `redact` exactly: a detector that saw something the redactor did not remove
/// would report a bundle as clean when it is not, and one that saw less would
/// let a leak through unreported.
pub fn contains_secret(input: &str) -> bool {
    redact(input) != input
}
