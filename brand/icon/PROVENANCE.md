# Chimera++ Icon Provenance

## Creation

- Created: 2026-07-12
- Design workflow: Codex `logo-designer` skill
- Model: GPT-5
- Format: original, self-contained SVG icon with a `512 x 512` viewBox
- License for the Chimera++ original icon: AGPL-3.0-only with this repository

## Brief

Create a minimal geometric application icon for Chimera++, a desktop launcher and management tool for ChimeraHub customers. The icon must communicate an enhanced entry point, remain legible at 16 and 32 pixels, work on light and dark surfaces, use the product green (`#31DD84`) with neutral dark and warm-white fills, and retain transparent outer corners. It must not resemble or imply affiliation with OpenAI, ChatGPT, Codex, or another third-party product.

## Input Boundary

OpenAI/ChatGPT/Codex assets were not used as inputs. No third-party bitmap, vector path, logo file, screenshot pixels, font outline, or extracted application icon was supplied to the concept generators or copied into this repository. Repository context was limited to the Chimera++ product name, purpose, existing UI color tokens, required platform sizes, and the written non-similarity constraint.

## Concepts And Selection

Three SVG concepts were created independently in `brand/icon/concepts/`: a negative-space monogram, modular convergence, and a gateway prism. The rendered comparison is retained in `brand/icon/preview.html`.

`concept-1-monogram.svg` was selected and refined as `iterations/iteration-1.svg` for `brand/icon/logo.svg`. It produced the clearest silhouette, used the fewest elements, and had the largest visual distance from the prohibited third-party marks. The refinement enlarged the plus from 88 to 112 SVG units and its stem from 40 to 48 units. Two further pixel-grid trials are retained as iterations 2 and 3. At the actual 16-pixel ICO size, every plus that stayed safely inside the C simplified to a green center block; enlarging it enough to retain four distinct arms caused the mark to merge with the C. The final therefore preserves a crisp C with a green enhancement accent at 16 pixels and the complete C+ reading from 32 pixels upward. The user delegated AI generation and instructed Codex to complete verification and the baseline release, so Codex made the final selection using those objective criteria.

## Export

All distributed PNG and ICO files are generated from `brand/icon/logo.svg` with the repository's pinned Tauri CLI. The primary PNG is 1024 x 1024 with transparent outer corners. The ICO contains at least 16, 32, 48, and 256 pixel entries. `scripts/verify-brand-icons.ps1` verifies the SVG restrictions, dimensions, transparency, byte-identical copies, resource references, provenance markers, and removal of the previous icon hashes.
