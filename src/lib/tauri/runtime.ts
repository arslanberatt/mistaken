/**
 * Runtime-validated typed command wrappers.
 *
 * Every Tauri payload is a trust boundary despite the TypeScript types in
 * `contracts.ts`. Every value returned here has been validated field by
 * field; a malformed native payload never reaches a caller unvalidated. No
 * command/event payload is logged.
 *
 * No schema package is added for this small, closed payload set (see
 * Spec 03); validation is hand-written and allocation-conscious. `isRecord`
 * is this module's one canonical object guard: every use narrows to
 * `Record<string, unknown>` and is immediately followed by explicit
 * per-field checks, never trusted on its own.
 */
import { invoke } from "@tauri-apps/api/core";

import type {
  AudioSourceStatus,
  AudioStatusEvent,
  CaptureStatus,
  CaptureStatusEvent,
  MicrophoneDevice,
  ModelStatus,
  ModelStatusEvent,
  RuntimeBridgeError,
  RuntimeBridgeErrorCode,
  RuntimeError,
  RuntimeErrorCode,
  RuntimeSnapshot,
  StartCaptureRequest,
  TranscriptSegment,
  TranscriptSource,
} from "./contracts";
import { RUNTIME_COMMANDS } from "./contracts";

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** A payload failed structural validation. Never exposed to callers. */
export class PayloadShapeError extends Error {}

const CAPTURE_STATUSES: readonly CaptureStatus[] = [
  "idle",
  "starting",
  "listening",
  "stopping",
  "error",
];

function isCaptureStatus(value: unknown): value is CaptureStatus {
  return (
    typeof value === "string" &&
    (CAPTURE_STATUSES as readonly string[]).includes(value)
  );
}

const TRANSCRIPT_SOURCES: readonly TranscriptSource[] = ["microphone", "system"];

export function isTranscriptSource(value: unknown): value is TranscriptSource {
  return (
    typeof value === "string" &&
    (TRANSCRIPT_SOURCES as readonly string[]).includes(value)
  );
}

const RUNTIME_ERROR_CODES: readonly RuntimeErrorCode[] = [
  "runtime_unavailable",
  "unsupported_platform",
  "invalid_request",
  "model_missing",
  "model_load_failed",
  "model_unsupported",
  "microphone_permission_denied",
  "system_audio_permission_denied",
  "microphone_unavailable",
  "system_audio_unavailable",
  "device_disconnected",
  "capture_already_active",
  "capture_not_active",
  "capture_start_failed",
  "capture_stop_failed",
  "audio_queue_overflow",
  "inference_lagging",
  "internal",
];

function isRuntimeErrorCode(value: unknown): value is RuntimeErrorCode {
  return (
    typeof value === "string" &&
    (RUNTIME_ERROR_CODES as readonly string[]).includes(value)
  );
}

export function parseRuntimeError(value: unknown): RuntimeError {
  if (!isRecord(value)) throw new PayloadShapeError("RuntimeError must be an object");
  const { code, message, recoverable, source } = value;
  if (!isRuntimeErrorCode(code)) {
    throw new PayloadShapeError("RuntimeError.code is invalid");
  }
  if (typeof message !== "string") {
    throw new PayloadShapeError("RuntimeError.message is invalid");
  }
  if (typeof recoverable !== "boolean") {
    throw new PayloadShapeError("RuntimeError.recoverable is invalid");
  }
  if (source !== undefined && !isTranscriptSource(source)) {
    throw new PayloadShapeError("RuntimeError.source is invalid");
  }

  const error: RuntimeError = { code, message, recoverable };
  if (source !== undefined) {
    error.source = source;
  }
  return error;
}

export function parseModelStatus(value: unknown): ModelStatus {
  if (!isRecord(value)) throw new PayloadShapeError("ModelStatus must be an object");

  switch (value.status) {
    case "missing":
      return { status: "missing" };
    case "loading": {
      const { modelId } = value;
      if (modelId !== undefined && typeof modelId !== "string") {
        throw new PayloadShapeError("ModelStatus.loading.modelId is invalid");
      }
      return modelId === undefined
        ? { status: "loading" }
        : { status: "loading", modelId };
    }
    case "ready": {
      if (typeof value.modelId !== "string") {
        throw new PayloadShapeError("ModelStatus.ready.modelId is invalid");
      }
      return { status: "ready", modelId: value.modelId };
    }
    case "failed":
      return { status: "failed", error: parseRuntimeError(value.error) };
    case "unsupported":
      return { status: "unsupported", error: parseRuntimeError(value.error) };
    default:
      throw new PayloadShapeError("ModelStatus.status is invalid");
  }
}

/** Shared by `starting`/`capturing`/`stopping`: all three carry the same
 * optional device identifier with identical validation. */
function parseOptionalDeviceId(value: unknown): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value !== "string") throw new PayloadShapeError("deviceId is invalid");
  return value;
}

export function parseAudioSourceStatus(value: unknown): AudioSourceStatus {
  if (!isRecord(value)) {
    throw new PayloadShapeError("AudioSourceStatus must be an object");
  }

  switch (value.status) {
    case "unavailable":
      return { status: "unavailable", error: parseRuntimeError(value.error) };
    case "idle":
      return { status: "idle" };
    case "starting": {
      const deviceId = parseOptionalDeviceId(value.deviceId);
      return deviceId === undefined
        ? { status: "starting" }
        : { status: "starting", deviceId };
    }
    case "capturing": {
      const deviceId = parseOptionalDeviceId(value.deviceId);
      if (value.activity !== "waiting" && value.activity !== "receiving") {
        throw new PayloadShapeError(
          "AudioSourceStatus.capturing.activity is invalid",
        );
      }
      const activity = value.activity;
      return deviceId === undefined
        ? { status: "capturing", activity }
        : { status: "capturing", deviceId, activity };
    }
    case "stopping": {
      const deviceId = parseOptionalDeviceId(value.deviceId);
      return deviceId === undefined
        ? { status: "stopping" }
        : { status: "stopping", deviceId };
    }
    case "error":
      return { status: "error", error: parseRuntimeError(value.error) };
    default:
      throw new PayloadShapeError("AudioSourceStatus.status is invalid");
  }
}

export function parseRuntimeSnapshot(value: unknown): RuntimeSnapshot {
  if (!isRecord(value)) {
    throw new PayloadShapeError("RuntimeSnapshot must be an object");
  }
  if (
    typeof value.revision !== "number" ||
    !Number.isSafeInteger(value.revision) ||
    value.revision < 0
  ) {
    throw new PayloadShapeError("RuntimeSnapshot.revision is invalid");
  }
  if (!isCaptureStatus(value.captureStatus)) {
    throw new PayloadShapeError("RuntimeSnapshot.captureStatus is invalid");
  }

  return {
    revision: value.revision,
    captureStatus: value.captureStatus,
    modelStatus: parseModelStatus(value.modelStatus),
    microphone: parseAudioSourceStatus(value.microphone),
    systemAudio: parseAudioSourceStatus(value.systemAudio),
  };
}

export function parseMicrophoneDevice(value: unknown): MicrophoneDevice {
  if (!isRecord(value)) {
    throw new PayloadShapeError("MicrophoneDevice must be an object");
  }
  const { id, label, isDefault } = value;
  if (typeof id !== "string" || id.length === 0) {
    throw new PayloadShapeError("MicrophoneDevice.id is invalid");
  }
  if (typeof label !== "string") {
    throw new PayloadShapeError("MicrophoneDevice.label is invalid");
  }
  if (typeof isDefault !== "boolean") {
    throw new PayloadShapeError("MicrophoneDevice.isDefault is invalid");
  }
  return { id, label, isDefault };
}

export function parseMicrophoneDeviceList(
  value: unknown,
): readonly MicrophoneDevice[] {
  if (!Array.isArray(value)) {
    throw new PayloadShapeError("microphone device list must be an array");
  }
  return value.map(parseMicrophoneDevice);
}

export function parseCaptureStatus(value: unknown): CaptureStatus {
  if (!isCaptureStatus(value)) throw new PayloadShapeError("CaptureStatus is invalid");
  return value;
}

/**
 * Validates a transcript segment exactly as received: no trimming,
 * normalization, spell/grammar correction, prefixing, or merging. `text`
 * and identity fields are forwarded byte-for-byte. `expectedIsFinal` must
 * match the segment's `isFinal` flag, tying the value to the event name it
 * arrived on (`transcript:partial` vs `transcript:final`).
 */
export function parseTranscriptSegment(
  value: unknown,
  expectedIsFinal: boolean,
): TranscriptSegment {
  if (!isRecord(value)) {
    throw new PayloadShapeError("TranscriptSegment must be an object");
  }
  const { id, source, text, startedAtMs, endedAtMs, isFinal } = value;

  if (typeof id !== "string" || id.length === 0) {
    throw new PayloadShapeError("TranscriptSegment.id is invalid");
  }
  if (!isTranscriptSource(source)) {
    throw new PayloadShapeError("TranscriptSegment.source is invalid");
  }
  if (typeof text !== "string") {
    throw new PayloadShapeError("TranscriptSegment.text is invalid");
  }
  if (typeof startedAtMs !== "number" || !Number.isFinite(startedAtMs)) {
    throw new PayloadShapeError("TranscriptSegment.startedAtMs is invalid");
  }
  if (
    endedAtMs !== undefined &&
    (typeof endedAtMs !== "number" || !Number.isFinite(endedAtMs))
  ) {
    throw new PayloadShapeError("TranscriptSegment.endedAtMs is invalid");
  }
  if (typeof isFinal !== "boolean" || isFinal !== expectedIsFinal) {
    throw new PayloadShapeError(
      "TranscriptSegment.isFinal does not match its event name",
    );
  }

  const segment: TranscriptSegment = { id, source, text, startedAtMs, isFinal };
  if (endedAtMs !== undefined) {
    segment.endedAtMs = endedAtMs;
  }
  return segment;
}

export function parseCaptureStatusEvent(value: unknown): CaptureStatusEvent {
  if (!isRecord(value)) {
    throw new PayloadShapeError("CaptureStatusEvent must be an object");
  }
  return { snapshot: parseRuntimeSnapshot(value.snapshot) };
}

export function parseAudioStatusEvent(value: unknown): AudioStatusEvent {
  if (!isRecord(value)) {
    throw new PayloadShapeError("AudioStatusEvent must be an object");
  }
  if (!isTranscriptSource(value.source)) {
    throw new PayloadShapeError("AudioStatusEvent.source is invalid");
  }
  return { source: value.source, snapshot: parseRuntimeSnapshot(value.snapshot) };
}

export function parseModelStatusEvent(value: unknown): ModelStatusEvent {
  if (!isRecord(value)) {
    throw new PayloadShapeError("ModelStatusEvent must be an object");
  }
  return { snapshot: parseRuntimeSnapshot(value.snapshot) };
}

const RUNTIME_BRIDGE_ERROR_CODES: readonly RuntimeBridgeErrorCode[] = [
  "listener_registration_failed",
  "command_failed",
  "invalid_command_response",
  "invalid_command_error",
  "invalid_event_payload",
];

/** Distinguishes a bridge-level failure from a native `RuntimeError`: the
 * two error unions share a `code: string` field but draw from disjoint
 * code sets, so checking membership is an exact discriminant. */
export function isRuntimeBridgeError(value: unknown): value is RuntimeBridgeError {
  return (
    isRecord(value) &&
    typeof value.operation === "string" &&
    typeof value.message === "string" &&
    typeof value.code === "string" &&
    (RUNTIME_BRIDGE_ERROR_CODES as readonly string[]).includes(value.code)
  );
}

/** Constructs a `RuntimeBridgeError`. Shared across command wrappers here
 * and the event handlers in `use-runtime-bridge.ts` so every bridge-level
 * failure is built the same way. */
export function bridgeError(
  code: RuntimeBridgeErrorCode,
  operation: string,
  message: string,
): RuntimeBridgeError {
  return { code, operation, message };
}

/**
 * Invokes a native command, validating both the success and rejection
 * channels. Resolves with the validated value, or rejects with either the
 * native `RuntimeError` (a real domain failure) or a `RuntimeBridgeError`
 * (the webview/native boundary itself misbehaved).
 */
async function invokeCommand<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  parseSuccess: (value: unknown) => T,
): Promise<T> {
  let raw: unknown;
  try {
    raw = await invoke(command, args);
  } catch (rejection) {
    try {
      throw parseRuntimeError(rejection);
    } catch (parsed) {
      if (parsed instanceof PayloadShapeError) {
        throw bridgeError(
          "invalid_command_error",
          command,
          `received a malformed error from ${command}`,
        );
      }
      throw parsed;
    }
  }

  try {
    return parseSuccess(raw);
  } catch (error) {
    if (error instanceof PayloadShapeError) {
      throw bridgeError(
        "invalid_command_response",
        command,
        `received a malformed response from ${command}`,
      );
    }
    throw error;
  }
}

export async function getRuntimeSnapshot(): Promise<RuntimeSnapshot> {
  return invokeCommand(RUNTIME_COMMANDS.getSnapshot, undefined, parseRuntimeSnapshot);
}

export async function listMicrophones(): Promise<readonly MicrophoneDevice[]> {
  return invokeCommand(
    RUNTIME_COMMANDS.listMicrophones,
    undefined,
    parseMicrophoneDeviceList,
  );
}

export async function startCapture(
  request: StartCaptureRequest,
): Promise<CaptureStatus> {
  return invokeCommand(
    RUNTIME_COMMANDS.startCapture,
    { request },
    parseCaptureStatus,
  );
}

export async function stopCapture(): Promise<CaptureStatus> {
  return invokeCommand(RUNTIME_COMMANDS.stopCapture, undefined, parseCaptureStatus);
}

/** A stable typed client for the four runtime commands. */
export interface RuntimeClient {
  getRuntimeSnapshot(): Promise<RuntimeSnapshot>;
  listMicrophones(): Promise<readonly MicrophoneDevice[]>;
  startCapture(request: StartCaptureRequest): Promise<CaptureStatus>;
  stopCapture(): Promise<CaptureStatus>;
}

export const runtimeClient: RuntimeClient = {
  getRuntimeSnapshot,
  listMicrophones,
  startCapture,
  stopCapture,
};
