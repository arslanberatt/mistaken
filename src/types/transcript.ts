/**
 * Shared transcript domain contract.
 *
 * Frozen for Wave 2 (Specs 02 and 03 consume these shapes without redefining
 * them). Microphone speech renders as plain text; system-audio speech is
 * prefixed with `- ` by formatting logic that lives outside this module.
 */

/** Structural origin of a transcript segment. Never inferred from text. */
export type TranscriptSource = "microphone" | "system";

/** A single partial or finalized recognized-speech segment. */
export interface TranscriptSegment {
  id: string;
  source: TranscriptSource;
  text: string;
  startedAtMs: number;
  endedAtMs?: number;
  isFinal: boolean;
}
