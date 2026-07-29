import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "@/App";

describe("Chimera++ application shell", () => {
  it("exposes only the Codex product navigation", () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "供应商" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "供应商" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "更新" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "词元" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "外观" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "设置" })).toBeInTheDocument();
    expect(screen.queryByText("Gemini")).not.toBeInTheDocument();
    expect(screen.queryByText("Claude Code")).not.toBeInTheDocument();
    expect(screen.queryByText("OpenClaw")).not.toBeInTheDocument();
  });

  it("switches between the runtime, token, appearance, and settings surfaces", () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "更新" }));
    expect(
      screen.getByRole("heading", {
        name: "本机 Codex 已准备就绪",
        level: 1,
      }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "词元" }));
    expect(
      screen.getByRole("heading", { name: "词元消耗" }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "外观" }));
    expect(
      screen.getByRole("heading", { name: "皮肤市场" }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    expect(
      screen.getByRole("heading", { name: "保持简单，也保留控制权" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /^数据与日志/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "恢复默认设置" }),
    ).toBeInTheDocument();
  });
});
