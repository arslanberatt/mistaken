import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, waitFor } from "@testing-library/react";
import { createElement } from "react";

import type { RuntimeSnapshot } from "./contracts";
import type * as RuntimeModule from "./runtime";
import { useRuntimeBridge, type UseRuntimeBridgeResult } from "./use-runtime-bridge";

const { listen } = vi.hoisted(() => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

const { getRuntimeSnapshot } = vi.hoisted(() => ({ getRuntimeSnapshot: vi.fn() }));
vi.mock("./runtime", async () => {
  const actual = await vi.importActual<typeof RuntimeModule>("./runtime");
  return { ...actual, getRuntimeSnapshot };
});

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function snapshot(revision: number): RuntimeSnapshot {
  return {
    revision,
    captureStatus: "idle",
    modelStatus: { status: "missing" },
    microphone: {
      status: "unavailable",
      error: { code: "runtime_unavailable", message: "n/a", recoverable: true },
    },
    systemAudio: {
      status: "unavailable",
      error: { code: "runtime_unavailable", message: "n/a", recoverable: true },
    },
  };
}

/** Captures the six per-event handlers registered through mocked `listen`. */
function captureHandlers(): {
  handlers: Map<string, (event: { payload: unknown }) => void>;
  unlistenSpies: Map<string, ReturnType<typeof vi.fn>>;
} {
  const handlers = new Map<string, (event: { payload: unknown }) => void>();
  const unlistenSpies = new Map<string, ReturnType<typeof vi.fn>>();

  listen.mockImplementation(async (event: string, handler: (event: { payload: unknown }) => void) => {
    handlers.set(event, handler);
    const unlisten = vi.fn(async () => {});
    unlistenSpies.set(event, unlisten);
    return unlisten;
  });

  return { handlers, unlistenSpies };
}

function HookHarness({
  onResult,
}: {
  onResult: (result: UseRuntimeBridgeResult) => void;
}) {
  const result = useRuntimeBridge();
  onResult(result);
  return null;
}

afterEach(() => {
  vi.clearAllMocks();
});

describe("useRuntimeBridge registration lifecycle", () => {
  it("registers all six listeners before invoking get_runtime_snapshot", async () => {
    const order: string[] = [];
    listen.mockImplementation(async (event: string) => {
      order.push(`listen:${event}`);
      return vi.fn(async () => {});
    });
    getRuntimeSnapshot.mockImplementation(async () => {
      order.push("get_runtime_snapshot");
      return snapshot(0);
    });

    let latest: UseRuntimeBridgeResult | undefined;
    render(createElement(HookHarness, { onResult: (result) => (latest = result) }));

    await waitFor(() => expect(latest?.bridgeReady).toBe(true));

    expect(listen).toHaveBeenCalledTimes(6);
    expect(order.indexOf("get_runtime_snapshot")).toBe(order.length - 1);
    expect(latest?.snapshot).toStrictEqual(snapshot(0));
    expect(latest?.bridgeError).toBeNull();
  });

  it("tears down every acquired listener when one registration fails", async () => {
    const { unlistenSpies } = captureHandlers();
    listen.mockImplementationOnce(async () => {
      throw new Error("registration failed");
    });

    let latest: UseRuntimeBridgeResult | undefined;
    render(createElement(HookHarness, { onResult: (result) => (latest = result) }));

    await waitFor(() =>
      expect(latest?.bridgeError?.code).toBe("listener_registration_failed"),
    );

    expect(latest?.bridgeReady).toBe(false);
    expect(getRuntimeSnapshot).not.toHaveBeenCalled();
    for (const unlisten of unlistenSpies.values()) {
      expect(unlisten).toHaveBeenCalledTimes(1);
    }
  });

  it("unmount before listen() resolves still unlistens exactly once per event and registers nothing live", async () => {
    const pending = deferred<void>();
    const unlistenSpies: Array<ReturnType<typeof vi.fn>> = [];
    listen.mockImplementation(async () => {
      await pending.promise;
      const unlisten = vi.fn(async () => {});
      unlistenSpies.push(unlisten);
      return unlisten;
    });

    const view = render(
      createElement(HookHarness, { onResult: () => undefined }),
    );
    view.unmount();

    await act(async () => {
      pending.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(unlistenSpies).toHaveLength(6);
    for (const unlisten of unlistenSpies) {
      expect(unlisten).toHaveBeenCalledTimes(1);
    }
  });

  it("mount -> unmount -> mount leaves exactly one active listener per event", async () => {
    const registrationsByEvent = new Map<
      string,
      Array<ReturnType<typeof vi.fn>>
    >();
    listen.mockImplementation(async (event: string) => {
      const unlisten = vi.fn(async () => {});
      const list = registrationsByEvent.get(event) ?? [];
      list.push(unlisten);
      registrationsByEvent.set(event, list);
      return unlisten;
    });
    getRuntimeSnapshot.mockResolvedValue(snapshot(0));

    const first = render(createElement(HookHarness, { onResult: () => undefined }));
    first.unmount();

    let latest: UseRuntimeBridgeResult | undefined;
    render(createElement(HookHarness, { onResult: (result) => (latest = result) }));

    await waitFor(() => expect(latest?.bridgeReady).toBe(true));

    for (const [, unlistens] of registrationsByEvent) {
      expect(unlistens).toHaveLength(2);
      // The first mount's listener was torn down; the live mount's was not.
      expect(unlistens[0]).toHaveBeenCalledTimes(1);
      expect(unlistens[1]).toHaveBeenCalledTimes(0);
    }
  });
});

describe("useRuntimeBridge revision handling", () => {
  it("does not let a stale snapshot response overwrite a newer status event", async () => {
    const { handlers } = captureHandlers();
    const pendingSnapshot = deferred<RuntimeSnapshot>();
    getRuntimeSnapshot.mockImplementation(() => pendingSnapshot.promise);

    let latest: UseRuntimeBridgeResult | undefined;
    render(createElement(HookHarness, { onResult: (result) => (latest = result) }));

    await waitFor(() => expect(handlers.size).toBe(6));

    act(() => {
      handlers.get("capture:status")?.({ payload: { snapshot: snapshot(5) } });
    });
    expect(latest?.snapshot?.revision).toBe(5);

    await act(async () => {
      pendingSnapshot.resolve(snapshot(0));
      await Promise.resolve();
    });

    expect(latest?.snapshot?.revision).toBe(5);
    expect(latest?.bridgeReady).toBe(true);
  });

  it("applies an equal revision idempotently", async () => {
    const { handlers } = captureHandlers();
    getRuntimeSnapshot.mockResolvedValue(snapshot(2));

    let latest: UseRuntimeBridgeResult | undefined;
    render(createElement(HookHarness, { onResult: (result) => (latest = result) }));

    await waitFor(() => expect(latest?.bridgeReady).toBe(true));

    act(() => {
      handlers.get("asr:model-status")?.({ payload: { snapshot: snapshot(2) } });
    });

    expect(latest?.snapshot?.revision).toBe(2);
  });

  it("never discards capture:error or transcript events based on revision", async () => {
    const { handlers } = captureHandlers();
    getRuntimeSnapshot.mockResolvedValue(snapshot(9));

    const onCaptureError = vi.fn();
    const onTranscriptSegment = vi.fn();
    function Harness() {
      useRuntimeBridge({ onCaptureError, onTranscriptSegment });
      return null;
    }
    render(createElement(Harness));

    await waitFor(() => expect(handlers.size).toBe(6));

    act(() => {
      handlers.get("capture:error")?.({
        payload: { code: "device_disconnected", message: "unplugged", recoverable: true },
      });
      handlers.get("transcript:final")?.({
        payload: {
          id: "mic-1-0",
          source: "microphone",
          text: "I didn't knew that",
          startedAtMs: 0,
          isFinal: true,
        },
      });
    });

    expect(onCaptureError).toHaveBeenCalledWith(
      expect.objectContaining({ code: "device_disconnected" }),
    );
    expect(onTranscriptSegment).toHaveBeenCalledWith(
      expect.objectContaining({ text: "I didn't knew that" }),
    );
  });
});

describe("useRuntimeBridge malformed event handling", () => {
  it("reports invalid_event_payload without invoking the handler or tearing down other listeners", async () => {
    const { handlers, unlistenSpies } = captureHandlers();
    getRuntimeSnapshot.mockResolvedValue(snapshot(0));

    const onCaptureStatus = vi.fn();
    function Harness({
      onResult,
    }: {
      onResult: (result: UseRuntimeBridgeResult) => void;
    }) {
      const result = useRuntimeBridge({ onCaptureStatus });
      onResult(result);
      return null;
    }

    let latest: UseRuntimeBridgeResult | undefined;
    render(createElement(Harness, { onResult: (result) => (latest = result) }));

    await waitFor(() => expect(latest?.bridgeReady).toBe(true));

    act(() => {
      handlers.get("capture:status")?.({ payload: { snapshot: { revision: "nope" } } });
    });

    await waitFor(() =>
      expect(latest?.bridgeError?.code).toBe("invalid_event_payload"),
    );
    expect(onCaptureStatus).not.toHaveBeenCalled();
    for (const unlisten of unlistenSpies.values()) {
      expect(unlisten).not.toHaveBeenCalled();
    }
  });
});

describe("useRuntimeBridge handler identity", () => {
  it("does not re-register listeners when the handler identity changes", async () => {
    getRuntimeSnapshot.mockResolvedValue(snapshot(0));
    listen.mockImplementation(async () => vi.fn(async () => {}));

    const first = vi.fn();
    const second = vi.fn();
    function Harness({ handler }: { handler: () => void }) {
      useRuntimeBridge({ onCaptureError: handler });
      return null;
    }

    const view = render(createElement(Harness, { handler: first }));
    await waitFor(() => expect(listen).toHaveBeenCalledTimes(6));

    view.rerender(createElement(Harness, { handler: second }));

    expect(listen).toHaveBeenCalledTimes(6);
  });
});
