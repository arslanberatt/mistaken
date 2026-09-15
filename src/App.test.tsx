import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import App from "./App";
import type { RuntimeSnapshot, TranscriptSegment } from "./lib/tauri";
import type * as RuntimeModule from "./lib/tauri/runtime";

const { listen } = vi.hoisted(() => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

const {
  getRuntimeSnapshot,
  listMicrophones,
  startCapture,
  stopCapture,
} = vi.hoisted(() => ({
  getRuntimeSnapshot: vi.fn(),
  listMicrophones: vi.fn(),
  startCapture: vi.fn(),
  stopCapture: vi.fn(),
}));

vi.mock("./lib/tauri/runtime", async (importOriginal) => {
  const actual = await importOriginal<typeof RuntimeModule>();
  return {
    ...actual,
    getRuntimeSnapshot,
    listMicrophones,
    startCapture,
    stopCapture,
    runtimeClient: {
      getRuntimeSnapshot,
      listMicrophones,
      startCapture,
      stopCapture,
    },
  };
});

function mockSnapshot(overrides: Partial<RuntimeSnapshot> = {}): RuntimeSnapshot {
  return {
    revision: 1,
    captureStatus: "idle",
    modelStatus: { status: "ready", modelId: "sherpa-zipformer-en-20M-2023-02-17-int8" },
    microphone: { status: "idle" },
    systemAudio: { status: "idle" },
    ...overrides,
  };
}

describe("App dual-source integration", () => {
  it("renders persistently visible Development ASR label and system audio toggle", async () => {
    let captureStatusHandler:
      | ((event: { payload: { snapshot: RuntimeSnapshot } }) => void)
      | undefined;
    let transcriptHandler: ((event: { payload: TranscriptSegment }) => void) | undefined;
    listen.mockImplementation(async (event: string, handler: (event: { payload: unknown }) => void) => {
      if (event === "transcript:final") {
        transcriptHandler = handler as (event: { payload: TranscriptSegment }) => void;
      }
      if (event === "capture:status") {
        captureStatusHandler = handler as (event: { payload: { snapshot: RuntimeSnapshot } }) => void;
      }
      return async () => {};
    });

    getRuntimeSnapshot.mockResolvedValue(mockSnapshot());
    listMicrophones.mockResolvedValue([
      { id: "mic-1", label: "Built-in Microphone", isDefault: true },
    ]);
    startCapture.mockResolvedValue("listening");
    stopCapture.mockResolvedValue("idle");

    await act(async () => {
      render(<App />);
    });

    // Verification of persistent maturity label (AC 12, AC 20)
    expect(screen.getByText(/Development ASR • Not release approved/)).toBeInTheDocument();

    // Verification of system audio control (AC 5, AC 8)
    const toggle = screen.getByRole("checkbox", { name: /System Audio/i });
    expect(toggle).not.toBeChecked();
    expect(screen.getByText("Off")).toBeInTheDocument();

    // Toggle system audio ON
    await act(async () => {
      fireEvent.click(toggle);
    });
    expect(toggle).toBeChecked();
    expect(screen.getByText("Ready")).toBeInTheDocument();

    // Start Listening with both sources enabled
    const startButton = screen.getByRole("button", { name: "Start Listening" });
    await act(async () => {
      fireEvent.click(startButton);
    });

    expect(startCapture).toHaveBeenCalledWith({
      microphoneDeviceId: "mic-1",
      systemAudioEnabled: true,
    });

    // Simulate transition to listening
    await act(async () => {
      captureStatusHandler?.({
        payload: {
          snapshot: mockSnapshot({ captureStatus: "listening" }),
        },
      });
    });

    // Feed a microphone segment and a system segment
    await act(async () => {
      transcriptHandler?.({
        payload: {
          id: "mic-1-0",
          source: "microphone",
          text: "I actually went there.",
          startedAtMs: 100,
          endedAtMs: 500,
          isFinal: true,
        },
      });
      transcriptHandler?.({
        payload: {
          id: "sys-1-0",
          source: "system",
          text: "Why did you go?",
          startedAtMs: 600,
          endedAtMs: 900,
          isFinal: true,
        },
      });
    });

    // Microphone segment renders without prefix (AC 7)
    expect(screen.getByText("I actually went there.")).toBeInTheDocument();
    // System audio segment renders with "- " prefix (AC 7)
    expect(screen.getByText("- Why did you go?")).toBeInTheDocument();

    // Stop listening
    const stopButton = screen.getByRole("button", { name: "Stop" });
    await act(async () => {
      fireEvent.click(stopButton);
    });
    expect(stopCapture).toHaveBeenCalled();
  });
});
