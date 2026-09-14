import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AudioSourceStatus } from "../../lib/tauri";
import { MicrophoneControl } from "./MicrophoneControl";
import type { UseMicrophoneControllerResult } from "./microphone-controller";

function makeController(
  overrides: Partial<UseMicrophoneControllerResult> = {},
): UseMicrophoneControllerResult {
  return {
    devices: [{ id: "a", label: "Built-in Microphone", isDefault: true }],
    selectedDeviceId: "a",
    listState: "ready",
    commandPending: null,
    error: null,
    refresh: vi.fn(),
    selectDevice: vi.fn(),
    startTest: vi.fn(),
    stopTest: vi.fn(),
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
    expect(screen.getByRole("button", { name: "Test microphone" })).toBeDisabled();
  });

  it("shows the no-devices state with Refresh enabled and Test disabled", () => {
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
    expect(screen.getByRole("button", { name: "Test microphone" })).toBeDisabled();
  });

  it("shows the ready state with a real device label and an enabled Test action", () => {
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController()}
        microphoneStatus={{ status: "idle" }}
        overflowWarning={false}
      />,
    );

    expect(screen.getByText("Ready to test locally.")).toBeInTheDocument();
    expect(screen.getByText("Built-in Microphone (default)")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Test microphone" })).toBeEnabled();
  });

  it("shows waiting activity with Stop test enabled and no receiving claim", () => {
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
    expect(screen.getByRole("button", { name: "Stop test" })).toBeEnabled();
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

    expect(screen.getByText("PCM signal received")).toBeInTheDocument();
  });

  it("shows the overflow warning instead of the receiving message while capturing", () => {
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
      screen.getByText("Microphone input is delayed; some audio was dropped."),
    ).toBeInTheDocument();
  });

  it("shows exact permission-denied guidance and disables Test until Refresh", () => {
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

      expect(screen.getByRole("button", { name: "Test microphone" })).toBeDisabled();
      expect(screen.getByRole("button", { name: "Refresh microphones" })).toBeEnabled();
      expect(screen.getByText(/Enable Mistaken in System Settings/)).toBeInTheDocument();
    } finally {
      platformSpy.mockRestore();
    }
  });

  it("shows Windows settings guidance for a denied-access start failure and allows retry", () => {
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
      // The user can act on the guidance and retry without a relaunch.
      expect(screen.getByRole("button", { name: "Test microphone" })).toBeEnabled();
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

    expect(
      screen.getByText("the selected microphone was disconnected"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh microphones" })).toBeEnabled();
  });

  it("disables every control while starting", () => {
    render(
      <MicrophoneControl
        bridgeReady={true}
        controller={makeController({ commandPending: "start" })}
        microphoneStatus={{ status: "starting", deviceId: "a" }}
        overflowWarning={false}
      />,
    );

    expect(screen.getByRole("button", { name: "Starting test…" })).toBeDisabled();
    expect(screen.getByRole("combobox")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Refresh microphones" })).toBeDisabled();
  });
});
