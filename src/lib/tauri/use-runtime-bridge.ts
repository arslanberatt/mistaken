/**
 * `useRuntimeBridge` — the one React entry point onto the native runtime
 * spine.
 *
 * Registers exactly six typed listeners, then fetches the initial
 * snapshot only once every listener is active. Cleanup is cancellation
 * safe: an unmount that races an in-flight `listen()` call still results
 * in exactly one unlisten per acquired listener and zero leaked
 * subscriptions, including under React StrictMode's mount/unmount/mount
 * cycle. Handlers are read through a ref so identity changes never trigger
 * re-registration.
 */
import { useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { NATIVE_EVENTS } from "./contracts";
import type { RuntimeBridgeHandlers, RuntimeBridgeState, RuntimeSnapshot } from "./contracts";
import {
  bridgeError,
  getRuntimeSnapshot,
  isRuntimeBridgeError,
  parseAudioStatusEvent,
  parseCaptureStatusEvent,
  parseModelStatusEvent,
  parseRuntimeError,
  parseTranscriptSegment,
  PayloadShapeError,
  runtimeClient,
  type RuntimeClient,
} from "./runtime";

const INITIAL_STATE: RuntimeBridgeState = {
  snapshot: null,
  bridgeReady: false,
  bridgeError: null,
};

export interface UseRuntimeBridgeResult extends RuntimeBridgeState {
  client: RuntimeClient;
}

export function useRuntimeBridge(
  handlers: RuntimeBridgeHandlers = {},
): UseRuntimeBridgeResult {
  const handlersRef = useRef(handlers);
  useEffect(() => {
    handlersRef.current = handlers;
  });

  const [state, setState] = useState<RuntimeBridgeState>(INITIAL_STATE);
  // -1 guarantees the very first applied revision (native revision 0) is
  // always accepted, regardless of whether it arrives via the snapshot
  // command or a status event first.
  const appliedRevisionRef = useRef(-1);

  useEffect(() => {
    let cancelled = false;
    const activeUnlistens: UnlistenFn[] = [];

    function acquireUnlisten(pending: Promise<UnlistenFn>): Promise<void> {
      return pending.then((unlisten) => {
        if (cancelled) {
          void unlisten();
          return;
        }
        activeUnlistens.push(unlisten);
      });
    }

    async function teardown(): Promise<void> {
      const unlistens = activeUnlistens.splice(0, activeUnlistens.length);
      await Promise.allSettled(unlistens.map((unlisten) => Promise.resolve(unlisten())));
    }

    function applySnapshotIfNewer(snapshot: RuntimeSnapshot): void {
      if (snapshot.revision < appliedRevisionRef.current) return;
      appliedRevisionRef.current = snapshot.revision;
      if (cancelled) return;
      setState((previous) => ({ ...previous, snapshot }));
    }

    function reportMalformedEvent(operation: string): void {
      if (cancelled) return;
      setState((previous) => ({
        ...previous,
        bridgeError: bridgeError(
          "invalid_event_payload",
          operation,
          `received a malformed ${operation} payload`,
        ),
      }));
    }

    async function registerAllListeners(): Promise<boolean> {
      const registrations = [
        acquireUnlisten(
          listen(NATIVE_EVENTS.captureStatus, (event) => {
            try {
              const payload = parseCaptureStatusEvent(event.payload);
              applySnapshotIfNewer(payload.snapshot);
              handlersRef.current.onCaptureStatus?.(payload);
            } catch (error) {
              if (!(error instanceof PayloadShapeError)) throw error;
              reportMalformedEvent(NATIVE_EVENTS.captureStatus);
            }
          }),
        ),
        acquireUnlisten(
          listen(NATIVE_EVENTS.audioStatus, (event) => {
            try {
              const payload = parseAudioStatusEvent(event.payload);
              applySnapshotIfNewer(payload.snapshot);
              handlersRef.current.onAudioStatus?.(payload);
            } catch (error) {
              if (!(error instanceof PayloadShapeError)) throw error;
              reportMalformedEvent(NATIVE_EVENTS.audioStatus);
            }
          }),
        ),
        acquireUnlisten(
          listen(NATIVE_EVENTS.modelStatus, (event) => {
            try {
              const payload = parseModelStatusEvent(event.payload);
              applySnapshotIfNewer(payload.snapshot);
              handlersRef.current.onModelStatus?.(payload);
            } catch (error) {
              if (!(error instanceof PayloadShapeError)) throw error;
              reportMalformedEvent(NATIVE_EVENTS.modelStatus);
            }
          }),
        ),
        acquireUnlisten(
          listen(NATIVE_EVENTS.transcriptPartial, (event) => {
            try {
              const segment = parseTranscriptSegment(event.payload, false);
              handlersRef.current.onTranscriptSegment?.(segment);
            } catch (error) {
              if (!(error instanceof PayloadShapeError)) throw error;
              reportMalformedEvent(NATIVE_EVENTS.transcriptPartial);
            }
          }),
        ),
        acquireUnlisten(
          listen(NATIVE_EVENTS.transcriptFinal, (event) => {
            try {
              const segment = parseTranscriptSegment(event.payload, true);
              handlersRef.current.onTranscriptSegment?.(segment);
            } catch (error) {
              if (!(error instanceof PayloadShapeError)) throw error;
              reportMalformedEvent(NATIVE_EVENTS.transcriptFinal);
            }
          }),
        ),
        acquireUnlisten(
          listen(NATIVE_EVENTS.captureError, (event) => {
            try {
              const runtimeError = parseRuntimeError(event.payload);
              handlersRef.current.onCaptureError?.(runtimeError);
            } catch (error) {
              if (!(error instanceof PayloadShapeError)) throw error;
              reportMalformedEvent(NATIVE_EVENTS.captureError);
            }
          }),
        ),
      ];

      const results = await Promise.allSettled(registrations);
      return results.every((result) => result.status === "fulfilled");
    }

    async function run(): Promise<void> {
      const registered = await registerAllListeners();
      if (cancelled) return;

      if (!registered) {
        await teardown();
        if (cancelled) return;
        setState({
          snapshot: null,
          bridgeReady: false,
          bridgeError: bridgeError(
            "listener_registration_failed",
            "listen",
            "failed to register one or more native event listeners",
          ),
        });
        return;
      }

      try {
        const snapshot = await getRuntimeSnapshot();
        if (cancelled) return;
        applySnapshotIfNewer(snapshot);
        setState((previous) => ({ ...previous, bridgeReady: true, bridgeError: null }));
      } catch (error) {
        if (cancelled) return;
        setState((previous) => ({
          ...previous,
          bridgeReady: false,
          bridgeError: isRuntimeBridgeError(error)
            ? error
            : bridgeError(
                "command_failed",
                "get_runtime_snapshot",
                "get_runtime_snapshot failed",
              ),
        }));
      }
    }

    void run();

    return () => {
      cancelled = true;
      void teardown();
    };
  }, []);

  return { ...state, client: runtimeClient };
}
