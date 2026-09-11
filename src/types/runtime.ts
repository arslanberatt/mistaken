/**
 * Shared native runtime status contract.
 *
 * Frozen for Wave 2 (Specs 02 and 03 consume this union without redefining
 * it). Spec 01 allocates no capture resources; later specs drive real
 * transitions through this same union.
 */

/** Lifecycle status of native audio capture. */
export type CaptureStatus =
  | "idle"
  | "starting"
  | "listening"
  | "stopping"
  | "error";
