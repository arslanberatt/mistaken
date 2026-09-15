/**
 * Feature-local React microphone controller: device list/selection state
 * only. Owns no native resource, PCM data, continuous level, or capture
 * command — the single production `Start Listening` / `Stop` action lives
 * in the transcript workspace footer and calls the typed runtime client
 * directly with this hook's `selectedDeviceId`.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import type {
  MicrophoneDevice,
  RuntimeBridgeError,
  RuntimeClient,
  RuntimeError,
} from "../../lib/tauri";

export interface MicrophoneControllerState {
  readonly devices: readonly MicrophoneDevice[];
  readonly selectedDeviceId: string | null;
  readonly listState: "loading" | "ready" | "error";
  readonly error: RuntimeError | RuntimeBridgeError | null;
}

export interface UseMicrophoneControllerResult extends MicrophoneControllerState {
  readonly refresh: () => void;
  readonly selectDevice: (id: string) => void;
}

function isRuntimeOrBridgeError(value: unknown): value is RuntimeError | RuntimeBridgeError {
  return typeof value === "object" && value !== null && "code" in value;
}

function chooseSelection(
  devices: readonly MicrophoneDevice[],
  previousSelection: string | null,
  isRefresh: boolean,
): string | null {
  if (isRefresh && previousSelection && devices.some((d) => d.id === previousSelection)) {
    return previousSelection;
  }
  const defaultDevice = devices.find((d) => d.isDefault);
  return defaultDevice?.id ?? devices[0]?.id ?? null;
}

/**
 * `client`/`bridgeReady` come from `useRuntimeBridge`. The initial device
 * list is fetched exactly once, after the bridge first becomes ready.
 */
export function useMicrophoneController(
  client: RuntimeClient,
  bridgeReady: boolean,
): UseMicrophoneControllerResult {
  const [devices, setDevices] = useState<readonly MicrophoneDevice[]>([]);
  const [selectedDeviceId, setSelectedDeviceId] = useState<string | null>(null);
  const [listState, setListState] = useState<"loading" | "ready" | "error">("loading");
  const [error, setError] = useState<RuntimeError | RuntimeBridgeError | null>(null);

  const hasLoadedOnceRef = useRef(false);
  const isMountedRef = useRef(true);
  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const loadDevices = useCallback(
    async (isRefresh: boolean) => {
      setListState("loading");
      try {
        const list = await client.listMicrophones();
        if (!isMountedRef.current) return;
        setDevices(list);
        setSelectedDeviceId((previous) => chooseSelection(list, previous, isRefresh));
        setListState("ready");
        setError(null);
      } catch (caught) {
        if (!isMountedRef.current) return;
        setListState("error");
        setError(isRuntimeOrBridgeError(caught) ? caught : null);
      }
    },
    [client],
  );

  useEffect(() => {
    if (!bridgeReady || hasLoadedOnceRef.current) return;
    hasLoadedOnceRef.current = true;
    void loadDevices(false);
  }, [bridgeReady, loadDevices]);

  const refresh = useCallback(() => {
    void loadDevices(true);
  }, [loadDevices]);

  const selectDevice = useCallback((id: string) => {
    setSelectedDeviceId(id);
  }, []);

  return {
    devices,
    selectedDeviceId,
    listState,
    error,
    refresh,
    selectDevice,
  };
}
