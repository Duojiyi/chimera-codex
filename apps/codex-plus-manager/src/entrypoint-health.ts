export type EntrypointPathState = {
  status: string;
  path: string | null;
};

export type EntrypointHealthOverview = {
  single_entrypoint: boolean;
  silent_shortcut: EntrypointPathState;
  management_shortcut: EntrypointPathState;
};

export type EntrypointHealthRow = EntrypointPathState & {
  kind: "primary" | "management";
};

export function entrypointHealthRows(
  overview: EntrypointHealthOverview | null,
): EntrypointHealthRow[] {
  if (!overview) {
    return [{ kind: "primary", status: "not_checked", path: null }];
  }

  const primary: EntrypointHealthRow = {
    kind: "primary",
    ...overview.silent_shortcut,
  };
  const management: EntrypointHealthRow = {
    kind: "management",
    ...overview.management_shortcut,
  };

  if (overview.single_entrypoint) {
    return [primary];
  }

  return [primary, management];
}
