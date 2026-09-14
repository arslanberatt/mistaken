/**
 * Feature-local React microphone controller: device list/selection state
 * and the Test microphone / Stop test command lifecycle. Owns no native
 * resource, PCM data, or continuous level — it only calls the typed runtime
 * client and tracks small presentation state.
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
  readonly commandPending: "start" | "stop" | null;
  readonly error: RuntimeError | RuntimeBridgeError | null;
}

export interface UseMicrophoneControllerResult extends MicrophoneControllerState {
  readonly refresh: () => void;
  readonly selectDevice: (id: string) => void;
  readonly startTest: () => void;
  readonly stopTest: () => void;
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
  const [commandPending, setCommandPending] = useState<"start" | "stop" | null>(null);
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
    if (commandPending) return;
    void loadDevices(true);
  }, [commandPending, loadDevices]);

  const selectDevice = useCallback(
    (id: string) => {
      if (commandPending) return;
      setSelectedDeviceId(id);
    },
    [commandPending],
  );

  const startTest = useCallback(() => {
    if (commandPending || !selectedDeviceId) return;
    setCommandPending("start");
    setError(null);
    client
      .startCapture({ microphoneDeviceId: selectedDeviceId, systemAudioEnabled: false })
      .catch((caught: unknown) => {
        if (!isMountedRef.current) return;
        setError(isRuntimeOrBridgeError(caught) ? caught : null);
      })
      .finally(() => {
        if (isMountedRef.current) setCommandPending(null);
      });
  }, [client, commandPending, selectedDeviceId]);

  const stopTest = useCallback(() => {
    if (commandPending) return;
    setCommandPending("stop");
    setError(null);
    client
      .stopCapture()
      .catch((caught: unknown) => {
        if (!isMountedRef.current) return;
        setError(isRuntimeOrBridgeError(caught) ? caught : null);
      })
      .finally(() => {
        if (isMountedRef.current) setCommandPending(null);
      });
  }, [client, commandPending]);

  return {
    devices,
    selectedDeviceId,
    listState,
    commandPending,
    error,
    refresh,
    selectDevice,
    startTest,
    stopTest,
  };
}
