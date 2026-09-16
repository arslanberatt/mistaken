import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, waitFor } from "@testing-library/react";
import { createElement } from "react";

import type { AudioSourceStatus } from "../../lib/tauri";
import { useSourceRecoveryPresentation, type RecoveryPresentation } from "./recovery-presentation";

const { listen } = vi.hoisted(() => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

interface Payload {
  code: string;
  message: string;
  recoverable: boolean;
  source?: "microphone" | "system";
}

function captureHandler(): {
  emit: (payload: Payload) => void;
} {
  let handler: ((event: { payload: unknown }) => void) | undefined;
  listen.mockImplementation(async (_event: string, fn: (event: { payload: unknown }) => void) => {
    handler = fn;
    return vi.fn(async () => {});
  });
  return {
    emit: (payload: Payload) => {
      act(() => {
        handler?.({ payload });
      });
    },
  };
}

function HookHarness({
  source,
  status,
  onResult,
}: {
  source: "microphone" | "system";
  status: AudioSourceStatus | undefined;
  onResult: (result: RecoveryPresentation) => void;
}) {
  const result = useSourceRecoveryPresentation(source, status);
  onResult(result);
  return null;
}

afterEach(() => {
  vi.clearAllMocks();
});

function renderHarness(source: "microphone" | "system", status: AudioSourceStatus | undefined) {
  let latest: RecoveryPresentation | undefined;
  const { rerender } = render(
    createElement(HookHarness, {
      source,
      status,
      onResult: (result) => (latest = result),
    }),
  );
  return {
    get latest() {
      return latest;
    },
    setStatus: (next: AudioSourceStatus | undefined) => {
      rerender(
        createElement(HookHarness, {
          source,
          status: next,
          onResult: (result) => (latest = result),
        }),
      );
    },
  };
}

describe("useSourceRecoveryPresentation", () => {
  it("starts with no recovering/degraded presentation", async () => {
    captureHandler();
    const harness = renderHarness("microphone", { status: "idle" });
    await waitFor(() => expect(listen).toHaveBeenCalled());
    expect(harness.latest).toStrictEqual({ recovering: null, degraded: false });
  });

  it("parses a Reconnecting capture:error into a structured attempt", async () => {
    const { emit } = captureHandler();
    const harness = renderHarness("microphone", { status: "starting", deviceId: "mic-1" });
    await waitFor(() => expect(listen).toHaveBeenCalled());

    emit({
      code: "device_disconnected",
      message: "Reconnecting microphone… attempt 1 of 3",
      recoverable: true,
      source: "microphone",
    });

    await waitFor(() =>
      expect(harness.latest?.recovering).toStrictEqual({ attempt: 1, max: 3 }),
    );
  });

  it("ignores capture:error events for the other source", async () => {
    const { emit } = captureHandler();
    const harness = renderHarness("microphone", { status: "starting", deviceId: "mic-1" });
    await waitFor(() => expect(listen).toHaveBeenCalled());

    emit({
      code: "system_audio_unavailable",
      message: "Reconnecting system audio… attempt 1 of 3",
      recoverable: true,
      source: "system",
    });

    // `emit` wraps the handler call in a synchronous `act()`, which
    // flushes any resulting state update before returning — no real
    // timer needed to observe that this event was correctly ignored.
    expect(harness.latest?.recovering).toBeNull();
  });

  it("clears recovering once status leaves starting", async () => {
    const { emit } = captureHandler();
    const harness = renderHarness("microphone", { status: "starting", deviceId: "mic-1" });
    await waitFor(() => expect(listen).toHaveBeenCalled());

    emit({
      code: "device_disconnected",
      message: "Reconnecting microphone… attempt 2 of 3",
      recoverable: true,
      source: "microphone",
    });
    await waitFor(() =>
      expect(harness.latest?.recovering).toStrictEqual({ attempt: 2, max: 3 }),
    );

    harness.setStatus({ status: "capturing", deviceId: "mic-1", activity: "receiving" });
    await waitFor(() => expect(harness.latest?.recovering).toBeNull());
  });

  it("enters and leaves the degraded state from the inference_lagging transition messages", async () => {
    const { emit } = captureHandler();
    const harness = renderHarness("system", {
      status: "capturing",
      activity: "receiving",
    });
    await waitFor(() => expect(listen).toHaveBeenCalled());

    emit({
      code: "inference_lagging",
      message:
        "System audio is transcribing slower than real time. Some audio is being skipped.",
      recoverable: true,
      source: "system",
    });
    await waitFor(() => expect(harness.latest?.degraded).toBe(true));

    emit({
      code: "inference_lagging",
      message: "System audio transcription has recovered.",
      recoverable: true,
      source: "system",
    });
    await waitFor(() => expect(harness.latest?.degraded).toBe(false));
  });

  it("clears degraded once status leaves capturing", async () => {
    const { emit } = captureHandler();
    const harness = renderHarness("system", {
      status: "capturing",
      activity: "receiving",
    });
    await waitFor(() => expect(listen).toHaveBeenCalled());

    emit({
      code: "inference_lagging",
      message:
        "System audio is transcribing slower than real time. Some audio is being skipped.",
      recoverable: true,
      source: "system",
    });
    await waitFor(() => expect(harness.latest?.degraded).toBe(true));

    harness.setStatus({ status: "idle" });
    await waitFor(() => expect(harness.latest?.degraded).toBe(false));
  });

  it("does not confuse a plain fresh start with a lingering recovery from an earlier episode", async () => {
    const { emit } = captureHandler();
    const harness = renderHarness("microphone", { status: "starting", deviceId: "mic-1" });
    await waitFor(() => expect(listen).toHaveBeenCalled());

    emit({
      code: "device_disconnected",
      message: "Reconnecting microphone… attempt 1 of 3",
      recoverable: true,
      source: "microphone",
    });
    await waitFor(() =>
      expect(harness.latest?.recovering).toStrictEqual({ attempt: 1, max: 3 }),
    );

    // Recovered, then later a completely unrelated Stop -> fresh Start.
    harness.setStatus({ status: "capturing", deviceId: "mic-1", activity: "receiving" });
    await waitFor(() => expect(harness.latest?.recovering).toBeNull());
    harness.setStatus({ status: "idle" });
    harness.setStatus({ status: "starting", deviceId: "mic-1" });

    expect(harness.latest?.recovering).toBeNull();
  });
});
