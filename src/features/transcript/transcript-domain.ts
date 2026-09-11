/**
 * Pure transcript session domain: reducer, display formatting, and
 * finalized-transcript clipboard serialization.
 *
 * No native/browser API, persistence, network call, or text
 * correction/normalization happens here. Source identity is structural
 * metadata carried on `TranscriptSegment`; it is never inferred from text.
 */
import type { TranscriptSegment } from "../../types/transcript";

/** Feature-local invariant error. Never merged with Spec 03's IPC/runtime error contract. */
export interface TranscriptSessionError {
  readonly code: "segment_identity_conflict";
  readonly segmentId: string;
}

export interface TranscriptSessionState {
  readonly segments: readonly TranscriptSegment[];
  readonly lastError: TranscriptSessionError | null;
}

export type TranscriptSessionAction =
  | { readonly type: "segment/received"; readonly segment: TranscriptSegment }
  | { readonly type: "session/cleared" };

export const initialTranscriptSessionState: TranscriptSessionState = {
  segments: [],
  lastError: null,
};

/**
 * Reducer semantics (see spec-02 section 6):
 * 1. A segment id is unique for the session lifetime.
 * 2. The first occurrence of an id is appended in first-seen order.
 * 3. An existing interim segment may be replaced only by a newer
 *    interim/final segment with the same id, source, and startedAtMs.
 * 4. Replacement preserves the original array position and every other
 *    segment's object identity.
 * 5. Once isFinal is true, every later update for that id is ignored and
 *    the existing state reference is returned unchanged.
 * 6. A same-id update that changes source or startedAtMs is rejected: every
 *    existing segment reference is preserved and lastError is set.
 * 7. Any other accepted update clears a prior lastError.
 * 8. session/cleared releases every segment/error reference.
 */
export function transcriptSessionReducer(
  state: TranscriptSessionState,
  action: TranscriptSessionAction,
): TranscriptSessionState {
  switch (action.type) {
    case "segment/received":
      return receiveSegment(state, action.segment);
    case "session/cleared":
      return { segments: [], lastError: null };
    default:
      return state;
  }
}

function receiveSegment(
  state: TranscriptSessionState,
  segment: TranscriptSegment,
): TranscriptSessionState {
  const existingIndex = state.segments.findIndex((s) => s.id === segment.id);

  if (existingIndex === -1) {
    return {
      segments: [...state.segments, segment],
      lastError: null,
    };
  }

  const existing = state.segments[existingIndex];

  if (existing.isFinal) {
    return state;
  }

  if (
    existing.source !== segment.source ||
    existing.startedAtMs !== segment.startedAtMs
  ) {
    return {
      segments: state.segments,
      lastError: { code: "segment_identity_conflict", segmentId: segment.id },
    };
  }

  const nextSegments = state.segments.slice();
  nextSegments[existingIndex] = segment;
  return {
    segments: nextSegments,
    lastError: null,
  };
}

/**
 * `microphone` renders as plain text; `system` gets a structural `- `
 * prefix. Never trims, case-folds, corrects, or infers source from text.
 */
export function formatTranscriptSegment(segment: TranscriptSegment): string {
  return segment.source === "system" ? `- ${segment.text}` : segment.text;
}

/**
 * Finalized segments only, in current stable order, joined by exactly one
 * blank line. No finals yields the empty string. No leading/trailing
 * newline, metadata, timestamp, Markdown, or interim marker is added.
 */
export function serializeFinalTranscript(
  segments: readonly TranscriptSegment[],
): string {
  return segments
    .filter((segment) => segment.isFinal)
    .map(formatTranscriptSegment)
    .join("\n\n");
}
