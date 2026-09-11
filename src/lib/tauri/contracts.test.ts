import { describe, expect, it } from "vitest";
import { NATIVE_EVENTS, RUNTIME_COMMANDS } from "./contracts";

describe("RUNTIME_COMMANDS", () => {
  it("names exactly the four frozen application commands", () => {
    expect(RUNTIME_COMMANDS).toStrictEqual({
      getSnapshot: "get_runtime_snapshot",
      listMicrophones: "list_microphones",
      startCapture: "start_capture",
      stopCapture: "stop_capture",
    });
  });
});

describe("NATIVE_EVENTS", () => {
  it("names exactly the six frozen native events", () => {
    expect(NATIVE_EVENTS).toStrictEqual({
      captureStatus: "capture:status",
      audioStatus: "audio:status",
      modelStatus: "asr:model-status",
      transcriptPartial: "transcript:partial",
      transcriptFinal: "transcript:final",
      captureError: "capture:error",
    });
  });
});
