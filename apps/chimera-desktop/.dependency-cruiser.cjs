/** @type {import('dependency-cruiser').IConfiguration} */
module.exports = {
  forbidden: [
    {
      name: "no-cross-feature-imports",
      comment: "Features must not import from sibling features. Use shared/ or shell/ for cross-cutting concerns.",
      severity: "error",
      from: { path: "^src/features/([^/]+)/" },
      to: {
        path: "^src/features/",
        pathNot: "^src/features/$1/",
      },
    },
    {
      name: "no-feature-imports-from-shell",
      comment: "Shell components (TopRail, etc.) must not import from features.",
      severity: "error",
      from: { path: "^src/shell/" },
      to: { path: "^src/features/" },
    },
    {
      name: "no-circular",
      comment: "Circular dependencies are banned.",
      severity: "error",
      from: {},
      to: { circular: true },
    },
  ],
  options: {
    doNotFollow: {
      path: "node_modules",
    },
    tsPreCompilationDeps: true,
    tsConfig: { fileName: "tsconfig.json" },
    reporterOptions: {
      text: { highlightFocused: true },
    },
  },
};
