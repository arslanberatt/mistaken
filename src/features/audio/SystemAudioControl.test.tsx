import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { SystemAudioControl, type SystemAudioControlProps } from "./SystemAudioControl";
import type { AudioSourceStatus } from "../../lib/tauri";

function renderControl(overrides: Partial<SystemAudioControlProps> = {}) {
  const props: SystemAudioControlProps = {
    enabled: false,
    onChange: vi.fn(),
    disabled: false,
    systemAudioStatus: { status: "idle" },
    ...overrides,
  };
  return { ...render(<SystemAudioControl {...props} />), props };
}

describe("SystemAudioControl", () => {
  it("renders unchecked and shows Off when idle and disabled", () => {
    renderControl({ enabled: false });
    const checkbox = screen.getByRole("checkbox", { name: /System Audio/i });
    expect(checkbox).not.toBeChecked();
    expect(screen.getByText("Off")).toBeInTheDocument();
  });

  it("renders checked and shows Ready when idle and enabled", () => {
    renderControl({ enabled: true });
    const checkbox = screen.getByRole("checkbox", { name: /System Audio/i });
    expect(checkbox).toBeChecked();
    expect(screen.getByText("Ready")).toBeInTheDocument();
  });

  it("calls onChange when clicked", () => {
    const { props } = renderControl({ enabled: false });
    const checkbox = screen.getByRole("checkbox", { name: /System Audio/i });
    fireEvent.click(checkbox);
    expect(props.onChange).toHaveBeenCalledWith(true);
  });

  it("is disabled when disabled prop is true", () => {
    renderControl({ disabled: true });
    const checkbox = screen.getByRole("checkbox", { name: /System Audio/i });
    expect(checkbox).toBeDisabled();
  });

  it("shows Capturing when capturing and receiving", () => {
    const status: AudioSourceStatus = {
      status: "capturing",
      activity: "receiving",
    };
    renderControl({ enabled: true, systemAudioStatus: status });
    expect(screen.getByText("Capturing")).toBeInTheDocument();
  });

  it("shows Waiting for audio when capturing and waiting", () => {
    const status: AudioSourceStatus = {
      status: "capturing",
      activity: "waiting",
    };
    renderControl({ enabled: true, systemAudioStatus: status });
    expect(screen.getByText("Waiting for audio…")).toBeInTheDocument();
  });

  it("shows platform guidance when permission is denied", () => {
    const status: AudioSourceStatus = {
      status: "error",
      error: {
        code: "system_audio_permission_denied",
        message: "permission denied",
        recoverable: true,
      },
    };
    renderControl({ enabled: true, systemAudioStatus: status });
    expect(screen.getByText("Error")).toBeInTheDocument();
    expect(
      screen.getByText(/Screen Recording/),
    ).toBeInTheDocument();
  });

  it("shows relaunch guidance when restart is required", () => {
    const status: AudioSourceStatus = {
      status: "error",
      error: {
        code: "system_audio_permission_denied",
        message: "permission requires relaunch",
        recoverable: true,
      },
    };
    renderControl({ enabled: true, systemAudioStatus: status });
    expect(
      screen.getByText(/Then relaunch Mistaken/),
    ).toBeInTheDocument();
  });

  it("shows privacy notice", () => {
    renderControl();
    expect(
      screen.getByText(/Mistaken captures system audio only, records nothing to disk, and uploads nothing./),
    ).toBeInTheDocument();
  });
});
