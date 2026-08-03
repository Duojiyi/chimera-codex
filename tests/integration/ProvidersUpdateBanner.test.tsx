/**
 * Integration test — Bug 2: update banner visibility in the providers view.
 *
 * Verifies that when an update is available and not yet dismissed,
 * NewProvidersView renders the banner with the correct version string,
 * and that the "稍后" / "立即更新" buttons behave correctly.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { NewProvidersView } from "@/ChimeraApp";
import type { Provider } from "@/types";

// ---------------------------------------------------------------------------
// Mock UpdateContext
// ---------------------------------------------------------------------------
const dismissUpdateMock = vi.fn();
const { useUpdateMock } = vi.hoisted(() => ({
  useUpdateMock: vi.fn(),
}));

vi.mock("@/contexts/UpdateContext", () => ({
  useUpdate: () => useUpdateMock(),
}));

// ---------------------------------------------------------------------------
// Shared minimal props for NewProvidersView
// ---------------------------------------------------------------------------
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

function makeProps(overrides: Partial<Parameters<typeof NewProvidersView>[0]> = {}) {
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
    onNavigate: vi.fn(),
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
describe("NewProvidersView — update banner (Bug 2)", () => {
  it("shows banner when update is available and not dismissed", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: false,
      updateInfo: { availableVersion: "2.1.4", currentVersion: "2.1.3" },
      dismissUpdate: dismissUpdateMock,
    });

    render(<NewProvidersView {...makeProps()} />);

    const banner = screen.getByRole("status");
    expect(banner).toBeInTheDocument();
    expect(banner).toHaveTextContent("Chimera++ 2.1.4 可用");
    expect(banner).toHaveTextContent("已通过签名验证，更新后将自动重启");
    expect(screen.getByRole("button", { name: "稍后" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "立即更新" }),
    ).toBeInTheDocument();
  });

  it("does not show banner when hasUpdate is false", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: false,
      isDismissed: false,
      updateInfo: null,
      dismissUpdate: dismissUpdateMock,
    });

    render(<NewProvidersView {...makeProps()} />);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("does not show banner when isDismissed is true", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: true,
      updateInfo: { availableVersion: "2.1.4", currentVersion: "2.1.3" },
      dismissUpdate: dismissUpdateMock,
    });

    render(<NewProvidersView {...makeProps()} />);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("does not show banner when updateInfo is null even if hasUpdate is true", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: false,
      updateInfo: null,
      dismissUpdate: dismissUpdateMock,
    });

    render(<NewProvidersView {...makeProps()} />);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("calls dismissUpdate when 稍后 is clicked", () => {
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: false,
      updateInfo: { availableVersion: "2.1.4", currentVersion: "2.1.3" },
      dismissUpdate: dismissUpdateMock,
    });

    render(<NewProvidersView {...makeProps()} />);
    fireEvent.click(screen.getByRole("button", { name: "稍后" }));
    expect(dismissUpdateMock).toHaveBeenCalledOnce();
  });

  it("calls dismissUpdate AND onNavigate('settings') when 立即更新 is clicked", () => {
    const onNavigate = vi.fn();
    useUpdateMock.mockReturnValue({
      hasUpdate: true,
      isDismissed: false,
      updateInfo: { availableVersion: "2.1.4", currentVersion: "2.1.3" },
      dismissUpdate: dismissUpdateMock,
    });

    render(<NewProvidersView {...makeProps({ onNavigate })} />);
    fireEvent.click(screen.getByRole("button", { name: "立即更新" }));
    expect(dismissUpdateMock).toHaveBeenCalledOnce();
    expect(onNavigate).toHaveBeenCalledWith("settings");
  });
});
