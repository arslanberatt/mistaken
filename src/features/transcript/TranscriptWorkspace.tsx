/**
 * Single-window transcript workspace: top/source/transcript/action regions
 * from ui-context.md. Renders presentation state and calls explicit
 * callbacks; it owns no native lifecycle, IPC event name, or capture logic.
 */
import {
  useCallback,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { MonitorSpeaker } from "lucide-react";
import type { CaptureStatus } from "../../types/runtime";
import type { TranscriptSegment } from "../../types/transcript";
import {
  formatTranscriptSegment,
  serializeFinalTranscript,
  type TranscriptSessionError,
} from "./transcript-domain";
import type { ClipboardWriter } from "./transcript-clipboard";

export interface TranscriptWorkspaceProps {
  readonly segments: readonly TranscriptSegment[];
  readonly sessionError: TranscriptSessionError | null;
  readonly captureStatus: CaptureStatus;
  readonly modelStatusLabel: string;
  readonly microphoneControl: ReactNode;
  readonly systemAudioControl?: ReactNode;
  readonly systemAudioLabel?: string;
  readonly elapsedMs: number;
  readonly canStart: boolean;
  readonly onStartRequested: (() => void) | null;
  readonly onStopRequested: (() => void) | null;
  readonly onClearRequested: () => void;
  readonly writeClipboard: ClipboardWriter;
}

type CopyFeedbackState = "idle" | "pending" | "success" | "failure";
type FocusTarget = "clearButton" | "confirmButton" | "workspace";

interface CaptureActionPresentation {
  readonly label: string;
  readonly disabled: boolean;
  readonly onClick: (() => void) | undefined;
}

function formatElapsedTime(elapsedMs: number): string {
  const totalSeconds = Math.max(0, Math.floor(elapsedMs / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const pad = (value: number) => value.toString().padStart(2, "0");
  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
}

function getCaptureActionPresentation(
  captureStatus: CaptureStatus,
  canStart: boolean,
  onStartRequested: (() => void) | null,
  onStopRequested: (() => void) | null,
): CaptureActionPresentation {
  switch (captureStatus) {
    case "idle": {
      const enabled = canStart && onStartRequested !== null;
      return {
        label: "Start Listening",
        disabled: !enabled,
        onClick: enabled ? onStartRequested : undefined,
      };
    }
    case "starting":
      return { label: "Starting…", disabled: true, onClick: undefined };
    case "listening": {
      const enabled = onStopRequested !== null;
      return {
        label: "Stop",
        disabled: !enabled,
        onClick: enabled ? onStopRequested : undefined,
      };
    }
    case "stopping":
      return { label: "Stopping…", disabled: true, onClick: undefined };
    case "error":
      return { label: "Capture error", disabled: true, onClick: undefined };
  }
}

export function TranscriptWorkspace({
  segments,
  sessionError,
  captureStatus,
  modelStatusLabel,
  microphoneControl,
  systemAudioControl,
  systemAudioLabel,
  elapsedMs,
  canStart,
  onStartRequested,
  onStopRequested,
  onClearRequested,
  writeClipboard,
}: TranscriptWorkspaceProps) {
  const clearButtonRef = useRef<HTMLButtonElement>(null);
  const confirmButtonRef = useRef<HTMLButtonElement>(null);
  const transcriptRegionRef = useRef<HTMLElement>(null);
  const focusTargetRef = useRef<FocusTarget | null>(null);
  const isMountedRef = useRef(true);
  const requestIdRef = useRef(0);
  const currentFinalizedTextRef = useRef("");
  const previousFinalizedTextRef = useRef("");

  const [clearConfirming, setClearConfirming] = useState(false);
  const [copyState, setCopyState] = useState<CopyFeedbackState>("idle");

  const finalizedText = useMemo(
    () => serializeFinalTranscript(segments),
    [segments],
  );
  useLayoutEffect(() => {
    currentFinalizedTextRef.current = finalizedText;
  }, [finalizedText]);

  const hasAnySegments = segments.length > 0;
  const hasFinalContent = finalizedText.length > 0;

  // Feedback resets whenever finalized content changes; a write already in
  // flight keeps its own pending state until it settles.
  useLayoutEffect(() => {
    if (previousFinalizedTextRef.current !== finalizedText) {
      previousFinalizedTextRef.current = finalizedText;
      setCopyState((prev) => (prev === "pending" ? prev : "idle"));
    }
  }, [finalizedText]);

  // Runs after every commit; only acts when a focus move was requested by
  // the event handler that triggered the render (Cancel/Confirm/open).
  useLayoutEffect(() => {
    if (focusTargetRef.current === "clearButton") {
      clearButtonRef.current?.focus();
    } else if (focusTargetRef.current === "confirmButton") {
      confirmButtonRef.current?.focus();
    } else if (focusTargetRef.current === "workspace") {
      transcriptRegionRef.current?.focus();
    }
    focusTargetRef.current = null;
  });

  useLayoutEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const handleClearActivate = useCallback(() => {
    if (!hasAnySegments) {
      return;
    }
    if (!hasFinalContent) {
      onClearRequested();
      return;
    }
    focusTargetRef.current = "confirmButton";
    setClearConfirming(true);
  }, [hasAnySegments, hasFinalContent, onClearRequested]);

  const handleClearCancel = useCallback(() => {
    focusTargetRef.current = "clearButton";
    setClearConfirming(false);
  }, []);

  const handleClearConfirm = useCallback(() => {
    focusTargetRef.current = "workspace";
    setClearConfirming(false);
    onClearRequested();
  }, [onClearRequested]);

  const handleConfirmationKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Escape") {
        event.preventDefault();
        handleClearCancel();
      }
    },
    [handleClearCancel],
  );

  const handleCopy = useCallback(() => {
    if (copyState === "pending" || !hasFinalContent) {
      return;
    }
    const snapshot = finalizedText;
    requestIdRef.current += 1;
    const requestId = requestIdRef.current;
    setCopyState("pending");

    writeClipboard(snapshot)
      .then(() => {
        if (!isMountedRef.current || requestIdRef.current !== requestId) {
          return;
        }
        // Only claim success for the exact snapshot written; a content
        // change while the write was pending must not be reported as copied.
        setCopyState(
          currentFinalizedTextRef.current === snapshot ? "success" : "idle",
        );
      })
      .catch(() => {
        if (!isMountedRef.current || requestIdRef.current !== requestId) {
          return;
        }
        setCopyState(
          currentFinalizedTextRef.current === snapshot ? "failure" : "idle",
        );
      });
  }, [copyState, hasFinalContent, finalizedText, writeClipboard]);

  const captureAction = getCaptureActionPresentation(
    captureStatus,
    canStart,
    onStartRequested,
    onStopRequested,
  );
  const startDisabledForIdle =
    captureStatus === "idle" && captureAction.disabled;

  return (
    <div className="flex min-h-screen flex-col bg-[var(--bg-base)] text-[var(--text-primary)]">
      <header className="flex items-center justify-between gap-4 border-b border-[var(--border-default)] px-6 py-4">
        <h1 className="text-lg font-semibold tracking-tight">Mistaken</h1>
        <div className="flex items-center gap-2 text-sm text-[var(--text-secondary)]">
          <span
            aria-hidden="true"
            className={`h-2 w-2 rounded-full ${
              captureStatus === "listening"
                ? "bg-[var(--state-success)]"
                : captureStatus === "error"
                  ? "bg-[var(--state-error)]"
                  : "bg-[var(--text-muted)]"
            }`}
          />
          <span>{modelStatusLabel}</span>
        </div>
      </header>

      <section
        aria-label="Audio sources"
        className="flex flex-wrap items-center gap-4 border-b border-[var(--border-default)] px-6 py-3 text-sm text-[var(--text-secondary)]"
      >
        <div className="flex items-center gap-2">{microphoneControl}</div>
        {systemAudioControl ?? (
          <span className="flex items-center gap-2">
            <MonitorSpeaker aria-hidden="true" className="h-4 w-4" />
            System Audio: {systemAudioLabel ?? "Not connected"}
          </span>
        )}
      </section>

      <section
        ref={transcriptRegionRef}
        role="log"
        aria-label="Transcript"
        tabIndex={-1}
        className="flex-1 overflow-y-auto px-6 py-6 focus:outline-none"
      >
        {sessionError && (
          <p
            role="status"
            className="mb-4 max-w-[75ch] text-sm text-[var(--state-warning)]"
          >
            A transcript update for segment "{sessionError.segmentId}" was
            rejected to protect existing content.
          </p>
        )}

        {segments.length === 0 ? (
          <div className="flex max-w-[60ch] flex-col gap-2 text-[var(--text-secondary)]">
            <p className="text-base font-medium text-[var(--text-primary)]">
              Ready to transcribe locally.
            </p>
            <p>Choose your microphone, then start listening.</p>
            <p>System audio will appear with a "-" prefix.</p>
            <p className="text-sm text-[var(--text-muted)]">
              No audio or transcript is uploaded.
            </p>
          </div>
        ) : (
          <ul className="flex flex-col gap-6">
            {segments.map((segment) => (
              <li key={segment.id} className="flex flex-col gap-1">
                {!segment.isFinal && (
                  <span className="text-xs font-semibold uppercase tracking-wide text-[var(--text-interim)]">
                    Interim
                  </span>
                )}
                <p
                  className={`max-w-[75ch] whitespace-pre-wrap font-[var(--font-transcript)] text-base leading-[1.65] ${
                    segment.isFinal
                      ? "text-[var(--text-primary)]"
                      : "text-[var(--text-interim)]"
                  }`}
                >
                  {formatTranscriptSegment(segment)}
                </p>
              </li>
            ))}
          </ul>
        )}
      </section>

      <footer className="flex flex-wrap items-center justify-between gap-4 border-t border-[var(--border-default)] px-6 py-4">
        <span className="font-[var(--font-mono)] text-sm text-[var(--text-muted)]">
          {formatElapsedTime(elapsedMs)}
        </span>

        <div className="flex flex-wrap items-center gap-3">
          {clearConfirming ? (
            <div
              className="flex items-center gap-2"
              onKeyDown={handleConfirmationKeyDown}
            >
              <span className="text-sm text-[var(--text-secondary)]">
                Clear all transcript content?
              </span>
              <button
                ref={confirmButtonRef}
                type="button"
                onClick={handleClearConfirm}
                className="rounded-lg border border-[var(--state-error)] px-3 py-1.5 text-sm font-medium text-[var(--state-error)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-primary)]"
              >
                Clear transcript
              </button>
              <button
                type="button"
                onClick={handleClearCancel}
                className="rounded-lg border border-[var(--border-strong)] px-3 py-1.5 text-sm font-medium text-[var(--text-secondary)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-primary)]"
              >
                Cancel
              </button>
            </div>
          ) : (
            <>
              <button
                ref={clearButtonRef}
                type="button"
                onClick={handleClearActivate}
                disabled={!hasAnySegments}
                aria-describedby={
                  hasAnySegments ? undefined : "clear-disabled-reason"
                }
                className="rounded-lg border border-[var(--border-default)] px-3 py-1.5 text-sm font-medium text-[var(--text-secondary)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-primary)] disabled:cursor-not-allowed disabled:opacity-50"
              >
                Clear
              </button>
              {!hasAnySegments && (
                <span id="clear-disabled-reason" className="sr-only">
                  No transcript to clear yet.
                </span>
              )}
            </>
          )}

          <button
            type="button"
            onClick={captureAction.onClick}
            disabled={captureAction.disabled}
            aria-describedby={
              startDisabledForIdle ? "start-disabled-reason" : undefined
            }
            className="rounded-lg bg-[var(--accent-primary)] px-4 py-1.5 text-sm font-semibold text-[var(--bg-base)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-hover)] disabled:cursor-not-allowed disabled:bg-[var(--border-strong)] disabled:text-[var(--text-muted)]"
          >
            {captureAction.label}
          </button>
          {startDisabledForIdle && (
            <span id="start-disabled-reason" className="sr-only">
              Local runtime is not available yet.
            </span>
          )}

          <button
            type="button"
            onClick={handleCopy}
            disabled={!hasFinalContent || copyState === "pending"}
            aria-describedby={
              hasFinalContent ? undefined : "copy-disabled-reason"
            }
            className="rounded-lg border border-[var(--border-default)] px-3 py-1.5 text-sm font-medium text-[var(--text-secondary)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-primary)] disabled:cursor-not-allowed disabled:opacity-50"
          >
            {copyState === "pending" ? "Copying…" : "Copy All"}
          </button>
          {!hasFinalContent && (
            <span id="copy-disabled-reason" className="sr-only">
              No finalized transcript to copy yet.
            </span>
          )}

          <div role="status" aria-live="polite" className="min-w-[6rem] text-sm">
            {copyState === "success" && (
              <span className="text-[var(--state-success)]">Copied</span>
            )}
            {copyState === "failure" && (
              <span className="text-[var(--state-error)]">
                Could not copy transcript.
              </span>
            )}
          </div>
        </div>
      </footer>
    </div>
  );
}
