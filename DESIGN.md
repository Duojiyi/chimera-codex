---
version: alpha
name: Chimera++ Precision Desktop
description: A restrained, high-confidence Windows desktop system for Codex connection and runtime management.
colors:
  canvas: "oklch(0.975 0.004 260)"
  surface: "oklch(0.995 0.002 260)"
  surface-raised: "#FFFFFF"
  surface-subtle: "oklch(0.955 0.008 260)"
  ink: "oklch(0.205 0.018 260)"
  ink-muted: "oklch(0.48 0.015 260)"
  line: "oklch(0.88 0.008 260)"
  sidebar: "oklch(0.18 0.018 260)"
  sidebar-raised: "oklch(0.245 0.018 260)"
  accent: "oklch(0.61 0.205 32)"
  accent-hover: "oklch(0.55 0.205 32)"
  accent-soft: "oklch(0.94 0.04 32)"
  success: "oklch(0.62 0.13 165)"
  success-soft: "oklch(0.94 0.035 165)"
  warning: "oklch(0.73 0.15 75)"
  error: "oklch(0.56 0.20 28)"
  focus: "oklch(0.56 0.16 240)"
typography:
  title-lg:
    fontFamily: "Inter, Microsoft YaHei UI, Segoe UI, sans-serif"
    fontSize: 24px
    fontWeight: 650
    lineHeight: 1.25
    letterSpacing: 0px
  title-md:
    fontFamily: "Inter, Microsoft YaHei UI, Segoe UI, sans-serif"
    fontSize: 18px
    fontWeight: 650
    lineHeight: 1.35
    letterSpacing: 0px
  body-md:
    fontFamily: "Inter, Microsoft YaHei UI, Segoe UI, sans-serif"
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: 0px
  body-sm:
    fontFamily: "Inter, Microsoft YaHei UI, Segoe UI, sans-serif"
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.45
    letterSpacing: 0px
  label-md:
    fontFamily: "Inter, Microsoft YaHei UI, Segoe UI, sans-serif"
    fontSize: 13px
    fontWeight: 550
    lineHeight: 1.3
    letterSpacing: 0px
  data-md:
    fontFamily: "JetBrains Mono, Cascadia Mono, Consolas, monospace"
    fontSize: 13px
    fontWeight: 500
    lineHeight: 1.4
    letterSpacing: 0px
rounded:
  xs: 4px
  sm: 6px
  md: 8px
  lg: 12px
  full: 9999px
spacing:
  unit: 4px
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
  xxl: 32px
  shell-gutter: 28px
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "#FFFFFF"
    rounded: "{rounded.sm}"
    height: 36px
  button-secondary:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    height: 36px
  nav-active:
    backgroundColor: "{colors.sidebar-raised}"
    textColor: "#FFFFFF"
    rounded: "{rounded.sm}"
  input-default:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    height: 40px
---

# Chimera++ Design System

## Overview

Chimera++ is a precision desktop utility, not a consumer dashboard. The visual experience should feel intentional from the first second: calm neutral surfaces, confident typography, selective signal-red actions, and crisp state changes. The reference composition is a connection console: users should immediately recognize what Codex is using, whether it is healthy, and what they can change.

Use a light workspace and a dark graphite navigation rail. The color accent is a signal, not decoration. Keep the interface dense enough for repeated work, but give the current connection and its next action enough space to be read at a glance. Avoid beige, blue-slate monoculture, gradients, glass effects, glowing ornaments, and generic metric-card grids.

## Colors

- **Canvas:** `canvas` is a cool near-neutral workspace background. It provides separation without making every region a card.
- **Surfaces:** `surface` and `surface-raised` create a maximum of three tonal layers. Use borders only where they explain containment or interaction.
- **Signal red:** `accent` identifies the one primary action on a screen, the active provider route, and critical confirmation states. It is never a page background or a decorative gradient.
- **Semantic states:** success is green, warning is amber, error is red. Status text and icons must accompany color.
- **Sidebar:** dark graphite is structural, not a second brand color. Selected navigation uses `sidebar-raised`; do not use a colored stripe or a glow.

## Typography

Use Inter with `Microsoft YaHei UI` / `Segoe UI` fallback for Chinese Windows installations. UI text must not drop below 13px. Reserve the monospace face for URLs, model IDs, versions, file paths, and diagnostic output; it must not become the general visual voice.

Page titles use `title-lg`; panel titles use `title-md`. Labels, button text, and navigation all use `label-md`. Body copy uses `body-md` or `body-sm`. Avoid uppercase English section kickers and wide tracking; Chinese interface labels should stay compact and natural.

## Layout

The desktop shell has a Windows-native title area, a scrollable workspace with `28px` outer gutters, and a floating pill navigation centred at the bottom of the workspace carrying the six destinations. There is no persistent sidebar: the earlier `208px` rail was replaced by this bottom bar when the v2 route interface landed, and v2.2.0 removed the leftover variant CSS. `.chimera-sidebar` now survives only as a `display: none` rule with no remaining references. The active page owns scrolling. Content should align to a 4px spacing system.

The window itself is frameless and transparent with the OS shadow disabled, so the shell's own 1px ring is the only thing separating the app from the desktop. That ring must be a translucent ink, not an opaque near-white: the divider tone sits at 1.26:1 against a white backdrop and disappears, while the shipped `--shell-edge` measures 2.00:1 there and still composites correctly over a dark one.

The control console uses a two-column desktop layout: a primary route area and a compact inspection panel. The active connection is the first visual object, not a small card buried in a grid. Provider switching is a dense list or a destination selector; provider editing opens inline or in a focused sheet.

The tools page groups health scan, connection test, backup, config transfer, and folder access into operational sections. Historical events are secondary and appear beneath useful actions, never as a standalone top-level page.

## Elevation & Depth

Use tonal layers and one-pixel borders for normal surfaces. Do not combine decorative borders with large soft shadows. Only sheets, menus, and dialogs may use elevation: `0 8px 24px rgba(20, 24, 32, 0.14)`. Modal backdrops use a solid translucent ink layer; no blur is required.

Liquid glass is reserved for three surfaces: the floating bottom navigation, the provider switcher, and the provider line rail. It is not frosted glass: keep the transmitted content clear, use edge refraction, a narrow specular highlight, subtle saturation/contrast, and physical press deformation. Do not use broad blur, cloudy translucent panels, or glass cards elsewhere in the product. Unsupported environments fall back to the same translucent fill and crisp edge treatment without filtering.

## Shapes

The shipped radius scale is `4px` / `8px` / `12px` / `16px`. Use the small end for controls (buttons land at `7px`), `8px` for panels, `12px` for large dialogs and preview frames, and `16px` for the window shell itself. Pills are reserved for compact state tags and for the floating bottom navigation. Windows window controls must use correct Windows behavior and recognizable symbols; never imitate macOS traffic lights.

## Components

- **Connection hero:** displays provider, model, endpoint health, and one primary action. It has a restrained status indicator, never a radar, threat overlay, or network-topology visualization. One deliberate exception, added in v2.3.0: a dotted globe (`RouteGlobe`) sits behind the route area. It is scoped tightly — geometry from a 110m Natural Earth land mask, dots tinted from the active palette, a static halo, no routes, arcs, markers, or scanning motion, and no threat semantics. It spins slowly and honours `prefers-reduced-motion` in JS (slow / freeze / spin, default slow, re-read on preference change rather than once at setup), so the blanket CSS reduced-motion rule is not what carries it. Where WebGL is unavailable it degrades to a pure-CSS dotted disc in the same colours.
- **Provider destination list:** each row contains provider name, endpoint domain, selected model, and connection state. Selection changes the inspector with a 180ms crossfade and 4px horizontal settle.
- **Buttons:** primary is signal-red; secondary is bordered white; tertiary is icon-only with a tooltip. Every button has default, hover, pressed, focus, disabled, loading, and error recovery states.
- **Inputs:** labels stay above inputs. URL, key, and model fields have clear helper text. Key reveal, model picker, folder picker, and test connection use familiar icon actions with tooltips.
- **Segmented controls:** use for stable versus portable distribution and automatic versus manual update source. They do not resize on selection.
- **Tool rows:** health scan, backup, import/export, and open-folder actions use a title, one concise explanation, current status, and an icon action.
- **Dialogs and sheets:** destructive maintenance actions always show scope, consequence, and an explicit confirm action. Configuration forms prefer a right-side sheet or inline inspector over a modal stack.
- **Empty states:** teach the next useful action in one sentence and one command. Do not use decorative illustrations or numbered marketing steps.

## Motion

Motion is a first-class interaction contract. It is short, physical, and tied to state. Default easing is `cubic-bezier(0.22, 1, 0.36, 1)`; never use bounce or elastic easing. Respect `prefers-reduced-motion` by reducing motion to an instant or 100ms opacity transition.

| Event | Motion | Duration |
| --- | --- | --- |
| Page switch | Content crossfade with 8px vertical settle; sidebar stays fixed | 180ms |
| Provider selection | Selected row gains tonal fill; inspector crossfades and settles 4px | 180ms |
| Connection test | Button label becomes progress state; status dot pulses once only while pending | 160ms in, state-driven |
| Successful save | Inline success line fades and a checkmark draws once | 220ms |
| Failed action | Error block expands from its trigger; focus moves to recovery action | 180ms |
| Download/update | Determinate progress bar advances continuously; stage text changes without layout shift | state-driven |
| Dialog/sheet | Backdrop fades; surface translates 8px and fades in | 180ms |
| Skin selection | List selection changes immediately; preview crossfades without scrolling the inspector | 200ms |

No page-load choreography, looping glows, parallax, or universal staggered entrances. Animations must never hide essential content before they run.

## Do's and Don'ts

- Do show the active provider and model before secondary configuration.
- Do use one clear primary action per screen and keep the rest visually quiet.
- Do maintain 4.5:1 contrast for normal text and a visible keyboard focus ring.
- Do make loading, success, error, and disabled states explicit in the design before implementation.
- Don't build screens from repeated same-size metric cards.
- Don't use upstream author names, sponsor marks, provider advertising, or repository labels in the customer interface.
- Don't hide expert settings, but don't put them in the beginner flow.
- Don't use fake macOS controls, gradient page backgrounds, frosted-glass cards, or large rounded rectangles as decoration.
- Don't make a full page scroll just to change a skin or switch a provider.
