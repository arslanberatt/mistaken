import { describe, expect, it } from "vitest";
import type { TranscriptSegment } from "../../types/transcript";
import {
  formatTranscriptSegment,
  initialTranscriptSessionState,
  serializeFinalTranscript,
  transcriptSessionReducer,
} from "./transcript-domain";

function segment(
  overrides: Pick<TranscriptSegment, "id" | "source" | "text"> &
    Partial<Omit<TranscriptSegment, "id" | "source" | "text">>,
): TranscriptSegment {
  return {
    startedAtMs: 0,
    isFinal: false,
    ...overrides,
  };
}

describe("transcriptSessionReducer", () => {
  it("appends new segment ids in first-seen order regardless of timestamp values", () => {
    const first = segment({
      id: "mic-1",
      source: "microphone",
      text: "first",
      startedAtMs: 5000,
    });
    const second = segment({
      id: "mic-2",
      source: "microphone",
      text: "second",
      startedAtMs: 1000,
    });

    let state = transcriptSessionReducer(initialTranscriptSessionState, {
      type: "segment/received",
      segment: first,
    });
    state = transcriptSessionReducer(state, {
      type: "segment/received",
      segment: second,
    });

    expect(state.segments.map((s) => s.id)).toEqual(["mic-1", "mic-2"]);
  });

  it("replaces a matching interim segment in place, preserving array position", () => {
    const before = segment({
      id: "sys-0",
      source: "system",
      text: "Why?",
      startedAtMs: 500,
      isFinal: true,
    });
    const original = segment({
      id: "mic-1",
      source: "microphone",
      text: "Um, because",
      startedAtMs: 1000,
    });
    const updated = segment({
      id: "mic-1",
      source: "microphone",
      text: "Um, because my friend",
      startedAtMs: 1000,
    });

    let state = transcriptSessionReducer(initialTranscriptSessionState, {
      type: "segment/received",
      segment: before,
    });
    state = transcriptSessionReducer(state, {
      type: "segment/received",
      segment: original,
    });
    state = transcriptSessionReducer(state, {
      type: "segment/received",
      segment: updated,
    });

    expect(state.segments).toHaveLength(2);
    expect(state.segments[1]).toBe(updated);
    expect(state.segments[1].text).toBe("Um, because my friend");
  });

  it("freezes a final segment against every later update, returning the same state reference", () => {
    const finalSeg = segment({
      id: "mic-1",
      source: "microphone",
      text: "I didn't knew anyone there.",
      startedAtMs: 1000,
      isFinal: true,
    });
    const attemptedRewrite: TranscriptSegment = {
      ...finalSeg,
      text: "I didn't know anyone there.",
    };

    let state = transcriptSessionReducer(initialTranscriptSessionState, {
      type: "segment/received",
      segment: finalSeg,
    });
    const afterFinal = state;
    state = transcriptSessionReducer(state, {
      type: "segment/received",
      segment: attemptedRewrite,
    });

    expect(state).toBe(afterFinal);
    expect(state.segments[0]).toBe(finalSeg);
    expect(state.segments[0].text).toBe("I didn't knew anyone there.");
  });

  it("rejects a same-id update that changes source, preserving the original row and reporting a conflict", () => {
    const original = segment({
      id: "seg-1",
      source: "microphone",
      text: "hello",
      startedAtMs: 1000,
    });
    const conflictingSource = segment({
      id: "seg-1",
      source: "system",
      text: "hello system",
      startedAtMs: 1000,
    });

    let state = transcriptSessionReducer(initialTranscriptSessionState, {
      type: "segment/received",
      segment: original,
    });
    const segmentsBeforeConflict = state.segments;
    state = transcriptSessionReducer(state, {
      type: "segment/received",
      segment: conflictingSource,
    });

    expect(state.segments).toBe(segmentsBeforeConflict);
    expect(state.segments[0]).toBe(original);
    expect(state.lastError).toEqual({
      code: "segment_identity_conflict",
      segmentId: "seg-1",
    });
  });

  it("rejects a same-id update that changes startedAtMs, preserving the original row", () => {
    const original = segment({
      id: "seg-1",
      source: "microphone",
      text: "hello",
      startedAtMs: 1000,
    });
    const conflictingStart = segment({
      id: "seg-1",
      source: "microphone",
      text: "hello again",
      startedAtMs: 2000,
    });

    let state = transcriptSessionReducer(initialTranscriptSessionState, {
      type: "segment/received",
      segment: original,
    });
    state = transcriptSessionReducer(state, {
      type: "segment/received",
      segment: conflictingStart,
    });

    expect(state.segments[0]).toBe(original);
    expect(state.lastError).toEqual({
      code: "segment_identity_conflict",
      segmentId: "seg-1",
    });
  });

  it("clears a prior lastError on the next accepted update", () => {
    const original = segment({
      id: "seg-1",
      source: "microphone",
      text: "hello",
      startedAtMs: 1000,
    });
    const conflict = segment({
      id: "seg-1",
      source: "system",
      text: "conflict",
      startedAtMs: 1000,
    });
    const validUpdate = segment({
      id: "seg-1",
      source: "microphone",
      text: "hello again",
      startedAtMs: 1000,
    });

    let state = transcriptSessionReducer(initialTranscriptSessionState, {
      type: "segment/received",
      segment: original,
    });
    state = transcriptSessionReducer(state, {
      type: "segment/received",
      segment: conflict,
    });
    expect(state.lastError).not.toBeNull();

    state = transcriptSessionReducer(state, {
      type: "segment/received",
      segment: validUpdate,
    });
    expect(state.lastError).toBeNull();
    expect(state.segments[0].text).toBe("hello again");
  });

  it("returns the exact prior state reference for an ignored post-final update", () => {
    const finalSeg = segment({
      id: "seg-1",
      source: "microphone",
      text: "done",
      startedAtMs: 1000,
      isFinal: true,
    });
    let state = transcriptSessionReducer(initialTranscriptSessionState, {
      type: "segment/received",
      segment: finalSeg,
    });
    const afterFinal = state;
    state = transcriptSessionReducer(state, {
      type: "segment/received",
      segment: { ...finalSeg, text: "changed" },
    });

    expect(state).toBe(afterFinal);
  });

  it("session/cleared releases every prior segment and error reference", () => {
    const finalSeg = segment({
      id: "seg-1",
      source: "microphone",
      text: "done",
      startedAtMs: 1000,
      isFinal: true,
    });
    let state = transcriptSessionReducer(initialTranscriptSessionState, {
      type: "segment/received",
      segment: finalSeg,
    });
    state = transcriptSessionReducer(state, { type: "session/cleared" });

    expect(state).toEqual({ segments: [], lastError: null });
  });

  it("retains unrelated segment object identity when a different id is updated", () => {
    const first = segment({
      id: "mic-1",
      source: "microphone",
      text: "first",
      startedAtMs: 1000,
    });
    const second = segment({
      id: "mic-2",
      source: "microphone",
      text: "second",
      startedAtMs: 2000,
    });
    const secondUpdated = segment({
      id: "mic-2",
      source: "microphone",
      text: "second updated",
      startedAtMs: 2000,
    });

    let state = transcriptSessionReducer(initialTranscriptSessionState, {
      type: "segment/received",
      segment: first,
    });
    state = transcriptSessionReducer(state, {
      type: "segment/received",
      segment: second,
    });
    const firstRefBeforeUnrelatedUpdate = state.segments[0];
    state = transcriptSessionReducer(state, {
      type: "segment/received",
      segment: secondUpdated,
    });

    expect(state.segments[0]).toBe(firstRefBeforeUnrelatedUpdate);
  });

  it("preserves intentionally incorrect grammar, fillers, casing, punctuation, and spacing verbatim", () => {
    const raw = "  Um, I actually have went  there yesterday,  didn't I??  ";
    const finalSeg = segment({
      id: "mic-raw",
      source: "microphone",
      text: raw,
      startedAtMs: 1000,
      isFinal: true,
    });

    const state = transcriptSessionReducer(initialTranscriptSessionState, {
      type: "segment/received",
      segment: finalSeg,
    });

    expect(state.segments[0].text).toBe(raw);
  });
});

describe("formatTranscriptSegment", () => {
  it("returns microphone text unprefixed", () => {
    expect(
      formatTranscriptSegment(
        segment({ id: "1", source: "microphone", text: "hello" }),
      ),
    ).toBe("hello");
  });

  it("prefixes system text with exactly '- '", () => {
    expect(
      formatTranscriptSegment(
        segment({ id: "1", source: "system", text: "hello" }),
      ),
    ).toBe("- hello");
  });

  it("does not trim, correct, or normalize text", () => {
    const raw = "I didn't knew   him.";
    expect(
      formatTranscriptSegment(
        segment({ id: "1", source: "microphone", text: raw }),
      ),
    ).toBe(raw);
  });
});

describe("serializeFinalTranscript", () => {
  it("returns an empty string when no segment is final", () => {
    const segments = [
      segment({
        id: "1",
        source: "microphone",
        text: "interim only",
        isFinal: false,
      }),
    ];
    expect(serializeFinalTranscript(segments)).toBe("");
  });

  it("includes only finalized segments, joined by exactly one blank line, in array order", () => {
    const segments = [
      segment({
        id: "1",
        source: "microphone",
        text: "I actually have went there yesterday.",
        isFinal: true,
      }),
      segment({
        id: "2",
        source: "system",
        text: "Why did you go there?",
        isFinal: true,
      }),
      segment({
        id: "3",
        source: "microphone",
        text: "still interim",
        isFinal: false,
      }),
      segment({
        id: "4",
        source: "microphone",
        text: "Um, because my friend invited me and I didn't knew anyone there.",
        isFinal: true,
      }),
      segment({ id: "5", source: "system", text: "Oh, okay.", isFinal: true }),
    ];

    expect(serializeFinalTranscript(segments)).toBe(
      "I actually have went there yesterday.\n\n- Why did you go there?\n\nUm, because my friend invited me and I didn't knew anyone there.\n\n- Oh, okay.",
    );
  });

  it("produces no leading or trailing newline", () => {
    const segments = [
      segment({ id: "1", source: "microphone", text: "only line", isFinal: true }),
    ];
    const result = serializeFinalTranscript(segments);
    expect(result.startsWith("\n")).toBe(false);
    expect(result.endsWith("\n")).toBe(false);
    expect(result).toBe("only line");
  });
});
