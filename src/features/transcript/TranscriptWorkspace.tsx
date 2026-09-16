/**
 * Single-window transcript workspace: top/source/transcript/action regions
 * from ui-context.md. Renders presentation state and calls explicit
 * callbacks; it owns no native lifecycle, IPC event name, or capture logic.
 */
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { ArrowDown, MonitorSpeaker } from "lucide-react";
import type { CaptureStatus } from "../../types/runtime";
import type { TranscriptSegment } from "../../types/transcript";
import {
  serializeFinalTranscript,
  type TranscriptSessionError,
} from "./transcript-domain";
import type { ClipboardWriter } from "./transcript-clipboard";
import { TranscriptRow } from "./TranscriptRow";
import { useAutoFollow } from "./use-auto-follow";
import { useWorkspaceShortcuts } from "./use-workspace-shortcuts";

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

const COPY_FEEDBACK_DURATION_MS = 2000;

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
  const copyFeedbackTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [clearConfirming, setClearConfirming] = useState(false);
  const [copyState, setCopyState] = useState<CopyFeedbackState>("idle");

  const { isFollowing, jumpToLatest } = useAutoFollow(transcriptRegionRef, segments);

  const finalizedText = useMemo(
    () => serializeFinalTranscript(segments),
    [segments],
  );
  useLayoutEffect(() => {
    currentFinalizedTextRef.current = finalizedText;
  }, [finalizedText]);

  const hasAnySegments = segments.length > 0;
  const hasFinalContent = finalizedText.length > 0;

  const clearCopyFeedbackTimeout = useCallback(() => {
    if (copyFeedbackTimeoutRef.current !== null) {
      clearTimeout(copyFeedbackTimeoutRef.current);
      copyFeedbackTimeoutRef.current = null;
    }
  }, []);

  useEffect(() => clearCopyFeedbackTimeout, [clearCopyFeedbackTimeout]);

  // Runs after every commit; only acts when a focus move was requested by
  // the event handler that triggered the render (Cancel/Confirm/open/jump).
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
    clearCopyFeedbackTimeout();
    setCopyState("idle");
    onClearRequested();
  }, [clearCopyFeedbackTimeout, onClearRequested]);

  const handleCopy = useCallback(() => {
    if (copyState === "pending" || !hasFinalContent) {
      return;
    }
    clearCopyFeedbackTimeout();
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
        if (currentFinalizedTextRef.current !== snapshot) {
          setCopyState("idle");
          return;
        }
        setCopyState("success");
        copyFeedbackTimeoutRef.current = setTimeout(() => {
          copyFeedbackTimeoutRef.current = null;
          setCopyState((prev) => (prev === "success" ? "idle" : prev));
        }, COPY_FEEDBACK_DURATION_MS);
      })
      .catch(() => {
        if (!isMountedRef.current || requestIdRef.current !== requestId) {
          return;
        }
        setCopyState(
          currentFinalizedTextRef.current === snapshot ? "failure" : "idle",
        );
      });
  }, [copyState, hasFinalContent, finalizedText, writeClipboard, clearCopyFeedbackTimeout]);

  const handleJumpToLatest = useCallback(() => {
    jumpToLatest();
    focusTargetRef.current = "workspace";
  }, [jumpToLatest]);

  const captureAction = getCaptureActionPresentation(
    captureStatus,
    canStart,
    onStartRequested,
    onStopRequested,
  );
  const startDisabledForIdle =
    captureStatus === "idle" && captureAction.disabled;

  useWorkspaceShortcuts({
    onToggleCapture: captureAction.onClick,
    onCopyAll: handleCopy,
    onCancelClearConfirmation: clearConfirming ? handleClearCancel : undefined,
  });

  return (
    <div className="flex h-screen flex-col overflow-y-auto bg-[var(--bg-base)] text-[var(--text-primary)]">
      <header className="flex shrink-0 items-center justify-between gap-4 border-b border-[var(--border-default)] px-6 py-4">
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
        className="flex shrink-0 flex-wrap items-center gap-4 border-b border-[var(--border-default)] px-6 py-3 text-sm text-[var(--text-secondary)]"
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
        tabIndex={0}
        className="flex-1 overflow-y-auto px-6 py-6 focus:outline-none focus-visible:outline focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-[var(--accent-primary)]"
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
              <TranscriptRow key={segment.id} segment={segment} />
            ))}
          </ul>
        )}
      </section>

      {!isFollowing && (
        <div className="flex shrink-0 justify-center border-t border-[var(--border-default)] bg-[var(--bg-elevated)] px-6 py-2">
          <button
            type="button"
            onClick={handleJumpToLatest}
            className="flex items-center gap-1.5 rounded-lg border border-[var(--border-strong)] px-3 py-1 text-xs font-medium text-[var(--text-secondary)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-primary)]"
          >
            <ArrowDown aria-hidden="true" className="h-4 w-4" />
            Jump to latest
          </button>
        </div>
      )}

      <footer className="flex shrink-0 flex-wrap items-center justify-between gap-4 border-t border-[var(--border-default)] px-6 py-4">
        <span
          className="font-[var(--font-mono)] text-sm text-[var(--text-secondary)]"
          aria-label={`Elapsed time ${formatElapsedTime(elapsedMs)}`}
        >
          {formatElapsedTime(elapsedMs)}
        </span>

        <div className="flex flex-wrap items-center gap-3">
          {clearConfirming ? (
            <div className="flex items-center gap-2">
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
