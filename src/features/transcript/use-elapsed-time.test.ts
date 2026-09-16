import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import type { CaptureStatus } from "../../types/runtime";
import { useElapsedTime } from "./use-elapsed-time";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

function renderElapsedTime(initialStatus: CaptureStatus) {
  return renderHook((status: CaptureStatus) => useElapsedTime(status), {
    initialProps: initialStatus,
  });
}

describe("useElapsedTime", () => {
  it("stays at zero while idle", () => {
    const { result } = renderElapsedTime("idle");
    expect(result.current).toBe(0);
  });

  it("advances once per second while listening, recomputed from the start instant", () => {
    const { result, rerender } = renderElapsedTime("idle");

    rerender("listening");
    expect(result.current).toBe(0);

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(result.current).toBe(1000);

    act(() => {
      vi.advanceTimersByTime(2500);
    });
    expect(result.current).toBe(3000);
  });

  it("freezes the displayed value on leaving listening and holds it", () => {
    const { result, rerender } = renderElapsedTime("listening");

    act(() => {
      vi.advanceTimersByTime(4200);
    });
    rerender("stopping");
    const frozen = result.current;
    expect(frozen).toBeGreaterThanOrEqual(4200);

    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(result.current).toBe(frozen);

    rerender("idle");
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(result.current).toBe(frozen);
  });

  it("resets to zero on the next start after freezing", () => {
    const { result, rerender } = renderElapsedTime("listening");

    act(() => {
      vi.advanceTimersByTime(9000);
    });
    rerender("idle");
    expect(result.current).toBeGreaterThan(0);

    rerender("listening");
    expect(result.current).toBe(0);

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(result.current).toBe(1000);
  });

  it("creates exactly one interval while listening and clears it on leaving", () => {
    const setIntervalSpy = vi.spyOn(globalThis, "setInterval");
    const clearIntervalSpy = vi.spyOn(globalThis, "clearInterval");
    const { rerender } = renderElapsedTime("idle");

    rerender("listening");
    expect(setIntervalSpy).toHaveBeenCalledTimes(1);

    rerender("idle");
    expect(clearIntervalSpy).toHaveBeenCalledTimes(1);

    setIntervalSpy.mockRestore();
    clearIntervalSpy.mockRestore();
  });

  it("clears its interval on unmount without leaking a timer", () => {
    const clearIntervalSpy = vi.spyOn(globalThis, "clearInterval");
    const { unmount } = renderElapsedTime("listening");

    unmount();
    expect(clearIntervalSpy).toHaveBeenCalledTimes(1);
    clearIntervalSpy.mockRestore();
  });
});
