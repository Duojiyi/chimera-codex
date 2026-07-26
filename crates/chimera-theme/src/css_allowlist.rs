//! CSS property allowlist — Step 8.1 (ADR-005).
//!
//! Deny-by-default: a property, at-rule, or value shape that is not
//! explicitly permitted is refused. This is not a sanitizer that strips the
//! offending bit and keeps the rest — any single violation refuses the whole
//! stylesheet, because a partially-applied theme with silently dropped rules
//! is a worse failure mode to debug than an import that was simply rejected.
//!
//! Three independent hazards this module exists to close, all through the
//! same mechanism (an explicit allowlist plus a `url()` scanner), rather than
//! three separate special cases:
//!   - `@import` / any other at-rule can load a remote stylesheet.
//!   - `url()` can point at a remote host, an absolute filesystem path, or
//!     (via `javascript:`/`data:`) carry active content.
//!   - `expression()` is IE's CSS-to-script escape hatch; dead in modern
//!     browsers but free to reject and a reasonable canary for "this
//!     stylesheet was not written for CSS the way we ship it".

use std::collections::HashSet;
use thiserror::Error;

/// Declaration properties a skin may set. Anything else is refused.
///
/// Deliberately excludes `position`, `top`/`right`/`bottom`/`left`, and
/// `z-index`: those are exactly what would let a skin pin an element over
/// Codex's real UI (a phishing-adjacent overlay), not a "did we forget a
/// color property" oversight.
const ALLOWED_PROPERTIES: &[&str] = &[
    "color",
    "background",
    "background-color",
    "background-image",
    "background-position",
    "background-repeat",
    "background-size",
    "border",
    "border-color",
    "border-width",
    "border-style",
    "border-radius",
    "border-top",
    "border-bottom",
    "border-left",
    "border-right",
    "padding",
    "margin",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "line-height",
    "letter-spacing",
    "text-align",
    "text-decoration",
    "text-transform",
    "box-shadow",
    "opacity",
    "display",
    "content",
    "width",
    "height",
    "max-width",
    "max-height",
    "min-width",
    "min-height",
    "overflow",
    "overflow-x",
    "overflow-y",
    "gap",
    "flex",
    "flex-direction",
    "align-items",
    "justify-content",
    "transition",
    "cursor",
];

/// Why a stylesheet was refused.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CssError {
    #[error("unbalanced braces in stylesheet")]
    UnbalancedBraces,
    #[error("nested rule blocks are refused")]
    NestedBlockRefused,
    #[error("at-rules are refused: {0:?}")]
    AtRuleRefused(String),
    #[error("property is not on the allowlist: {0}")]
    DisallowedProperty(String),
    #[error("url() must reference a bundled package asset by relative path, got {0:?}")]
    UnbundledUrl(String),
    #[error("value contains a known CSS-to-script vector: {0:?}")]
    ScriptVector(String),
    #[error("value contains a CSS escape sequence, which is not permitted in a skin: {0:?}")]
    EscapeRefused(String),
}

/// Validate a stylesheet against the allowlist.
///
/// `bundled_assets` is the exact set of relative, forward-slash paths that
/// were actually extracted from the `.codexskin` package (see
/// [`crate::package`]) — a `url()` is accepted only when it names one of
/// these paths verbatim after normalisation, never by pattern.
pub fn validate_css(css: &str, bundled_assets: &HashSet<String>) -> Result<(), CssError> {
    let stripped = strip_comments(css);
    let chars: Vec<char> = stripped.chars().collect();

    let mut depth: i32 = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut block_start = 0usize;
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];

        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }

        match c {
            '"' | '\'' => quote = Some(c),
            '@' => return Err(CssError::AtRuleRefused(snippet(&chars, i))),
            '{' => {
                depth += 1;
                if depth > 1 {
                    return Err(CssError::NestedBlockRefused);
                }
                block_start = i + 1;
            }
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return Err(CssError::UnbalancedBraces);
                }
                if depth == 0 {
                    let body: String = chars[block_start..i].iter().collect();
                    validate_declarations(&body, bundled_assets)?;
                }
            }
            _ => {}
        }
        i += 1;
    }

    // A trailing open quote or open brace both mean the tokenizer never saw
    // the file the way a real CSS parser would — refused rather than
    // interpreted best-effort.
    if depth != 0 || quote.is_some() {
        return Err(CssError::UnbalancedBraces);
    }
    Ok(())
}

/// A short, printable window around an offending index, for error messages.
/// Bounded so a huge stylesheet cannot make the error message itself huge.
fn snippet(chars: &[char], at: usize) -> String {
    let end = (at + 24).min(chars.len());
    chars[at..end].iter().collect()
}

/// Remove `/* ... */` comments, leaving quoted strings untouched so a
/// stylesheet cannot hide a comment terminator inside `content: "*/"` to
/// desync a naive scanner.
fn strip_comments(css: &str) -> String {
    let chars: Vec<char> = css.chars().collect();
    let mut out = String::with_capacity(css.len());
    let mut i = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;

    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            out.push(c);
            i += 1;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            out.push(' '); // keep tokens on either side from gluing together
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Split a rule body into individual `prop: value` declarations on top-level
/// `;`, respecting quoted strings so `content: ";"` is not mistaken for a
/// declaration terminator.
fn split_top_level_semicolons(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for c in body.chars() {
        if let Some(q) = quote {
            current.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            current.push(c);
            continue;
        }
        if c == ';' {
            out.push(std::mem::take(&mut current));
            continue;
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        out.push(current);
    }
    out
}

fn validate_declarations(body: &str, bundled_assets: &HashSet<String>) -> Result<(), CssError> {
    for decl in split_top_level_semicolons(body) {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let mut parts = decl.splitn(2, ':');
        let prop = parts.next().unwrap_or("").trim().to_ascii_lowercase();
        let value = parts.next().unwrap_or("").trim();
        if prop.is_empty() || value.is_empty() {
            return Err(CssError::DisallowedProperty(decl.to_string()));
        }
        if !ALLOWED_PROPERTIES.contains(&prop.as_str()) {
            return Err(CssError::DisallowedProperty(prop));
        }

        // Refuse escape sequences outright, before anything else looks at the
        // value.
        //
        // An adversarial review reproduced this end to end: the url() scanner
        // searched for the literal bytes `url(`, but CSS's escaped-code-point
        // grammar means `\75rl(` decodes to `url(` in every standards-compliant
        // tokenizer — including the Chromium engine this crate injects into —
        // so the argument never reached the check that rejects remote schemes.
        // A skin could load remote content, which G9 forbids outright.
        //
        // Teaching the scanner to decode escapes would be an arms race
        // (`\75`, `\000075`, `\75 rl(`, mixed case, escapes inside the argument
        // itself) against a tokenizer whose behaviour is not ours to define,
        // and we would only ever be one spelling behind. A skin has no
        // legitimate need for an escape sequence, so this removes the surface
        // instead of chasing it. That is the whole point of an allowlist.
        if value.contains('\\') {
            return Err(CssError::EscapeRefused(value.to_string()));
        }

        let lower_value = value.to_ascii_lowercase();
        if lower_value.contains("expression(") {
            return Err(CssError::ScriptVector(value.to_string()));
        }

        for arg in extract_url_args(value) {
            validate_url_arg(&arg, bundled_assets)?;
        }
    }
    Ok(())
}

/// Pull every `url(...)` argument out of a declaration value, in order.
fn extract_url_args(value: &str) -> Vec<String> {
    let chars: Vec<char> = value.chars().collect();
    let lower: Vec<char> = value.to_ascii_lowercase().chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;

    while i + 4 <= lower.len() {
        if lower[i..i + 4] == ['u', 'r', 'l', '('] {
            let mut j = i + 4;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            let quote = match chars.get(j) {
                Some('"') | Some('\'') => {
                    let q = chars[j];
                    j += 1;
                    Some(q)
                }
                _ => None,
            };
            let start = j;
            match quote {
                Some(q) => {
                    while j < chars.len() && chars[j] != q {
                        j += 1;
                    }
                }
                None => {
                    while j < chars.len() && chars[j] != ')' {
                        j += 1;
                    }
                }
            }
            let arg: String = chars[start..j.min(chars.len())].iter().collect();
            out.push(arg.trim().to_string());
            i = j + 1;
            continue;
        }
        i += 1;
    }
    out
}

/// A `url()` argument is accepted only if it is a bare relative path that,
/// after normalisation, is exactly one of the package's own bundled assets.
///
/// Every one of these checks exists for a real bypass otherwise: a scheme
/// (`https:`, `javascript:`, `data:`) is banned outright rather than
/// allow-listed, because deciding "which data: URIs are safe" is not a
/// tractable problem and a bundled asset already covers the legitimate case.
fn validate_url_arg(raw: &str, bundled_assets: &HashSet<String>) -> Result<(), CssError> {
    let refuse = || Err(CssError::UnbundledUrl(raw.to_string()));

    if raw.is_empty() {
        return refuse();
    }
    if raw.contains(':') {
        // Catches every scheme: http:, https:, javascript:, data:, and a
        // Windows drive letter (`C:\...`) alike.
        return refuse();
    }
    if raw.starts_with("//") {
        return refuse(); // protocol-relative
    }
    if raw.starts_with('/') || raw.starts_with('\\') {
        return refuse(); // absolute path, POSIX or Windows-style
    }
    if raw.contains("..") {
        return refuse(); // traversal, regardless of where it lands
    }
    if raw.contains('\\') {
        return refuse(); // backslash anywhere is not a zip-internal path
    }

    if bundled_assets.contains(raw) {
        Ok(())
    } else {
        refuse()
    }
}
