/**
 * Typed Tauri IPC contract.
 *
 * This module declares every command name, event name, and payload shape
 * that crosses the frontend/native boundary. It imports the shared
 * transcript/runtime domain types from `src/types/**` instead of
 * redeclaring them. Nothing here performs runtime validation; see
 * `runtime.ts` for the closed set of validators that guard this contract
 * at the trust boundary.
 */
import type { CaptureStatus } from "../../types/runtime";
import type { TranscriptSegment, TranscriptSource } from "../../types/transcript";

export type { CaptureStatus, TranscriptSegment, TranscriptSource };

/** The only four application commands registered by the native runtime. */
export const RUNTIME_COMMANDS = {
  getSnapshot: "get_runtime_snapshot",
  listMicrophones: "list_microphones",
  startCapture: "start_capture",
  stopCapture: "stop_capture",
} as const;

/** The exact six native event names. Native emission targets `main` only. */
export const NATIVE_EVENTS = {
  captureStatus: "capture:status",
  audioStatus: "audio:status",
  modelStatus: "asr:model-status",
  transcriptPartial: "transcript:partial",
  transcriptFinal: "transcript:final",
  captureError: "capture:error",
} as const;

export interface StartCaptureRequest {
  microphoneDeviceId: string | null;
  systemAudioEnabled: boolean;
}

export interface MicrophoneDevice {
  id: string;
  label: string;
  isDefault: boolean;
}

export type RuntimeErrorCode =
  | "runtime_unavailable"
  | "unsupported_platform"
  | "invalid_request"
  | "model_missing"
  | "model_load_failed"
  | "model_unsupported"
  | "microphone_permission_denied"
  | "system_audio_permission_denied"
  | "microphone_unavailable"
  | "system_audio_unavailable"
  | "device_disconnected"
  | "capture_already_active"
  | "capture_not_active"
  | "capture_start_failed"
  | "capture_stop_failed"
  | "audio_queue_overflow"
  | "inference_lagging"
  | "internal";

export interface RuntimeError {
  code: RuntimeErrorCode;
  message: string;
  recoverable: boolean;
  source?: TranscriptSource;
}

export type ModelStatus =
  | { status: "missing" }
  | { status: "loading"; modelId?: string }
  | { status: "ready"; modelId: string }
  | { status: "failed"; error: RuntimeError }
  | { status: "unsupported"; error: RuntimeError };

export type AudioSourceStatus =
  | { status: "unavailable"; error: RuntimeError }
  | { status: "idle" }
  | { status: "starting"; deviceId?: string }
  | {
      status: "capturing";
      deviceId?: string;
      activity: "waiting" | "receiving";
    }
  | { status: "stopping"; deviceId?: string }
  | { status: "error"; error: RuntimeError };

export interface RuntimeSnapshot {
  revision: number;
  captureStatus: CaptureStatus;
  modelStatus: ModelStatus;
  microphone: AudioSourceStatus;
  systemAudio: AudioSourceStatus;
}

export interface CaptureStatusEvent {
  snapshot: RuntimeSnapshot;
}

export interface AudioStatusEvent {
  source: TranscriptSource;
  snapshot: RuntimeSnapshot;
}

export interface ModelStatusEvent {
  snapshot: RuntimeSnapshot;
}

export interface RuntimeBridgeState {
  snapshot: RuntimeSnapshot | null;
  bridgeReady: boolean;
  bridgeError: RuntimeBridgeError | null;
}

export interface RuntimeBridgeHandlers {
  onCaptureStatus?(event: CaptureStatusEvent): void;
  onAudioStatus?(event: AudioStatusEvent): void;
  onModelStatus?(event: ModelStatusEvent): void;
  onTranscriptSegment?(segment: TranscriptSegment): void;
  onCaptureError?(error: RuntimeError): void;
}

export type RuntimeBridgeErrorCode =
  | "listener_registration_failed"
  | "command_failed"
  | "invalid_command_response"
  | "invalid_command_error"
  | "invalid_event_payload";

export interface RuntimeBridgeError {
  code: RuntimeBridgeErrorCode;
  operation: string;
  message: string;
}
