import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { NewSettingsView } from "@/ChimeraApp";

const { checkUpdateMock, installUpdateAndRestartMock, toastInfoMock } =
  vi.hoisted(() => ({
    checkUpdateMock: vi.fn(),
    installUpdateAndRestartMock: vi.fn(),
    toastInfoMock: vi.fn(),
  }));

vi.mock("@/contexts/UpdateContext", () => ({
  useUpdate: () => ({
    hasUpdate: true,
    updateInfo: {
      currentVersion: "2.0.12",
      availableVersion: "2.0.13",
      notes: "第一项修复\n第二项修复",
    },
    isChecking: false,
    error: null,
    lastCheckedAt: Date.now(),
    checkUpdate: checkUpdateMock,
  }),
}));

vi.mock("@/lib/api/settings", () => ({
  settingsApi: {
    installUpdateAndRestart: installUpdateAndRestartMock,
  },
}));

vi.mock("sonner", () => ({
  toast: {
    info: toastInfoMock,
    error: vi.fn(),
    success: vi.fn(),
  },
}));

describe("settings application update", () => {
  it("recovers when an advertised update is no longer available", async () => {
    installUpdateAndRestartMock.mockResolvedValueOnce(false);
    checkUpdateMock.mockResolvedValueOnce(false);
    render(<NewSettingsView />);

    const disclosure = screen.getByRole("button", { name: "查看更新" });
    expect(disclosure).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(disclosure);
    expect(disclosure).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText(/第一项修复/)).toHaveTextContent(
      "第一项修复 第二项修复",
    );

    fireEvent.click(screen.getByRole("button", { name: "立即更新" }));

    await waitFor(() => {
      expect(installUpdateAndRestartMock).toHaveBeenCalledTimes(1);
      expect(checkUpdateMock).toHaveBeenCalledTimes(1);
      expect(screen.queryByRole("button", { name: "立即更新" })).toBeNull();
    });
    expect(toastInfoMock).toHaveBeenCalledWith("该更新已不可用", {
      description: "正在重新检查更新",
    });
  });
});
