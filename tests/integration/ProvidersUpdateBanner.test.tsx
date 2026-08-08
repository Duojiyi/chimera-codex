import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { NewProvidersView } from "@/ChimeraApp";
import type { Provider } from "@/types";

const dismissUpdateMock = vi.fn();
const installUpdateMock = vi.fn().mockResolvedValue(true);
const { useUpdateMock } = vi.hoisted(() => ({
  useUpdateMock: vi.fn(),
}));

vi.mock("@/contexts/UpdateContext", () => ({
  useUpdate: () => useUpdateMock(),
}));

const mockProvider: Provider = {
  id: "relay-1",
  name: "ChimeraHub Relay",
  settingsConfig: {
    config:
      'model_provider = "custom"\n[model_providers.custom]\nbase_url = "https://api.chimerahub.org/v1"\n',
  },
  category: "third_party",
  sortIndex: 0,
  createdAt: Date.now(),
};

function makeProps(
  overrides: Partial<Parameters<typeof NewProvidersView>[0]> = {},
) {
  return {
    providers: [mockProvider],
    currentId: mockProvider.id,
    currentSource: "live" as const,
    connection: { kind: "unknown" as const, message: "" },
    loading: false,
    codexProcess: null,
    launchingCodex: false,
    restartRequired: false,
    onOpenCodex: vi.fn().mockResolvedValue(undefined),
    onSwitch: vi.fn().mockResolvedValue(undefined),
    onEdit: vi.fn(),
    onAdd: vi.fn(),
    ...overrides,
  };
}

describe("providers update banner", () => {
  it("shows the verified update banner with a direct install action", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: false,
      updateInfo: { availableVersion: "2.1.4", currentVersion: "2.1.3" },
      dismissUpdate: dismissUpdateMock,
      installUpdate: installUpdateMock,
      isInstalling: false,
      downloadProgress: null,
    });

    render(<NewProvidersView {...makeProps()} />);

    const banner = screen.getByRole("status");
    expect(banner).toHaveTextContent("Chimera++ 2.1.4 \u53ef\u7528");
    expect(banner).toHaveTextContent(
      "\u5df2\u901a\u8fc7\u7b7e\u540d\u9a8c\u8bc1\uff0c\u66f4\u65b0\u540e\u5c06\u81ea\u52a8\u91cd\u542f\u3002",
    );
    expect(
      screen.getByRole("button", { name: /\u7a0d\u540e/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /\u4e0b\u8f7d\u5e76\u5b89\u88c5/ }),
    ).toBeInTheDocument();
  });

  it("does not show the banner when no update is available", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: false,
      isDismissed: false,
      updateInfo: null,
      dismissUpdate: dismissUpdateMock,
    });

    render(<NewProvidersView {...makeProps()} />);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("does not show the banner after the update was dismissed", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: true,
      updateInfo: { availableVersion: "2.1.4", currentVersion: "2.1.3" },
      dismissUpdate: dismissUpdateMock,
    });

    render(<NewProvidersView {...makeProps()} />);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("does not show the banner without update metadata", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: false,
      updateInfo: null,
      dismissUpdate: dismissUpdateMock,
    });

    render(<NewProvidersView {...makeProps()} />);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("dismisses the banner when the user chooses to defer it", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: false,
      updateInfo: { availableVersion: "2.1.4", currentVersion: "2.1.3" },
      dismissUpdate: dismissUpdateMock,
    });

    render(<NewProvidersView {...makeProps()} />);
    fireEvent.click(screen.getByRole("button", { name: /\u7a0d\u540e/ }));
    expect(dismissUpdateMock).toHaveBeenCalledOnce();
  });

  it("starts installation without dismissing the update", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: false,
      updateInfo: { availableVersion: "2.1.4", currentVersion: "2.1.3" },
      dismissUpdate: dismissUpdateMock,
      installUpdate: installUpdateMock,
      isInstalling: false,
      downloadProgress: null,
    });

    render(<NewProvidersView {...makeProps()} />);
    fireEvent.click(
      screen.getByRole("button", { name: /\u4e0b\u8f7d\u5e76\u5b89\u88c5/ }),
    );
    expect(dismissUpdateMock).not.toHaveBeenCalled();
    expect(installUpdateMock).toHaveBeenCalledOnce();
  });

  it("says the staged package is ready to install", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: false,
      updateInfo: { availableVersion: "2.1.4", currentVersion: "2.1.3" },
      dismissUpdate: dismissUpdateMock,
      stagedVersion: "2.1.4",
      installUpdate: installUpdateMock,
      isInstalling: false,
      downloadProgress: null,
    });

    render(<NewProvidersView {...makeProps()} />);
    expect(screen.getByRole("status")).toHaveTextContent(
      "\u5b89\u88c5\u5305\u5df2\u5728\u540e\u53f0\u4e0b\u8f7d\u5b8c\u6bd5",
    );
    expect(
      screen.getByRole("button", { name: /\u5b89\u88c5\u5e76\u91cd\u542f/ }),
    ).toBeInTheDocument();
  });

  it("disables the action and shows progress while installing", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: false,
      updateInfo: { availableVersion: "2.1.4", currentVersion: "2.1.3" },
      dismissUpdate: dismissUpdateMock,
      installUpdate: installUpdateMock,
      isInstalling: true,
      downloadProgress: { downloaded: 60, total: 100 },
    });

    render(<NewProvidersView {...makeProps()} />);

    expect(screen.getByRole("status")).toHaveTextContent("正在下载 60%");
    expect(screen.getByRole("button", { name: /正在更新…/ })).toBeDisabled();
  });

  it("does not advertise a stale staged package as ready", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: false,
      updateInfo: { availableVersion: "2.1.4", currentVersion: "2.1.3" },
      dismissUpdate: dismissUpdateMock,
      stagedVersion: "2.1.3",
    });

    render(<NewProvidersView {...makeProps()} />);
    const banner = screen.getByRole("status");
    expect(banner).not.toHaveTextContent(
      "\u5b89\u88c5\u5305\u5df2\u5728\u540e\u53f0\u4e0b\u8f7d\u5b8c\u6bd5",
    );
    expect(banner).toHaveTextContent(
      "\u5df2\u901a\u8fc7\u7b7e\u540d\u9a8c\u8bc1",
    );
  });
});
