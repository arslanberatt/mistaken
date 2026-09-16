/**
 * Derives Spec 10 recovering/degraded source presentation purely from the
 * existing `capture:error` event stream plus the source's current
 * `AudioSourceStatus`, without widening the frozen event/status contract.
 *
 * Per `docs/lifecycle-policy.md` sections 2 and 4, a recovery attempt
 * counter and a sustained-lag transition both travel as free-text
 * `capture:error` messages on the existing codes — never a new payload
 * field or a new event. This hook is the one place that turns that text
 * back into small, typed presentation state; the two audio controls only
 * render it.
 *
 * Registers exactly one native listener (`capture:error`) independent of
 * `useRuntimeBridge`'s own six, so no change to `src/App.tsx` or
 * `src/lib/tauri/**` is required to observe it here.
 */
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { NATIVE_EVENTS } from "../../lib/tauri";
import type { AudioSourceStatus, TranscriptSource } from "../../lib/tauri";
import { parseRuntimeError, PayloadShapeError } from "../../lib/tauri/runtime";

export interface RecoveryAttempt {
  readonly attempt: number;
  readonly max: number;
}

export interface RecoveryPresentation {
  readonly recovering: RecoveryAttempt | null;
  readonly degraded: boolean;
}

const NONE_STATE: RecoveryPresentation = { recovering: null, degraded: false };

const RECONNECTING_PATTERN = /^Reconnecting .+ attempt (\d+) of (\d+)$/u;

function parseRecoveringMessage(message: string): RecoveryAttempt | null {
  const match = RECONNECTING_PATTERN.exec(message);
  if (!match) return null;
  const attempt = Number(match[1]);
  const max = Number(match[2]);
  if (!Number.isFinite(attempt) || !Number.isFinite(max)) return null;
  return { attempt, max };
}

/**
 * `source`: which source this instance tracks. `status`: that source's
 * current status from the already-flowing snapshot prop, used only to
 * clear stale presentation once the state transition it described is
 * over (recovered into `capturing`, exhausted into `error`, or Stopped).
 */
export function useSourceRecoveryPresentation(
  source: TranscriptSource,
  status: AudioSourceStatus | undefined,
): RecoveryPresentation {
  const [state, setState] = useState<RecoveryPresentation>(NONE_STATE);

  // React-recommended "adjusting state when a prop changes" pattern:
  // compared and, if needed, updated directly during render rather than
  // inside an effect. This clears stale recovering/degraded presentation
  // exactly when the status transitions away from the phase it
  // described (recovered into `capturing`, exhausted into `error`, or
  // stopped), without an extra effect-driven render pass.
  const [observedStatusKind, setObservedStatusKind] = useState(status?.status);
  if (status?.status !== observedStatusKind) {
    setObservedStatusKind(status?.status);
    if (status?.status !== "starting" && state.recovering) {
      setState((previous) => ({ ...previous, recovering: null }));
    }
    if (status?.status !== "capturing" && state.degraded) {
      setState((previous) => ({ ...previous, degraded: false }));
    }
  }

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen(NATIVE_EVENTS.captureError, (event) => {
      let error;
      try {
        error = parseRuntimeError(event.payload);
      } catch (cause) {
        if (cause instanceof PayloadShapeError) return;
        throw cause;
      }
      if (error.source !== source) return;

      if (error.recoverable) {
        const recovering = parseRecoveringMessage(error.message);
        if (recovering) {
          setState((previous) => ({ ...previous, recovering }));
          return;
        }
      }
      if (error.code === "inference_lagging") {
        if (error.message.includes("transcription has recovered")) {
          setState((previous) => ({ ...previous, degraded: false }));
        } else if (error.message.includes("transcribing slower than real time")) {
          setState((previous) => ({ ...previous, degraded: true }));
        }
      }
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch(() => {
        // A failed listener registration leaves presentation at its
        // truthful default (not recovering, not degraded); the
        // underlying status text still reflects reality either way.
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [source]);

  return state;
}
