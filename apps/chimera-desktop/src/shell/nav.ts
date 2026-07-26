// Navigation vocabulary for the shell.
//
// This lives in shell/ rather than App.tsx so that TopRail can name the routes
// it renders without importing the app root. The reverse edge (shell → App)
// created a cycle that dependency-cruiser's no-circular rule rejects, and it
// inverted the layering: the rail is part of the shell, so the shell owns the
// route union and App consumes it.

export type ActiveFeature = "home" | "providers" | "codex" | "appearance" | "settings";

/** Every route, in rail order. The single source of truth for what is navigable. */
export const FEATURES: readonly ActiveFeature[] = [
  "home",
  "providers",
  "codex",
  "appearance",
  "settings",
] as const;
