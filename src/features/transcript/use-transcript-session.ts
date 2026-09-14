/**
 * React binding for the pure transcript session reducer. Owns no native
 * resource, timer, listener, or persistence; it only exposes dispatch
 * helpers around `transcriptSessionReducer`.
 */
import { useCallback, useReducer } from "react";
import type { TranscriptSegment } from "../../types/transcript";
import {
  initialTranscriptSessionState,
  transcriptSessionReducer,
  type TranscriptSessionState,
} from "./transcript-domain";

export interface UseTranscriptSessionResult {
  readonly state: TranscriptSessionState;
  readonly receiveSegment: (segment: TranscriptSegment) => void;
  readonly clear: () => void;
}

export function useTranscriptSession(
  initialState: TranscriptSessionState = initialTranscriptSessionState,
): UseTranscriptSessionResult {
  const [state, dispatch] = useReducer(
    transcriptSessionReducer,
    initialState,
  );

  const receiveSegment = useCallback((segment: TranscriptSegment) => {
    dispatch({ type: "segment/received", segment });
  }, []);

  const clear = useCallback(() => {
    dispatch({ type: "session/cleared" });
  }, []);

  return { state, receiveSegment, clear };
}
