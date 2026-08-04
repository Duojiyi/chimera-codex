import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  UPDATE_CHECK_INTERVAL_MS,
  UPDATE_CHECK_POLL_MS,
  UpdateProvider,
  useUpdate,
} from "@/contexts/UpdateContext";

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

  it("checks again about 15 minutes later while the app stays visible", async () => {
    render(
      <UpdateProvider>
        <UpdateProbe />
      </UpdateProvider>,
    );

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(checkForUpdateMock).toHaveBeenCalledTimes(1);

    // Well short of the interval: must not fire yet.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10 * 60 * 1000);
    });
    expect(checkForUpdateMock).toHaveBeenCalledTimes(1);

    // Past the interval (plus one poll period, so a tick is guaranteed to land
    // beyond the threshold): now it must fire.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(
        UPDATE_CHECK_INTERVAL_MS - 10 * 60 * 1000 + UPDATE_CHECK_POLL_MS,
      );
    });
    expect(checkForUpdateMock).toHaveBeenCalledTimes(2);
  });

  // Guards the drift trap documented on UPDATE_CHECK_POLL_MS: if the poll period
  // is ever raised to equal the interval, timer drift makes each tick land just
  // short of the threshold, it skips, and the effective spacing silently
  // doubles. Fake timers are exact, so no behavioural test can catch this —
  // only the arithmetic relationship can.
  it("polls strictly more often than the staleness threshold", () => {
    expect(UPDATE_CHECK_POLL_MS).toBeLessThan(UPDATE_CHECK_INTERVAL_MS);
    expect(UPDATE_CHECK_INTERVAL_MS).toBe(15 * 60 * 1000);
  });
});
