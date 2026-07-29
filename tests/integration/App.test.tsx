import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "@/App";

describe("Chimera++ application shell", () => {
  it("exposes only the Codex product navigation", () => {
    render(<App />);

    expect(
      screen.getByRole("heading", { name: "路由门" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "路由门" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "运行时" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "词元" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "外观" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "设置" })).toBeInTheDocument();
    expect(screen.queryByText("Gemini")).not.toBeInTheDocument();
    expect(screen.queryByText("Claude Code")).not.toBeInTheDocument();
    expect(screen.queryByText("OpenClaw")).not.toBeInTheDocument();
  });

  it("switches between the runtime, token, appearance, and settings surfaces", () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "运行时" }));
    expect(
      screen.getByRole("heading", { name: "运行时", level: 1 }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "词元" }));
    expect(
      screen.getByRole("heading", { name: "词元" }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "外观" }));
    expect(screen.getByRole("heading", { name: "外观" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    expect(screen.getByRole("heading", { name: "设置" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /^检查 Codex 更新/ }),
    ).toBeInTheDocument();
  });
});
