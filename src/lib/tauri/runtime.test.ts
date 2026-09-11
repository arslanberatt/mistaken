import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";

import {
  getRuntimeSnapshot,
  isRuntimeBridgeError,
  listMicrophones,
  parseAudioSourceStatus,
  parseAudioStatusEvent,
  parseCaptureStatus,
  parseCaptureStatusEvent,
  parseMicrophoneDeviceList,
  parseModelStatus,
  parseModelStatusEvent,
  parseRuntimeError,
  parseRuntimeSnapshot,
  parseTranscriptSegment,
  startCapture,
  stopCapture,
} from "./runtime";
import type { RuntimeSnapshot } from "./contracts";

function validSnapshot(): RuntimeSnapshot {
  return {
    revision: 0,
    captureStatus: "idle",
    modelStatus: { status: "missing" },
    microphone: {
      status: "unavailable",
      error: { code: "runtime_unavailable", message: "n/a", recoverable: true },
    },
    systemAudio: {
      status: "unavailable",
      error: { code: "runtime_unavailable", message: "n/a", recoverable: true },
    },
  };
}

describe("parseRuntimeError", () => {
  it("accepts a full error with an optional source", () => {
    const error = parseRuntimeError({
      code: "device_disconnected",
      message: "microphone unplugged",
      recoverable: true,
      source: "microphone",
    });
    expect(error).toStrictEqual({
      code: "device_disconnected",
      message: "microphone unplugged",
      recoverable: true,
      source: "microphone",
    });
  });

  it("accepts an error without a source and omits the field", () => {
    const error = parseRuntimeError({
      code: "internal",
      message: "oops",
      recoverable: false,
    });
    expect(error.source).toBeUndefined();
  });

  it.each([
    ["unknown code", { code: "bogus", message: "m", recoverable: true }],
    ["non-string message", { code: "internal", message: 1, recoverable: true }],
    ["non-boolean recoverable", { code: "internal", message: "m", recoverable: "yes" }],
    [
      "invalid source",
      { code: "internal", message: "m", recoverable: true, source: "robot" },
    ],
    ["not an object", "internal"],
  ])("rejects %s", (_label, value) => {
    expect(() => parseRuntimeError(value)).toThrow();
  });
});

describe("parseModelStatus", () => {
  it("accepts every documented variant", () => {
    expect(parseModelStatus({ status: "missing" })).toStrictEqual({ status: "missing" });
    expect(parseModelStatus({ status: "loading" })).toStrictEqual({ status: "loading" });
    expect(parseModelStatus({ status: "loading", modelId: "candidate" })).toStrictEqual({
      status: "loading",
      modelId: "candidate",
    });
    expect(parseModelStatus({ status: "ready", modelId: "en-2023" })).toStrictEqual({
      status: "ready",
      modelId: "en-2023",
    });
    const failure = { code: "model_load_failed", message: "m", recoverable: false } as const;
    expect(parseModelStatus({ status: "failed", error: failure })).toStrictEqual({
      status: "failed",
      error: failure,
    });
  });

  it("rejects ready without a modelId and an unknown status", () => {
    expect(() => parseModelStatus({ status: "ready" })).toThrow();
    expect(() => parseModelStatus({ status: "loaded" })).toThrow();
  });
});

describe("parseAudioSourceStatus", () => {
  it("accepts every documented variant including capturing activity", () => {
    expect(parseAudioSourceStatus({ status: "idle" })).toStrictEqual({ status: "idle" });
    expect(
      parseAudioSourceStatus({ status: "starting", deviceId: "mic-1" }),
    ).toStrictEqual({ status: "starting", deviceId: "mic-1" });
    expect(
      parseAudioSourceStatus({
        status: "capturing",
        deviceId: "mic-1",
        activity: "receiving",
      }),
    ).toStrictEqual({ status: "capturing", deviceId: "mic-1", activity: "receiving" });
    expect(
      parseAudioSourceStatus({ status: "capturing", activity: "waiting" }),
    ).toStrictEqual({ status: "capturing", activity: "waiting" });
  });

  it("rejects an invalid activity and an invalid deviceId type", () => {
    expect(() =>
      parseAudioSourceStatus({ status: "capturing", activity: "listening" }),
    ).toThrow();
    expect(() =>
      parseAudioSourceStatus({ status: "starting", deviceId: 5 }),
    ).toThrow();
  });
});

describe("parseRuntimeSnapshot", () => {
  it("accepts a fully valid snapshot", () => {
    expect(parseRuntimeSnapshot(validSnapshot())).toStrictEqual(validSnapshot());
  });

  it.each([
    ["negative revision", { ...validSnapshot(), revision: -1 }],
    ["fractional revision", { ...validSnapshot(), revision: 1.5 }],
    ["unsafe integer revision", { ...validSnapshot(), revision: 2 ** 53 }],
    ["invalid captureStatus", { ...validSnapshot(), captureStatus: "paused" }],
  ])("rejects %s", (_label, value) => {
    expect(() => parseRuntimeSnapshot(value)).toThrow();
  });
});

describe("parseMicrophoneDeviceList", () => {
  it("accepts a valid device array", () => {
    const devices = [{ id: "1", label: "Built-in", isDefault: true }];
    expect(parseMicrophoneDeviceList(devices)).toStrictEqual(devices);
  });

  it("rejects a non-array and an entry missing a field", () => {
    expect(() => parseMicrophoneDeviceList({})).toThrow();
    expect(() =>
      parseMicrophoneDeviceList([{ id: "1", label: "Built-in" }]),
    ).toThrow();
  });
});

describe("parseCaptureStatus", () => {
  it("accepts a known status and rejects an unknown one", () => {
    expect(parseCaptureStatus("listening")).toBe("listening");
    expect(() => parseCaptureStatus("paused")).toThrow();
  });
});

describe("parseTranscriptSegment", () => {
  it("preserves ungrammatical text verbatim for a partial segment", () => {
    const raw = {
      id: "mic-1-0",
      source: "microphone",
      text: "I didn't knew anyone there",
      startedAtMs: 100,
      isFinal: false,
    };
    expect(parseTranscriptSegment(raw, false)).toStrictEqual(raw);
  });

  it("preserves a final segment including endedAtMs without rewriting text", () => {
    const raw = {
      id: "sys-1-0",
      source: "system",
      text: "- Why not?",
      startedAtMs: 100,
      endedAtMs: 400,
      isFinal: true,
    };
    expect(parseTranscriptSegment(raw, true)).toStrictEqual(raw);
  });

  it("rejects an isFinal mismatch against the event it arrived on", () => {
    const finalLookingPayload = {
      id: "mic-1-0",
      source: "microphone",
      text: "hello",
      startedAtMs: 0,
      isFinal: true,
    };
    expect(() => parseTranscriptSegment(finalLookingPayload, false)).toThrow();
  });

  it("rejects an invalid source and a missing id", () => {
    expect(() =>
      parseTranscriptSegment(
        { id: "1", source: "robot", text: "x", startedAtMs: 0, isFinal: false },
        false,
      ),
    ).toThrow();
    expect(() =>
      parseTranscriptSegment(
        { id: "", source: "microphone", text: "x", startedAtMs: 0, isFinal: false },
        false,
      ),
    ).toThrow();
  });
});

describe("event envelope parsers", () => {
  it("parses capture:status, audio:status, and asr:model-status payloads", () => {
    const snapshot = validSnapshot();
    expect(parseCaptureStatusEvent({ snapshot })).toStrictEqual({ snapshot });
    expect(parseModelStatusEvent({ snapshot })).toStrictEqual({ snapshot });
    expect(
      parseAudioStatusEvent({ source: "system", snapshot }),
    ).toStrictEqual({ source: "system", snapshot });
  });

  it("rejects an audio:status payload with an invalid source", () => {
    expect(() =>
      parseAudioStatusEvent({ source: "robot", snapshot: validSnapshot() }),
    ).toThrow();
  });
});

describe("isRuntimeBridgeError", () => {
  it("distinguishes a bridge error from a native RuntimeError", () => {
    expect(
      isRuntimeBridgeError({
        code: "invalid_event_payload",
        operation: "capture:status",
        message: "bad",
      }),
    ).toBe(true);
    expect(
      isRuntimeBridgeError({
        code: "runtime_unavailable",
        message: "bad",
        recoverable: true,
      }),
    ).toBe(false);
  });
});

describe("typed command wrappers", () => {
  beforeEach(() => {
    mockIPC(() => {
      throw new Error("no handler installed for this test");
    });
  });

  afterEach(() => {
    clearMocks();
  });

  it("getRuntimeSnapshot resolves a validated snapshot", async () => {
    const snapshot = validSnapshot();
    mockIPC((cmd) => {
      expect(cmd).toBe("get_runtime_snapshot");
      return snapshot;
    });

    await expect(getRuntimeSnapshot()).resolves.toStrictEqual(snapshot);
  });

  it("getRuntimeSnapshot rejects a malformed success with a bridge error", async () => {
    mockIPC(() => ({ revision: "not-a-number" }));

    await expect(getRuntimeSnapshot()).rejects.toMatchObject({
      code: "invalid_command_response",
      operation: "get_runtime_snapshot",
    });
  });

  it("listMicrophones rejects with the native RuntimeError on a structured failure", async () => {
    mockIPC(() => {
      throw {
        code: "runtime_unavailable",
        message: "not configured",
        recoverable: true,
      };
    });

    await expect(listMicrophones()).rejects.toStrictEqual({
      code: "runtime_unavailable",
      message: "not configured",
      recoverable: true,
    });
  });

  it("listMicrophones surfaces an unrecognizable rejection as a bridge error", async () => {
    mockIPC(() => {
      throw "totally unexpected";
    });

    await expect(listMicrophones()).rejects.toMatchObject({
      code: "invalid_command_error",
      operation: "list_microphones",
    });
  });

  it("startCapture serializes the request under a `request` key and validates the response", async () => {
    mockIPC((cmd, args) => {
      expect(cmd).toBe("start_capture");
      expect(args).toStrictEqual({
        request: { microphoneDeviceId: "default", systemAudioEnabled: true },
      });
      throw { code: "runtime_unavailable", message: "n/a", recoverable: true };
    });

    await expect(
      startCapture({ microphoneDeviceId: "default", systemAudioEnabled: true }),
    ).rejects.toMatchObject({ code: "runtime_unavailable" });
  });

  it("stopCapture resolves a validated capture status", async () => {
    mockIPC((cmd) => {
      expect(cmd).toBe("stop_capture");
      return "idle";
    });

    await expect(stopCapture()).resolves.toBe("idle");
  });
});
