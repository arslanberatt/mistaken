import { act, createElement, memo, StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import type { TranscriptSegment } from "../../types/transcript";
import { TranscriptWorkspace, type TranscriptWorkspaceProps } from "./TranscriptWorkspace";
import type * as TranscriptRowModule from "./TranscriptRow";

// A memoized wrapper around the real `TranscriptRow` that also records
// every render, so the render-isolation test below can assert on actual
// invocation counts of the row's render function rather than a duration
// heuristic. The wrapper preserves the real component's default shallow
// memo comparison by wrapping it in its own `memo`, so it bails under the
// exact same conditions the shipped row does. `importOriginal` is
// vitest's own module-mocking mechanism (not an author-time-known static
// path), so it is exempt from the static-import rule.
const { transcriptRowRenderSpy } = vi.hoisted(() => ({
  transcriptRowRenderSpy: vi.fn<(segmentId: string) => void>(),
}));
vi.mock("./TranscriptRow", async (importOriginal) => {
  const actual = await importOriginal<typeof TranscriptRowModule>();
  const Spy = memo(function TranscriptRowSpy(props: { segment: TranscriptSegment }) {
    transcriptRowRenderSpy(props.segment.id);
    return createElement(actual.TranscriptRow, props);
  });
  return { ...actual, TranscriptRow: Spy };
});

function renderWorkspace(overrides: Partial<TranscriptWorkspaceProps> = {}) {
  const props: TranscriptWorkspaceProps = {
    segments: [],
    sessionError: null,
    captureStatus: "idle",
    modelStatusLabel: "Local • Runtime unavailable",
    microphoneControl: "No microphone available",
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
          microphoneControl="No microphone available"
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

function stubPlatform(isMac: boolean) {
  vi.stubGlobal("navigator", {
    ...navigator,
    platform: isMac ? "MacIntel" : "Win32",
    userAgent: isMac ? "Macintosh" : "Windows NT 10.0",
  });
}

describe("TranscriptWorkspace keyboard shortcuts", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("Cmd+Enter toggles capture through the exact same callback as the Start Listening button", () => {
    stubPlatform(true);
    const onStartRequested = vi.fn();
    renderWorkspace({ captureStatus: "idle", canStart: true, onStartRequested });

    fireEvent.keyDown(window, { key: "Enter", metaKey: true });
    expect(onStartRequested).toHaveBeenCalledTimes(1);
  });

  it("ignores the wrong-platform modifier", () => {
    stubPlatform(true);
    const onStartRequested = vi.fn();
    renderWorkspace({ captureStatus: "idle", canStart: true, onStartRequested });

    fireEvent.keyDown(window, { key: "Enter", ctrlKey: true });
    expect(onStartRequested).not.toHaveBeenCalled();
  });

  it("ignores the shortcut while Start Listening is disabled, without preventing default", () => {
    stubPlatform(true);
    const onStartRequested = vi.fn();
    renderWorkspace({ captureStatus: "idle", canStart: false, onStartRequested });

    const notPrevented = fireEvent.keyDown(window, { key: "Enter", metaKey: true });
    expect(onStartRequested).not.toHaveBeenCalled();
    expect(notPrevented).toBe(true);
  });

  it("ignores Cmd+Shift+C while Copy All is disabled, without preventing default", () => {
    stubPlatform(true);
    renderWorkspace({ segments: [] });

    const notPrevented = fireEvent.keyDown(window, { key: "C", metaKey: true, shiftKey: true });
    expect(screen.queryByText("Copied")).not.toBeInTheDocument();
    expect(notPrevented).toBe(true);
  });


  it("Ctrl+Enter toggles capture on a Windows platform instead of Cmd+Enter", () => {
    stubPlatform(false);
    const onStopRequested = vi.fn();
    renderWorkspace({ captureStatus: "listening", onStopRequested });

    fireEvent.keyDown(window, { key: "Enter", metaKey: true });
    expect(onStopRequested).not.toHaveBeenCalled();

    fireEvent.keyDown(window, { key: "Enter", ctrlKey: true });
    expect(onStopRequested).toHaveBeenCalledTimes(1);
  });

  it("Cmd+Shift+C copies through the exact same handler as Copy All", async () => {
    stubPlatform(true);
    const writeClipboard = vi.fn().mockResolvedValue(undefined);
    renderWorkspace({ segments: [finalMicSegment], writeClipboard });

    await act(async () => {
      fireEvent.keyDown(window, { key: "C", metaKey: true, shiftKey: true });
    });

    expect(writeClipboard).toHaveBeenCalledWith(finalMicSegment.text);
    expect(await screen.findByText("Copied")).toBeInTheDocument();
  });

  it("Escape cancels the Clear confirmation from the same global listener used for the other shortcuts", () => {
    stubPlatform(true);
    renderWorkspace({ segments: [finalMicSegment] });

    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.queryByRole("button", { name: "Clear transcript" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Clear" })).toHaveFocus();
  });

  it("Space never toggles capture, even with the transcript focused", () => {
    stubPlatform(true);
    const onStartRequested = vi.fn();
    renderWorkspace({ captureStatus: "idle", canStart: true, onStartRequested });

    screen.getByRole("log").focus();
    fireEvent.keyDown(window, { key: " " });
    expect(onStartRequested).not.toHaveBeenCalled();
  });

  it("keeps native Cmd/Ctrl+C, Cmd/Ctrl+A, and arrow-key navigation untouched", () => {
    stubPlatform(true);
    renderWorkspace({ segments: [finalMicSegment] });
    for (const combo of [
      { key: "c", metaKey: true },
      { key: "a", metaKey: true },
      { key: "ArrowDown" },
      { key: "Tab" },
    ]) {
      const notPrevented = fireEvent.keyDown(window, combo);
      expect(notPrevented).toBe(true);
    }
  });

  it("registers exactly one window keydown listener regardless of remounts", () => {
    stubPlatform(true);
    const addSpy = vi.spyOn(window, "addEventListener");
    const removeSpy = vi.spyOn(window, "removeEventListener");
    const { unmount } = renderWorkspace();

    expect(addSpy.mock.calls.filter(([type]) => type === "keydown")).toHaveLength(1);
    unmount();
    expect(removeSpy.mock.calls.filter(([type]) => type === "keydown")).toHaveLength(1);

    addSpy.mockRestore();
    removeSpy.mockRestore();
  });
});

describe("TranscriptWorkspace copy feedback lifecycle", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("Copied clears automatically after 2 seconds via a bounded timeout", async () => {
    const writeClipboard = vi.fn().mockResolvedValue(undefined);
    renderWorkspace({ segments: [finalMicSegment], writeClipboard });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy All" }));
    });
    expect(screen.getByText("Copied")).toBeInTheDocument();

    await act(async () => {
      vi.advanceTimersByTime(1999);
    });
    expect(screen.getByText("Copied")).toBeInTheDocument();

    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    expect(screen.queryByText("Copied")).not.toBeInTheDocument();
  });

  it("a copy failure persists until the next attempt, unaffected by new content arriving", async () => {
    const writeClipboard = vi.fn().mockRejectedValue(new Error("denied"));
    const { rerender, props } = renderWorkspace({ segments: [finalMicSegment], writeClipboard });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy All" }));
    });
    expect(screen.getByText("Could not copy transcript.")).toBeInTheDocument();

    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    expect(screen.getByText("Could not copy transcript.")).toBeInTheDocument();

    rerender(
      <TranscriptWorkspace {...props} segments={[finalMicSegment, finalSystemSegment]} />,
    );
    expect(screen.getByText("Could not copy transcript.")).toBeInTheDocument();
  });

  it("Clear resets copy feedback immediately and cancels the pending timeout", async () => {
    const writeClipboard = vi.fn().mockResolvedValue(undefined);
    renderWorkspace({ segments: [finalMicSegment], writeClipboard });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy All" }));
    });
    expect(screen.getByText("Copied")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    fireEvent.click(screen.getByRole("button", { name: "Clear transcript" }));
    expect(screen.queryByText("Copied")).not.toBeInTheDocument();

    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    expect(screen.queryByText("Copied")).not.toBeInTheDocument();
  });

  it("clears the pending success timeout on unmount", async () => {
    const writeClipboard = vi.fn().mockResolvedValue(undefined);
    const { unmount } = renderWorkspace({ segments: [finalMicSegment], writeClipboard });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy All" }));
    });
    expect(screen.getByText("Copied")).toBeInTheDocument();

    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    unmount();
    vi.advanceTimersByTime(5000);
    expect(consoleError).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });
});

describe("TranscriptWorkspace announcement policy", () => {
  it("marks only interim rows aria-live=off, leaving finalized rows to the ambient polite log", () => {
    renderWorkspace({ segments: [finalMicSegment, interimMicSegment] });
    const log = screen.getByRole("log");
    const items = within(log).getAllByRole("listitem");
    expect(items[0]).not.toHaveAttribute("aria-live");
    expect(items[1]).toHaveAttribute("aria-live", "off");
  });

  it("exposes the transcript log as keyboard-focusable and reachable in the tab order", () => {
    renderWorkspace();
    expect(screen.getByRole("log")).toHaveAttribute("tabindex", "0");
  });

  // Regression: making the log region keyboard-reachable (tabIndex 0)
  // without pairing its `focus:outline-none` reset with a `focus-visible:`
  // replacement left a real, keyboard-only-reachable region with zero
  // visible focus indicator — a real WCAG 2.4.7 defect found and fixed
  // via real Chromium computed-style verification during this spec's
  // implementation (jsdom loads no compiled stylesheet, so this asserts
  // the class-level contract rather than rendered pixels).
  it("never removes the default outline without a focus-visible replacement on the log region", () => {
    renderWorkspace();
    const log = screen.getByRole("log");
    expect(log.className).toContain("focus:outline-none");
    expect(log.className).toContain("focus-visible:outline");
  });
});

describe("TranscriptWorkspace fixed-shell layout at large text scale", () => {
  // Regression: at 720x520 with 150%/200% OS text scale, the header plus
  // source bar plus footer alone can exceed the viewport height. An
  // earlier fix used `h-screen overflow-hidden` on the shell, which kept
  // the four-region look but silently clipped the Clear/Start/Copy
  // controls with no way to reach them (overflow:hidden permits no
  // scroll at all) — found via real Chromium layout measurement. The
  // shell must stay exactly one viewport tall (`h-screen`) with a
  // scrollable fallback (`overflow-y-auto`, never `overflow-hidden`), and
  // every fixed band must refuse to compress (`shrink-0`) so all the
  // squeeze lands on the transcript's own scroll region first.
  it("keeps the shell at a fixed viewport height with a scrollable fallback, never a clipping overflow", () => {
    const { container } = renderWorkspace();
    const shell = container.firstElementChild as HTMLElement;
    expect(shell.className).toContain("h-screen");
    expect(shell.className).toContain("overflow-y-auto");
    expect(shell.className).not.toContain("overflow-hidden");
    expect(shell.className).not.toContain("min-h-screen");
  });

  it("marks every fixed band non-shrinking so only the transcript absorbs overflow", () => {
    renderWorkspace();
    expect(screen.getByRole("banner").className).toContain("shrink-0");
    expect(screen.getByRole("region", { name: /audio sources/i }).className).toContain(
      "shrink-0",
    );
    expect(screen.getByRole("contentinfo").className).toContain("shrink-0");
  });
});

describe("TranscriptWorkspace auto-follow and Jump to latest", () => {
  beforeEach(() => {
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
      queueMicrotask(() => cb(0));
      return 1;
    });
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  function makeLogScrollable(log: HTMLElement) {
    Object.defineProperty(log, "scrollHeight", { value: 2000, configurable: true });
    Object.defineProperty(log, "clientHeight", { value: 400, configurable: true });
    Object.defineProperty(log, "scrollTop", { value: 0, configurable: true, writable: true });
  }

  async function scrollAwayFromBottom(log: HTMLElement) {
    Object.defineProperty(log, "scrollTop", { value: 200, configurable: true, writable: true });
    await act(async () => {
      fireEvent.scroll(log);
      await Promise.resolve();
      await Promise.resolve();
    });
  }

  it("is hidden while following, appears once the user scrolls away, and disappears again after activation", async () => {
    renderWorkspace({ segments: [finalMicSegment] });
    const log = screen.getByRole("log");
    makeLogScrollable(log);

    expect(screen.queryByRole("button", { name: /jump to latest/i })).not.toBeInTheDocument();

    await scrollAwayFromBottom(log);

    const jumpButton = await screen.findByRole("button", { name: /jump to latest/i });
    expect(jumpButton).toBeInTheDocument();

    fireEvent.click(jumpButton);
    expect(screen.queryByRole("button", { name: /jump to latest/i })).not.toBeInTheDocument();
  });

  it("returns focus to the transcript region after activation", async () => {
    renderWorkspace({ segments: [finalMicSegment] });
    const log = screen.getByRole("log");
    makeLogScrollable(log);

    await scrollAwayFromBottom(log);
    const jumpButton = await screen.findByRole("button", { name: /jump to latest/i });
    fireEvent.click(jumpButton);

    expect(screen.getByRole("log")).toHaveFocus();
  });
});

describe("TranscriptWorkspace focus integrity", () => {
  it("keeps focus on a control across an unrelated transcript update", () => {
    const { rerender, props } = renderWorkspace({ segments: [finalMicSegment] });
    const copyButton = screen.getByRole("button", { name: "Copy All" });
    copyButton.focus();
    expect(copyButton).toHaveFocus();

    rerender(
      <TranscriptWorkspace {...props} segments={[finalMicSegment, finalSystemSegment]} />,
    );

    expect(screen.getByRole("button", { name: "Copy All" })).toHaveFocus();
  });
});

describe("TranscriptWorkspace render isolation at scale", () => {
  it("an interim update rerenders only its own row, not the other ~1000 rows", () => {
    const finals: TranscriptSegment[] = Array.from({ length: 999 }, (_, i) => ({
      id: `final-${i}`,
      source: i % 2 === 0 ? "microphone" : "system",
      text: `Segment number ${i}.`,
      startedAtMs: i * 1000,
      endedAtMs: i * 1000 + 500,
      isFinal: true,
    }));
    const interim: TranscriptSegment = {
      id: "interim-live",
      source: "microphone",
      text: "Um, so anyway",
      startedAtMs: 999_000,
      isFinal: false,
    };

    const { rerender, props } = renderWorkspace({ segments: [...finals, interim] });
    transcriptRowRenderSpy.mockClear();

    const revisedInterim: TranscriptSegment = { ...interim, text: "Um, so anyway I think" };
    rerender(
      <TranscriptWorkspace {...props} segments={[...finals, revisedInterim]} />,
    );

    expect(transcriptRowRenderSpy).toHaveBeenCalledTimes(1);
    expect(transcriptRowRenderSpy).toHaveBeenCalledWith("interim-live");
  });
});
