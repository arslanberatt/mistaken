import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AudioSourceStatus } from "../../lib/tauri";
import { MicrophoneControl } from "./MicrophoneControl";
import type { UseMicrophoneControllerResult } from "./microphone-controller";

const { listen } = vi.hoisted(() => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

beforeEach(() => {
  listen.mockResolvedValue(vi.fn(async () => {}));
});

afterEach(() => {
  vi.clearAllMocks();
});

function captureErrorHandler(): (payload: unknown) => void {
  let handler: ((event: { payload: unknown }) => void) | undefined;
  listen.mockImplementation(async (_event: string, fn: (event: { payload: unknown }) => void) => {
    handler = fn;
    return vi.fn(async () => {});
  });
  return (payload: unknown) => {
    act(() => {
      handler?.({ payload });
    });
  };
}

function makeController(
  overrides: Partial<UseMicrophoneControllerResult> = {},
): UseMicrophoneControllerResult {
  return {
    devices: [{ id: "a", label: "Built-in Microphone", isDefault: true }],
    selectedDeviceId: "a",
    listState: "ready",
    error: null,
    refresh: vi.fn(),
    selectDevice: vi.fn(),
    ...overrides,
  };
}

describe("MicrophoneControl", () => {
  it("shows the bridge-pending state and disables every control", () => {
    render(
      <MicrophoneControl
        bridgeReady={false}
        controller={makeController()}
        microphoneStatus={undefined}
        overflowWarning={false}
      />,
    );

    expect(screen.getByText("Connecting to local audio…")).toBeInTheDocument();
    expect(screen.getByRole("combobox")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Refresh microphones" })).toBeDisabled();
  });

  it("shows the no-devices state with Refresh enabled", () => {
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController({ devices: [], selectedDeviceId: null })}
        microphoneStatus={undefined}
        overflowWarning={false}
      />,
    );

    expect(screen.getByText("No microphone found.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh microphones" })).toBeEnabled();
    expect(screen.getByRole("combobox")).toBeDisabled();
  });

  it("shows the ready state with a real device label", () => {
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController()}
        microphoneStatus={{ status: "idle" }}
        overflowWarning={false}
      />,
    );

    expect(screen.getByText("Ready.")).toBeInTheDocument();
    expect(screen.getByText("Built-in Microphone (default)")).toBeInTheDocument();
    expect(screen.getByRole("combobox")).toBeEnabled();
  });

  it("shows waiting activity and disables the selector while capturing", () => {
    const status: AudioSourceStatus = {
      status: "capturing",
      deviceId: "a",
      activity: "waiting",
    };
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController()}
        microphoneStatus={status}
        overflowWarning={false}
      />,
    );

    expect(screen.getByText("Waiting for microphone signal…")).toBeInTheDocument();
    expect(screen.getByRole("combobox")).toBeDisabled();
  });

  it("shows receiving activity once a real signal arrives", () => {
    const status: AudioSourceStatus = {
      status: "capturing",
      deviceId: "a",
      activity: "receiving",
    };
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController()}
        microphoneStatus={status}
        overflowWarning={false}
      />,
    );

    expect(screen.getByText("Microphone active")).toBeInTheDocument();
  });

  it("shows the overflow warning instead of the active message while capturing", () => {
    const status: AudioSourceStatus = {
      status: "capturing",
      deviceId: "a",
      activity: "receiving",
    };
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController()}
        microphoneStatus={status}
        overflowWarning={true}
      />,
    );

    expect(
      screen.getByText("Local transcription is delayed; some audio was dropped."),
    ).toBeInTheDocument();
  });

  it("shows exact permission-denied guidance and keeps the selector usable", () => {
    const platformSpy = vi
      .spyOn(window.navigator, "platform", "get")
      .mockReturnValue("MacIntel");
    const status: AudioSourceStatus = {
      status: "error",
      error: {
        code: "microphone_permission_denied",
        message: "microphone access is off",
        recoverable: true,
        source: "microphone",
      },
    };
    try {
      render(
        <MicrophoneControl
          bridgeReady={true}
          controller={makeController()}
          microphoneStatus={status}
          overflowWarning={false}
        />,
      );

      expect(screen.getByText("Microphone access is off.")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Refresh microphones" })).toBeEnabled();
      expect(screen.getByText(/Enable Mistaken in System Settings/)).toBeInTheDocument();
    } finally {
      platformSpy.mockRestore();
    }
  });

  it("shows Windows settings guidance for a denied-access start failure", () => {
    // Verified on real Windows 10 22H2 hardware: turning off "Let desktop
    // apps access your microphone" surfaces through WASAPI/CPAL as an
    // unclassified start failure, never as microphone_permission_denied,
    // so the exact Settings navigation has to ride along with this code.
    const platformSpy = vi.spyOn(window.navigator, "platform", "get").mockReturnValue("Win32");
    const uaSpy = vi
      .spyOn(window.navigator, "userAgent", "get")
      .mockReturnValue("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");
    const status: AudioSourceStatus = {
      status: "error",
      error: {
        code: "capture_start_failed",
        message: "the microphone could not be started",
        recoverable: true,
        source: "microphone",
      },
    };
    try {
      render(
        <MicrophoneControl
          bridgeReady={true}
          controller={makeController()}
          microphoneStatus={status}
          overflowWarning={false}
        />,
      );

      expect(screen.getByText("the microphone could not be started")).toBeInTheDocument();
      expect(
        screen.getByText(
          "Check Settings → Privacy & security → Microphone → Let desktop apps access your microphone.",
        ),
      ).toBeInTheDocument();
      expect(screen.getByRole("combobox")).toBeEnabled();
      expect(screen.getByRole("button", { name: "Refresh microphones" })).toBeEnabled();
    } finally {
      platformSpy.mockRestore();
      uaSpy.mockRestore();
    }
  });

  it("does not show Windows settings guidance for a start failure on macOS", () => {
    const platformSpy = vi.spyOn(window.navigator, "platform", "get").mockReturnValue("MacIntel");
    const status: AudioSourceStatus = {
      status: "error",
      error: {
        code: "capture_start_failed",
        message: "the microphone could not be started",
        recoverable: true,
        source: "microphone",
      },
    };
    try {
      render(
        <MicrophoneControl
          bridgeReady={true}
          controller={makeController()}
          microphoneStatus={status}
          overflowWarning={false}
        />,
      );

      expect(screen.queryByText(/Let desktop apps access your microphone/)).toBeNull();
    } finally {
      platformSpy.mockRestore();
    }
  });

  it("shows a disconnect error as an actionable local message with Refresh enabled", () => {
    const status: AudioSourceStatus = {
      status: "error",
      error: {
        code: "device_disconnected",
        message: "the selected microphone was disconnected",
        recoverable: true,
        source: "microphone",
      },
    };
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController()}
        microphoneStatus={status}
        overflowWarning={false}
      />,
    );

    expect(screen.getByText("the selected microphone was disconnected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh microphones" })).toBeEnabled();
  });

  it("disables every control while starting", () => {
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController()}
        microphoneStatus={{ status: "starting", deviceId: "a" }}
        overflowWarning={false}
      />,
    );

    expect(screen.getByText("Starting…")).toBeInTheDocument();
    expect(screen.getByRole("combobox")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Refresh microphones" })).toBeDisabled();
  });

  it("always shows the non-release development-adapter explanation", () => {
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController()}
        microphoneStatus={{ status: "idle" }}
        overflowWarning={false}
      />,
    );

    expect(
      screen.getByText(/Speech is transcribed locally for development/),
    ).toBeInTheDocument();
    expect(screen.getByText(/not release approved/)).toBeInTheDocument();
  });

  it("shows a reconnecting attempt counter while recovering", async () => {
    const emit = captureErrorHandler();
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController()}
        microphoneStatus={{ status: "starting", deviceId: "a" }}
        overflowWarning={false}
      />,
    );
    await waitFor(() => expect(listen).toHaveBeenCalled());

    emit({
      code: "device_disconnected",
      message: "Reconnecting microphone… attempt 1 of 3",
      recoverable: true,
      source: "microphone",
    });

    expect(
      screen.getByText("Reconnecting microphone… attempt 1 of 3"),
    ).toBeInTheDocument();
  });

  it("shows the degraded-lag banner while transcribing slower than real time", async () => {
    const emit = captureErrorHandler();
    const status: AudioSourceStatus = {
      status: "capturing",
      deviceId: "a",
      activity: "receiving",
    };
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController()}
        microphoneStatus={status}
        overflowWarning={false}
      />,
    );
    await waitFor(() => expect(listen).toHaveBeenCalled());

    emit({
      code: "inference_lagging",
      message: "Microphone is transcribing slower than real time. Some audio is being skipped.",
      recoverable: true,
      source: "microphone",
    });

    expect(
      screen.getByText(
        "Microphone is transcribing slower than real time. Some audio is being skipped.",
      ),
    ).toBeInTheDocument();
  });

  it("shows the exact reason plus Automatic reconnection stopped. when recovery is exhausted", () => {
    const status: AudioSourceStatus = {
      status: "error",
      error: {
        code: "device_disconnected",
        message: "the selected microphone was disconnected Automatic reconnection stopped.",
        recoverable: true,
      },
    };
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController()}
        microphoneStatus={status}
        overflowWarning={false}
      />,
    );

    expect(
      screen.getByText("the selected microphone was disconnected Automatic reconnection stopped."),
    ).toBeInTheDocument();
  });
});
