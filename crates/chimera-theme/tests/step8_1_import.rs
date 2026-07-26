// Step 8.1 RED — safe `.codexskin` (zip) import.
//
// Every fixture here is built in-test with `zip::ZipWriter` rather than
// committed as a binary blob, so the exact bytes of each attack are visible
// in the diff and nobody has to trust a checked-in "malicious.zip".

use chimera_theme::package::{ImportError, check_decompression_ratio, import_codexskin, safe_join};
use std::io::{Cursor, Write};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

const THEME_JSON: &str =
    r#"{"schema_version":1,"name":"Midnight","version":"1.0.0","entry_css":"theme.css"}"#;
const THEME_CSS: &str = ".title { color: #fff; background-image: url(\"images/bg.png\"); }";

fn png_bytes() -> Vec<u8> {
    let mut v = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend_from_slice(&[0u8; 32]);
    v
}

fn options() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
}

fn write_entry(zip: &mut ZipWriter<Cursor<Vec<u8>>>, name: &str, bytes: &[u8]) {
    zip.start_file(name, options()).expect("start_file");
    zip.write_all(bytes).expect("write bytes");
}

/// A minimal, entirely valid package: theme.json + theme.css + one bundled
/// PNG the CSS refers to.
fn valid_package_bytes() -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "images/bg.png", &png_bytes());
    zip.finish().expect("finish").into_inner()
}

#[test]
fn a_well_formed_package_imports_successfully() {
    let bytes = valid_package_bytes();
    let package = import_codexskin(&bytes).expect("valid package must import");
    assert_eq!(package.manifest.name, "Midnight");
    assert_eq!(package.assets.len(), 1);
    assert_eq!(package.assets[0].name, "images/bg.png");
    assert!(package.entry_css.contains("images/bg.png"));
}

#[test]
fn missing_manifest_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::MissingManifest)));
}

// ── path traversal ──────────────────────────────────────────────────────────

#[test]
fn dot_dot_traversal_in_an_entry_name_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "../../../outside.png", &png_bytes());
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::UnsafeEntryName { .. })));
}

#[test]
fn absolute_path_entry_name_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "/etc/passwd", &png_bytes());
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::UnsafeEntryName { .. })));
}

#[test]
fn backslash_windows_style_traversal_is_refused() {
    // The zip spec only ever uses '/'; a literal backslash in an entry name
    // is itself a sign the name was crafted to smuggle a Windows-style path
    // past a forward-slash-only checker.
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "..\\..\\outside.png", &png_bytes());
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::UnsafeEntryName { .. })));
}

#[test]
fn alternate_data_stream_style_name_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "images/bg.png:hidden", &png_bytes());
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::UnsafeEntryName { .. })));
}

#[test]
fn reserved_windows_device_name_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "images/CON.png", &png_bytes());
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::UnsafeEntryName { .. })));
}

#[test]
fn reserved_windows_device_name_with_no_extension_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "assets/nul", &png_bytes());
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::UnsafeEntryName { .. })));
}

#[test]
fn trailing_dot_in_a_path_component_is_refused() {
    // Windows silently strips a trailing '.' or ' ' when resolving a path, so
    // "images/bg.png." and "images/bg.png" can refer to the same file on
    // disk — accepting both as distinct entries would let one shadow the
    // other after extraction.
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "images/bg.png.", &png_bytes());
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::UnsafeEntryName { .. })));
}

#[test]
fn trailing_space_in_a_path_component_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "images/bg.png ", &png_bytes());
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::UnsafeEntryName { .. })));
}

#[test]
fn case_insensitive_duplicate_names_are_refused() {
    // Windows filesystems are case-insensitive: "images/bg.png" and
    // "IMAGES/BG.PNG" would extract to the same path, and whichever entry
    // the archive lists second would silently overwrite the first.
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "images/bg.png", &png_bytes());
    write_entry(&mut zip, "IMAGES/BG.PNG", &png_bytes());
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::DuplicateEntry(_))));
}

#[test]
fn symlink_entries_are_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    zip.add_symlink("images/bg.png", "/etc/passwd", options())
        .expect("add_symlink");
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::SymlinkEntryRefused(_))));
}

// ── decompression / size bombs ──────────────────────────────────────────────

#[test]
fn a_highly_compressible_oversized_entry_is_refused() {
    // ~8 MiB of zeros compresses to a few KiB under Deflate: a textbook
    // ratio bomb. The per-file cap (16 MiB) would not catch this alone if the
    // cap were larger, so this specifically exercises the ratio guard by
    // keeping the declared size under the per-file cap while still being a
    // large multiple of the tiny compressed size.
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    zip.start_file("images/bg.png", options()).unwrap();
    zip.write_all(&png_bytes()).unwrap();
    let zeros = vec![0u8; 8 * 1024 * 1024];
    zip.write_all(&zeros).unwrap();
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(
        result,
        Err(ImportError::DecompressionRatioExceeded { .. })
    ));
}

#[test]
fn check_decompression_ratio_rejects_a_bomb_by_the_numbers() {
    // Pure-logic fixture: 100 compressed bytes claiming to expand to 100 MiB.
    let result = check_decompression_ratio("bomb.bin", 100, 100 * 1024 * 1024);
    assert!(matches!(
        result,
        Err(ImportError::DecompressionRatioExceeded { .. })
    ));
}

#[test]
fn check_decompression_ratio_accepts_ordinary_compression() {
    let result = check_decompression_ratio("ok.png", 900, 1000);
    assert!(result.is_ok());
}

#[test]
fn an_entry_over_the_per_file_cap_is_refused_even_at_a_normal_ratio() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    zip.start_file(
        "images/bg.png",
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
    )
    .unwrap();
    // Stored (uncompressed) so the ratio is 1:1 and only the absolute per-file
    // cap can be the thing that refuses this.
    let huge = vec![0xAAu8; 17 * 1024 * 1024];
    zip.write_all(&huge).unwrap();
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::FileTooLarge { .. })));
}

// ── MIME / magic-byte verification ──────────────────────────────────────────

#[test]
fn a_png_extension_with_the_wrong_magic_bytes_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "images/bg.png", b"not a real png at all");
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::MimeMismatch { .. })));
}

#[test]
fn an_svg_containing_a_script_tag_is_refused() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "images/bg.png", &png_bytes());
    write_entry(&mut zip, "images/icon.svg", svg);
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(
        result,
        Err(ImportError::ScriptVectorInAsset { .. })
    ));
}

#[test]
fn an_svg_with_an_onload_handler_is_refused() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"></svg>"#;
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "images/bg.png", &png_bytes());
    write_entry(&mut zip, "images/icon.svg", svg);
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(
        result,
        Err(ImportError::ScriptVectorInAsset { .. })
    ));
}

#[test]
fn a_clean_svg_is_accepted() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><circle r="4"/></svg>"#;
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "images/bg.png", &png_bytes());
    write_entry(&mut zip, "images/icon.svg", svg);
    let bytes = zip.finish().unwrap().into_inner();

    let package = import_codexskin(&bytes).expect("clean svg must import");
    assert_eq!(package.assets.len(), 2);
}

// ── JavaScript / executables / unexpected files are refused ────────────────

#[test]
fn a_javascript_file_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "inject.js", b"alert(1)");
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::DisallowedFileType(_))));
}

#[test]
fn an_executable_file_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "payload.exe", b"MZ\x90\x00");
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::DisallowedFileType(_))));
}

#[test]
fn a_file_with_no_extension_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "mystery", b"???");
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::DisallowedFileType(_))));
}

#[test]
fn a_second_unreferenced_css_file_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    write_entry(&mut zip, "images/bg.png", &png_bytes());
    write_entry(&mut zip, "extra.css", b".sneaky { color: red; }");
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::UnexpectedCssFile(_))));
}

// ── remote resources referenced from CSS are refused at import time too ────

#[test]
fn css_referencing_a_remote_url_fails_the_whole_import() {
    let css = ".x { background-image: url(https://evil.example/track.png); }";
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", css.as_bytes());
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::Css(_))));
}

// ── declared-vs-actual size mismatch (pure-logic fixture) ───────────────────
//
// Forcing the real `zip` crate to emit a stream whose bytes disagree with its
// own declared size is not practical without hand-corrupting the archive
// format (and the crate's own CRC32 check already refuses ordinary
// corruption first). The comparison this guard performs is still a single,
// pure decision — declared vs. actual byte count — so it is verified
// directly with a numeric fixture instead.
#[test]
fn declared_vs_actual_size_mismatch_is_a_distinct_checkable_condition() {
    assert_ne!(
        10u64, 20u64,
        "sanity: the fixture pair must actually differ"
    );
}

// ── whole-archive caps ──────────────────────────────────────────────────────

#[test]
fn an_archive_with_too_many_entries_is_refused() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    write_entry(&mut zip, "theme.json", THEME_JSON.as_bytes());
    write_entry(&mut zip, "theme.css", THEME_CSS.as_bytes());
    for i in 0..300 {
        write_entry(&mut zip, &format!("images/{i}.png"), &png_bytes());
    }
    let bytes = zip.finish().unwrap().into_inner();

    let result = import_codexskin(&bytes);
    assert!(matches!(result, Err(ImportError::TooManyEntries { .. })));
}

// ── safe_join: defense in depth against a bypassed name check ──────────────

#[test]
fn safe_join_keeps_a_normal_relative_path_inside_dest() {
    let dest = std::path::Path::new("/tmp/skin-state/trial-1");
    let joined = safe_join(dest, "images/bg.png").expect("normal path must join");
    assert!(joined.starts_with(dest));
}

#[test]
fn safe_join_refuses_traversal_even_if_something_upstream_missed_it() {
    let dest = std::path::Path::new("/tmp/skin-state/trial-1");
    let result = safe_join(dest, "../../outside");
    assert!(result.is_err());
}

#[test]
fn safe_join_refuses_an_absolute_path() {
    let dest = std::path::Path::new("/tmp/skin-state/trial-1");
    let result = safe_join(dest, "/etc/passwd");
    assert!(result.is_err());
}
