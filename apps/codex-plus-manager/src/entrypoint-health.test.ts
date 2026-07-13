import assert from "node:assert";
import { describe, it } from "node:test";
import { entrypointHealthRows } from "./entrypoint-health.ts";

describe("entrypoint health rows", () => {
  it("单桌面主入口同时承载管理工具时只显示一行", () => {
    const rows = entrypointHealthRows({
      single_entrypoint: true,
      silent_shortcut: {
        status: "installed",
        path: "C:\\Users\\A\\Desktop\\Chimera++.lnk",
      },
      management_shortcut: {
        status: "installed",
        path: "C:\\Users\\A\\Desktop\\Chimera++.lnk",
      },
    });

    assert.deepStrictEqual(rows, [
      {
        kind: "primary",
        status: "installed",
        path: "C:\\Users\\A\\Desktop\\Chimera++.lnk",
      },
    ]);
  });

  it("独立主入口和管理工具入口仍分别显示", () => {
    const rows = entrypointHealthRows({
      single_entrypoint: false,
      silent_shortcut: {
        status: "installed",
        path: "/Applications/Chimera++.app",
      },
      management_shortcut: {
        status: "installed",
        path: "/Applications/Chimera++ 管理工具.app",
      },
    });

    assert.deepStrictEqual(
      rows.map((row) => row.kind),
      ["primary", "management"],
    );
  });

  it("未检查时只显示主入口占位，避免短暂出现已废弃的双桌面模型", () => {
    assert.deepStrictEqual(entrypointHealthRows(null), [
      { kind: "primary", status: "not_checked", path: null },
    ]);
  });

  it("两个独立入口都没有候选路径时仍保留两行", () => {
    const rows = entrypointHealthRows({
      single_entrypoint: false,
      silent_shortcut: { status: "missing", path: null },
      management_shortcut: { status: "missing", path: null },
    });

    assert.deepStrictEqual(
      rows.map((row) => row.kind),
      ["primary", "management"],
    );
  });

  it("同一路径但健康状态不一致时不得去重", () => {
    const rows = entrypointHealthRows({
      single_entrypoint: false,
      silent_shortcut: {
        status: "installed",
        path: "C:\\Users\\A\\Desktop\\Chimera++.lnk",
      },
      management_shortcut: {
        status: "missing",
        path: "C:\\Users\\A\\Desktop\\Chimera++.lnk",
      },
    });

    assert.deepStrictEqual(
      rows.map((row) => row.kind),
      ["primary", "management"],
    );
  });

  it("单入口平台即使无法解析候选路径也只显示主入口", () => {
    const rows = entrypointHealthRows({
      single_entrypoint: true,
      silent_shortcut: { status: "missing", path: null },
      management_shortcut: { status: "missing", path: null },
    });

    assert.deepStrictEqual(rows, [
      { kind: "primary", status: "missing", path: null },
    ]);
  });
});
