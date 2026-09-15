/**
 * Real elapsed-capture-time clock. Exactly one 1 s interval exists while
 * `captureStatus` is `"listening"`; it is created on entering that status
 * and cleared on leaving it and on unmount. The displayed value is always
 * recomputed from the start instant rather than accumulated from ticks, so
 * a throttled background window or a skipped tick cannot desynchronize it.
 * It freezes at whatever value it held on leaving `listening` and resets
 * to zero only on the next entry into `listening`.
 *
 * The zero-reset is applied during render (matching React's guidance for
 * state that must reset when an external value changes, already used in
 * `App.tsx` for the microphone-status transition) using only the constant
 * `0`, never a wall-clock read — `Date.now()` is impure and must not run
 * during render. Every wall-clock read happens inside the interval effect,
 * either in its scheduled tick or its cleanup, never synchronously in the
 * effect's own body.
 */
import { useEffect, useRef, useState } from "react";
import type { CaptureStatus } from "../../types/runtime";

const TICK_INTERVAL_MS = 1000;

export function useElapsedTime(captureStatus: CaptureStatus): number {
  const isListening = captureStatus === "listening";
  const [previousIsListening, setPreviousIsListening] = useState(isListening);
  const [elapsedMs, setElapsedMs] = useState(0);
  const startInstantRef = useRef<number | null>(null);

  if (isListening !== previousIsListening) {
    setPreviousIsListening(isListening);
    if (isListening) {
      setElapsedMs(0);
    }
  }

  useEffect(() => {
    if (!isListening) {
      return;
    }

    startInstantRef.current = Date.now();
    const intervalId = setInterval(() => {
      if (startInstantRef.current !== null) {
        setElapsedMs(Date.now() - startInstantRef.current);
      }
    }, TICK_INTERVAL_MS);

    return () => {
      clearInterval(intervalId);
      if (startInstantRef.current !== null) {
        setElapsedMs(Date.now() - startInstantRef.current);
      }
      startInstantRef.current = null;
    };
  }, [isListening]);

  return elapsedMs;
}
