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
    await waitFor(() => expect(client.listMicrophones).toHaveBeenCalledTimes(2));
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

    await waitFor(() => expect(list).toHaveBeenCalledTimes(2));
    expect(result.current.selectedDeviceId).toBe("b");
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

  it("startTest invokes startCapture with the selected device and systemAudioEnabled false", async () => {
    const startCapture = vi.fn().mockResolvedValue("listening");
    const client = makeClient({
      listMicrophones: vi.fn().mockResolvedValue([deviceA]),
      startCapture,
    });
    const { result } = renderHook(() => useMicrophoneController(client, true));
    await waitFor(() => expect(result.current.selectedDeviceId).toBe("a"));

    act(() => result.current.startTest());
    expect(result.current.commandPending).toBe("start");
    await waitFor(() => expect(result.current.commandPending).toBeNull());

    expect(startCapture).toHaveBeenCalledWith({
      microphoneDeviceId: "a",
      systemAudioEnabled: false,
    });
  });

  it("a startTest failure clears commandPending and records the error", async () => {
    const client = makeClient({
      listMicrophones: vi.fn().mockResolvedValue([deviceA]),
      startCapture: vi.fn().mockRejectedValue({
        code: "microphone_permission_denied",
        message: "denied",
        recoverable: true,
      }),
    });
    const { result } = renderHook(() => useMicrophoneController(client, true));
    await waitFor(() => expect(result.current.selectedDeviceId).toBe("a"));

    act(() => result.current.startTest());
    await waitFor(() => expect(result.current.commandPending).toBeNull());
    expect(result.current.error?.code).toBe("microphone_permission_denied");
  });

  it("stopTest invokes stopCapture and toggles commandPending", async () => {
    const stopCapture = vi.fn().mockResolvedValue("idle");
    const client = makeClient({ stopCapture });
    const { result } = renderHook(() => useMicrophoneController(client, true));

    act(() => result.current.stopTest());
    expect(result.current.commandPending).toBe("stop");
    await waitFor(() => expect(result.current.commandPending).toBeNull());
    expect(stopCapture).toHaveBeenCalledTimes(1);
  });

  it("selectDevice and refresh are no-ops while a command is pending", async () => {
    // `Promise.withResolvers` needs an `es2024`+ TS `lib`; the shared root
    // `tsconfig.json` targets `ES2020`, so this test builds the deferred
    // manually rather than widen a config file this spec does not own.
    let resolveStart: (status: string) => void = () => {};
    const startCapture = vi.fn(
      () =>
        new Promise<string>((resolve) => {
          resolveStart = resolve;
        }),
    );
    const list = vi.fn().mockResolvedValue([deviceA, deviceB]);
    const client = makeClient({ listMicrophones: list, startCapture: startCapture as never });
    const { result } = renderHook(() => useMicrophoneController(client, true));
    await waitFor(() => expect(result.current.selectedDeviceId).toBe("b"));

    act(() => result.current.startTest());
    expect(result.current.commandPending).toBe("start");

    act(() => result.current.selectDevice("a"));
    expect(result.current.selectedDeviceId).toBe("b");

    act(() => result.current.refresh());
    expect(list).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveStart("listening");
      await Promise.resolve();
    });
  });
});
