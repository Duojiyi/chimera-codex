import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

describe("fixed-size desktop window", () => {
  it("keeps the main window fixed and non-maximizable", () => {
    const config = JSON.parse(
      readFileSync(resolve(process.cwd(), "src-tauri/tauri.conf.json"), "utf8"),
    ) as {
      app?: {
        windows?: Array<{
          width?: number;
          height?: number;
          minWidth?: number;
          minHeight?: number;
          maxWidth?: number;
          maxHeight?: number;
          resizable?: boolean;
          maximizable?: boolean;
          fullscreen?: boolean;
        }>;
      };
    };
    const main = config.app?.windows?.find((window) => window.width === 1140);

    expect(main).toMatchObject({
      width: 1140,
      height: 816,
      minWidth: 1140,
      minHeight: 816,
      maxWidth: 1140,
      maxHeight: 816,
      resizable: false,
      maximizable: false,
      fullscreen: false,
    });
  });
});
