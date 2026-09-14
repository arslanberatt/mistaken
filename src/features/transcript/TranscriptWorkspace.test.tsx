import { act, StrictMode } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import type { TranscriptSegment } from "../../types/transcript";
import { TranscriptWorkspace, type TranscriptWorkspaceProps } from "./TranscriptWorkspace";

function renderWorkspace(overrides: Partial<TranscriptWorkspaceProps> = {}) {
  const props: TranscriptWorkspaceProps = {
    segments: [],
    sessionError: null,
    captureStatus: "idle",
    modelStatusLabel: "Local • Runtime unavailable",
    microphoneLabel: "No microphone available",
    systemAudioLabel: "Not connected",
    elapsedMs: 0,
    canStart: false,
    onStartRequested: null,
    onStopRequested: null,
    onClearRequested: vi.fn(),
    writeClipboard: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
  return { ...render(<TranscriptWorkspace {...props} />), props };
}

const finalMicSegment: TranscriptSegment = {
  id: "mic-1",
  source: "microphone",
  text: "I actually have went there yesterday.",
  startedAtMs: 0,
  endedAtMs: 1200,
  isFinal: true,
};

const finalSystemSegment: TranscriptSegment = {
  id: "sys-1",
  source: "system",
  text: "Why did you go there?",
  startedAtMs: 500,
  endedAtMs: 1800,
  isFinal: true,
};

const interimMicSegment: TranscriptSegment = {
  id: "mic-2",
  source: "microphone",
  text: "Um, because my friend...",
  startedAtMs: 2000,
  isFinal: false,
};

describe("TranscriptWorkspace committed empty state", () => {
  it("renders the four-region layout with practical empty copy and disabled actions", () => {
    renderWorkspace();

    expect(
      screen.getByRole("heading", { name: "Mistaken" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Local • Runtime unavailable")).toBeInTheDocument();
    expect(screen.getByText(/No microphone available/)).toBeInTheDocument();
    expect(screen.getByText(/Not connected/)).toBeInTheDocument();
    expect(screen.getByText("Ready to transcribe locally.")).toBeInTheDocument();
    expect(
      screen.getByText("Choose your microphone, then start listening."),
    ).toBeInTheDocument();
    expect(
      screen.getByText('System audio will appear with a "-" prefix.'),
    ).toBeInTheDocument();
    expect(
      screen.getByText("No audio or transcript is uploaded."),
    ).toBeInTheDocument();
    expect(screen.getByText("00:00:00")).toBeInTheDocument();

    expect(screen.getByRole("button", { name: "Start Listening" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Clear" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Copy All" })).toBeDisabled();
  });
});

describe("TranscriptWorkspace populated/interim rendering", () => {
  it("renders microphone text without a prefix, system text with '- ', and marks interim rows", () => {
    renderWorkspace({
      segments: [finalMicSegment, finalSystemSegment, interimMicSegment],
    });

    expect(screen.getByText(finalMicSegment.text)).toBeInTheDocument();
    expect(screen.getByText(`- ${finalSystemSegment.text}`)).toBeInTheDocument();
    expect(screen.getByText(interimMicSegment.text)).toBeInTheDocument();
    expect(screen.getAllByText("Interim")).toHaveLength(1);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("keeps first-seen row order in the rendered list", () => {
    renderWorkspace({
      segments: [finalMicSegment, finalSystemSegment, interimMicSegment],
    });

    const log = screen.getByRole("log");
    const items = within(log).getAllByRole("listitem");
    expect(items).toHaveLength(3);
    expect(items[0]).toHaveTextContent(finalMicSegment.text);
    expect(items[1]).toHaveTextContent(`- ${finalSystemSegment.text}`);
    expect(items[2]).toHaveTextContent(interimMicSegment.text);
  });
});

describe("TranscriptWorkspace domain invariant rejection status", () => {
  it("shows a concise rejected-update status without hiding existing content", () => {
    renderWorkspace({
      segments: [finalMicSegment],
      sessionError: { code: "segment_identity_conflict", segmentId: finalMicSegment.id },
    });

    expect(screen.getByText(finalMicSegment.text)).toBeInTheDocument();
    expect(screen.getByText(new RegExp(finalMicSegment.id))).toBeInTheDocument();
  });
});

describe("TranscriptWorkspace capture status presentation", () => {
  it("idle: stays disabled when canStart is false", () => {
    renderWorkspace({ captureStatus: "idle", canStart: false, onStartRequested: null });
    expect(screen.getByRole("button", { name: "Start Listening" })).toBeDisabled();
  });

  it("idle: enables Start Listening only when canStart is true and a callback exists, and calls it", () => {
    const onStartRequested = vi.fn();
    renderWorkspace({ captureStatus: "idle", canStart: true, onStartRequested });

    const button = screen.getByRole("button", { name: "Start Listening" });
    expect(button).toBeEnabled();
    fireEvent.click(button);
    expect(onStartRequested).toHaveBeenCalledTimes(1);
  });

  it("idle: stays disabled when canStart is true but no callback is supplied", () => {
    renderWorkspace({ captureStatus: "idle", canStart: true, onStartRequested: null });
    expect(screen.getByRole("button", { name: "Start Listening" })).toBeDisabled();
  });

  it("starting: shows disabled 'Starting…'", () => {
    renderWorkspace({ captureStatus: "starting" });
    expect(screen.getByRole("button", { name: "Starting…" })).toBeDisabled();
  });

  it("listening: enables Stop only when a stop callback exists, and calls it", () => {
    const onStopRequested = vi.fn();
    renderWorkspace({ captureStatus: "listening", onStopRequested });

    const button = screen.getByRole("button", { name: "Stop" });
    expect(button).toBeEnabled();
    fireEvent.click(button);
    expect(onStopRequested).toHaveBeenCalledTimes(1);
  });

  it("listening: stays disabled without a stop callback", () => {
    renderWorkspace({ captureStatus: "listening", onStopRequested: null });
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
  });

  it("stopping: shows disabled 'Stopping…'", () => {
    renderWorkspace({ captureStatus: "stopping" });
    expect(screen.getByRole("button", { name: "Stopping…" })).toBeDisabled();
  });

  it("error: shows a disabled explicit error status with no retry", () => {
    renderWorkspace({ captureStatus: "error" });
    expect(screen.getByRole("button", { name: "Capture error" })).toBeDisabled();
  });
});

describe("TranscriptWorkspace clear behavior", () => {
  it("disables Clear when the transcript is empty", () => {
    renderWorkspace({ segments: [] });
    expect(screen.getByRole("button", { name: "Clear" })).toBeDisabled();
  });

  it("clears interim-only content immediately without a confirmation step", () => {
    const onClearRequested = vi.fn();
    renderWorkspace({ segments: [interimMicSegment], onClearRequested });

    fireEvent.click(screen.getByRole("button", { name: "Clear" }));

    expect(onClearRequested).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Clear transcript" })).not.toBeInTheDocument();
  });

  it("requires confirmation before clearing any finalized content", () => {
    const onClearRequested = vi.fn();
    renderWorkspace({ segments: [finalMicSegment], onClearRequested });

    fireEvent.click(screen.getByRole("button", { name: "Clear" }));

    expect(onClearRequested).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Clear transcript" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
  });

  it("Cancel preserves content and restores focus to the Clear button", () => {
    renderWorkspace({ segments: [finalMicSegment] });

    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(screen.queryByRole("button", { name: "Clear transcript" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Clear" })).toHaveFocus();
  });

  it("Escape cancels the inline confirmation and preserves content", () => {
    renderWorkspace({ segments: [finalMicSegment] });

    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    fireEvent.keyDown(screen.getByRole("button", { name: "Clear transcript" }), {
      key: "Escape",
    });

    expect(screen.queryByRole("button", { name: "Clear transcript" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Clear" })).toHaveFocus();
  });

  it("Confirm clears every in-memory segment and moves focus to a stable workspace target", () => {
    const onClearRequested = vi.fn();
    renderWorkspace({ segments: [finalMicSegment], onClearRequested });

    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    fireEvent.click(screen.getByRole("button", { name: "Clear transcript" }));

    expect(onClearRequested).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("log")).toHaveFocus();
  });
});

describe("TranscriptWorkspace copy behavior", () => {
  it("disables Copy All when no finalized content exists, even with an interim row", () => {
    renderWorkspace({ segments: [interimMicSegment] });
    expect(screen.getByRole("button", { name: "Copy All" })).toBeDisabled();
  });

  it("writes the exact serialized finalized transcript once per activation and shows success feedback", async () => {
    const writeClipboard = vi.fn().mockResolvedValue(undefined);
    renderWorkspace({
      segments: [finalMicSegment, finalSystemSegment],
      writeClipboard,
    });

    const button = screen.getByRole("button", { name: "Copy All" });
    await act(async () => {
      fireEvent.click(button);
    });

    expect(writeClipboard).toHaveBeenCalledTimes(1);
    expect(writeClipboard).toHaveBeenCalledWith(
      `${finalMicSegment.text}\n\n- ${finalSystemSegment.text}`,
    );
    expect(await screen.findByText("Copied")).toBeInTheDocument();
  });

  it("shows actionable failure feedback and leaves transcript state unchanged on rejection", async () => {
    const writeClipboard = vi.fn().mockRejectedValue(new Error("denied"));
    const onClearRequested = vi.fn();
    renderWorkspace({
      segments: [finalMicSegment],
      writeClipboard,
      onClearRequested,
    });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy All" }));
    });

    expect(await screen.findByText("Could not copy transcript.")).toBeInTheDocument();
    expect(screen.getByText(finalMicSegment.text)).toBeInTheDocument();
    expect(onClearRequested).not.toHaveBeenCalled();
  });

  it("ignores repeated activation while a write is pending", async () => {
    let resolveWrite: (() => void) | undefined;
    const writeClipboard = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveWrite = resolve;
        }),
    );
    renderWorkspace({ segments: [finalMicSegment], writeClipboard });

    const button = screen.getByRole("button", { name: "Copy All" });
    fireEvent.click(button);
    expect(button).toBeDisabled();
    fireEvent.click(button);

    expect(writeClipboard).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveWrite?.();
      await Promise.resolve();
    });
  });

  it("does not claim success once finalized content changes while a write is pending", async () => {
    let resolveWrite: (() => void) | undefined;
    const writeClipboard = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveWrite = resolve;
        }),
    );
    const { rerender, props } = renderWorkspace({
      segments: [finalMicSegment],
      writeClipboard,
    });

    fireEvent.click(screen.getByRole("button", { name: "Copy All" }));

    rerender(
      <TranscriptWorkspace
        {...props}
        segments={[finalMicSegment, finalSystemSegment]}
      />,
    );

    await act(async () => {
      resolveWrite?.();
      await Promise.resolve();
    });

    expect(screen.queryByText("Copied")).not.toBeInTheDocument();
  });

  it("resolves a pending copy to success under React.StrictMode (matches real main.tsx mounting)", async () => {
    const writeClipboard = vi.fn().mockResolvedValue(undefined);
    const onClearRequested = vi.fn();
    render(
      <StrictMode>
        <TranscriptWorkspace
          segments={[finalMicSegment]}
          sessionError={null}
          captureStatus="idle"
          modelStatusLabel="Local • Runtime unavailable"
          microphoneLabel="No microphone available"
          systemAudioLabel="Not connected"
          elapsedMs={0}
          canStart={false}
          onStartRequested={null}
          onStopRequested={null}
          onClearRequested={onClearRequested}
          writeClipboard={writeClipboard}
        />
      </StrictMode>,
    );

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy All" }));
    });

    expect(await screen.findByText("Copied")).toBeInTheDocument();
  });
});

describe("TranscriptWorkspace accessibility", () => {
  it("exposes semantic header/log/footer regions with accessible names", () => {
    renderWorkspace();

    expect(screen.getByRole("banner")).toBeInTheDocument();
    expect(screen.getByRole("log", { name: /transcript/i })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: /audio sources/i })).toBeInTheDocument();
    expect(screen.getByRole("contentinfo")).toBeInTheDocument();
  });

  it("exposes a reason for the disabled Start Listening control without relying on tooltip-only content", () => {
    renderWorkspace();
    const startButton = screen.getByRole("button", { name: "Start Listening" });
    expect(startButton).toHaveAccessibleDescription(/not available/i);
  });

  it("exposes a reason for the disabled Clear and Copy All controls", () => {
    renderWorkspace();
    expect(screen.getByRole("button", { name: "Clear" })).toHaveAccessibleDescription(
      /no transcript/i,
    );
    expect(
      screen.getByRole("button", { name: "Copy All" }),
    ).toHaveAccessibleDescription(/no finalized transcript/i);
  });

  it("gives every button a visible accessible name (no icon-only critical actions)", () => {
    renderWorkspace({ segments: [finalMicSegment] });
    for (const button of screen.getAllByRole("button")) {
      expect(button.textContent?.trim().length).toBeGreaterThan(0);
    }
  });
});
