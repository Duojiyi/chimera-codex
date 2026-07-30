import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { UpdateProvider, useUpdate } from "@/contexts/UpdateContext";

const { checkForUpdateMock } = vi.hoisted(() => ({
  checkForUpdateMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
}));

vi.mock("@/lib/updater", () => ({
  checkForUpdate: checkForUpdateMock,
}));

function UpdateProbe() {
  const { lastCheckedAt } = useUpdate();
  return <output>{lastCheckedAt ?? "never"}</output>;
}

describe("UpdateProvider periodic checks", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-31T00:00:00Z"));
    localStorage.clear();
    checkForUpdateMock.mockResolvedValue({ status: "up-to-date" });
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("checks again when a hidden app becomes visible after the interval", async () => {
    render(
      <UpdateProvider>
        <UpdateProbe />
      </UpdateProvider>,
    );

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });
    expect(checkForUpdateMock).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("status")).not.toHaveTextContent("never");

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "hidden",
    });
    await act(async () => {
      vi.advanceTimersByTime(6 * 60 * 60 * 1000);
    });
    expect(checkForUpdateMock).toHaveBeenCalledTimes(1);

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
      await Promise.resolve();
    });

    expect(checkForUpdateMock).toHaveBeenCalledTimes(2);
  });
});
