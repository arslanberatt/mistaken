import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { MicrophoneDevice, RuntimeClient } from "../../lib/tauri";
import { useMicrophoneController } from "./microphone-controller";

function makeClient(overrides: Partial<RuntimeClient> = {}): RuntimeClient {
  return {
    getRuntimeSnapshot: vi.fn(),
    listMicrophones: vi.fn().mockResolvedValue([]),
    startCapture: vi.fn().mockResolvedValue("listening"),
    stopCapture: vi.fn().mockResolvedValue("idle"),
    ...overrides,
  } as RuntimeClient;
}

const deviceA: MicrophoneDevice = { id: "a", label: "Built-in", isDefault: false };
const deviceB: MicrophoneDevice = { id: "b", label: "USB Mic", isDefault: true };

describe("useMicrophoneController", () => {
  it("does not list devices before the bridge is ready", () => {
    const client = makeClient();
    renderHook(() => useMicrophoneController(client, false));

    expect(client.listMicrophones).not.toHaveBeenCalled();
  });

  it("lists devices exactly once after the bridge becomes ready and selects the default", async () => {
    const client = makeClient({
      listMicrophones: vi.fn().mockResolvedValue([deviceA, deviceB]),
    });
    const { result, rerender } = renderHook(
      ({ ready }) => useMicrophoneController(client, ready),
      { initialProps: { ready: false } },
    );

    rerender({ ready: true });
    await waitFor(() => expect(result.current.listState).toBe("ready"));

    expect(client.listMicrophones).toHaveBeenCalledTimes(1);
    expect(result.current.devices).toEqual([deviceA, deviceB]);
    expect(result.current.selectedDeviceId).toBe("b");

    rerender({ ready: true });
    expect(client.listMicrophones).toHaveBeenCalledTimes(1);
  });

  it("selects the first device deterministically when no default exists", async () => {
    const client = makeClient({
      listMicrophones: vi.fn().mockResolvedValue([
        { id: "z", label: "Zeta", isDefault: false },
        { id: "a", label: "Alpha", isDefault: false },
      ]),
    });
    const { result } = renderHook(() => useMicrophoneController(client, true));

    await waitFor(() => expect(result.current.listState).toBe("ready"));
    expect(result.current.selectedDeviceId).toBe("z");
  });

  it("reports an empty list as ready with a null selection, not an error", async () => {
    const client = makeClient({ listMicrophones: vi.fn().mockResolvedValue([]) });
    const { result } = renderHook(() => useMicrophoneController(client, true));

    await waitFor(() => expect(result.current.listState).toBe("ready"));
    expect(result.current.devices).toEqual([]);
    expect(result.current.selectedDeviceId).toBeNull();
  });

  it("refresh preserves the current selection when it still exists", async () => {
    const client = makeClient({
      listMicrophones: vi.fn().mockResolvedValue([deviceA, deviceB]),
    });
    const { result } = renderHook(() => useMicrophoneController(client, true));
    await waitFor(() => expect(result.current.selectedDeviceId).toBe("b"));

    act(() => result.current.selectDevice("a"));
    expect(result.current.selectedDeviceId).toBe("a");

    act(() => result.current.refresh());
    // refresh() flips listState to "loading" synchronously, so waiting for
    // "ready" proves the second list actually landed before asserting that
    // the selection survived it.
    expect(result.current.listState).toBe("loading");
    await waitFor(() => expect(result.current.listState).toBe("ready"));
    expect(client.listMicrophones).toHaveBeenCalledTimes(2);
    expect(result.current.selectedDeviceId).toBe("a");
  });

  it("refresh falls back to the new default when the selected device disappears", async () => {
    const list = vi
      .fn()
      .mockResolvedValueOnce([deviceA, deviceB])
      .mockResolvedValueOnce([deviceB]);
    const client = makeClient({ listMicrophones: list });
    const { result } = renderHook(() => useMicrophoneController(client, true));
    await waitFor(() => expect(result.current.selectedDeviceId).toBe("b"));

    act(() => result.current.selectDevice("a"));
    act(() => result.current.refresh());

    // `list` is called synchronously inside refresh(), so waiting on the
    // call count alone can observe the pre-update selection. Wait on the
    // state the criterion actually cares about instead.
    await waitFor(() => expect(result.current.selectedDeviceId).toBe("b"));
    expect(list).toHaveBeenCalledTimes(2);
    expect(result.current.devices).toEqual([deviceB]);
  });

  it("a failed list reports an error and does not fabricate a device", async () => {
    const client = makeClient({
      listMicrophones: vi.fn().mockRejectedValue({
        code: "runtime_unavailable",
        message: "unavailable",
        recoverable: true,
      }),
    });
    const { result } = renderHook(() => useMicrophoneController(client, true));

    await waitFor(() => expect(result.current.listState).toBe("error"));
    expect(result.current.devices).toEqual([]);
    expect(result.current.error?.code).toBe("runtime_unavailable");
  });
});
