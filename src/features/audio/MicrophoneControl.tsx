/**
 * Real microphone selector, Refresh, and Test microphone / Stop test
 * controls for the source bar. Presentation only: native lifecycle lives
 * behind `useMicrophoneController` and the typed runtime client.
 */
import type { ChangeEvent } from "react";
import { Mic } from "lucide-react";
import type { AudioSourceStatus } from "../../lib/tauri";
import type { UseMicrophoneControllerResult } from "./microphone-controller";

export interface MicrophoneControlProps {
  readonly bridgeReady: boolean;
  readonly controller: UseMicrophoneControllerResult;
  readonly microphoneStatus: AudioSourceStatus | undefined;
  readonly overflowWarning: boolean;
}

interface Presentation {
  readonly statusText: string;
  readonly actionLabel: string;
  readonly actionDisabled: boolean;
  readonly onAction: (() => void) | undefined;
  readonly selectorDisabled: boolean;
  readonly refreshDisabled: boolean;
  readonly guidance: string | null;
  readonly tone: "muted" | "success" | "warning" | "error";
}

function permissionGuidance(): string {
  const isMac =
    typeof navigator !== "undefined" &&
    (/Mac/.test(navigator.platform) || /Mac/.test(navigator.userAgent));
  return isMac
    ? "Microphone access is off. Enable Mistaken in System Settings → Privacy & Security → Microphone."
    : "Check Settings → Privacy & security → Microphone → Let desktop apps access your microphone.";
}

function buildPresentation(props: MicrophoneControlProps): Presentation {
  const { bridgeReady, controller, microphoneStatus, overflowWarning } = props;

  if (!bridgeReady) {
    return {
      statusText: "Connecting to local audio…",
      actionLabel: "Test microphone",
      actionDisabled: true,
      onAction: undefined,
      selectorDisabled: true,
      refreshDisabled: true,
      guidance: null,
      tone: "muted",
    };
  }

  if (controller.listState === "loading" && controller.devices.length === 0) {
    return {
      statusText: "Finding microphones…",
      actionLabel: "Test microphone",
      actionDisabled: true,
      onAction: undefined,
      selectorDisabled: true,
      refreshDisabled: true,
      guidance: null,
      tone: "muted",
    };
  }

  if (controller.listState === "ready" && controller.devices.length === 0) {
    return {
      statusText: "No microphone found.",
      actionLabel: "Test microphone",
      actionDisabled: true,
      onAction: undefined,
      selectorDisabled: true,
      refreshDisabled: false,
      guidance: null,
      tone: "muted",
    };
  }

  const status = microphoneStatus?.status ?? "idle";

  if (controller.commandPending === "start" || status === "starting") {
    return {
      statusText: "Starting test…",
      actionLabel: "Starting test…",
      actionDisabled: true,
      onAction: undefined,
      selectorDisabled: true,
      refreshDisabled: true,
      guidance: null,
      tone: "muted",
    };
  }

  if (controller.commandPending === "stop" || status === "stopping") {
    return {
      statusText: "Stopping test…",
      actionLabel: "Stopping test…",
      actionDisabled: true,
      onAction: undefined,
      selectorDisabled: true,
      refreshDisabled: true,
      guidance: null,
      tone: "muted",
    };
  }

  if (status === "capturing" && microphoneStatus?.status === "capturing") {
    const receiving = microphoneStatus.activity === "receiving";
    return {
      statusText: overflowWarning
        ? "Microphone input is delayed; some audio was dropped."
        : receiving
          ? "PCM signal received"
          : "Waiting for microphone signal…",
      actionLabel: "Stop test",
      actionDisabled: false,
      onAction: controller.stopTest,
      selectorDisabled: true,
      refreshDisabled: true,
      guidance: null,
      tone: overflowWarning ? "warning" : receiving ? "success" : "muted",
    };
  }

  if (status === "error" && microphoneStatus?.status === "error") {
    const code = microphoneStatus.error.code;
    if (code === "microphone_permission_denied") {
      return {
        statusText: "Microphone access is off.",
        actionLabel: "Test microphone",
        actionDisabled: true,
        onAction: undefined,
        selectorDisabled: false,
        refreshDisabled: false,
        guidance: permissionGuidance(),
        tone: "error",
      };
    }
    return {
      statusText: microphoneStatus.error.message,
      actionLabel: "Test microphone",
      actionDisabled: !controller.selectedDeviceId,
      onAction: controller.selectedDeviceId ? controller.startTest : undefined,
      selectorDisabled: false,
      refreshDisabled: false,
      guidance: null,
      tone: "error",
    };
  }

  if (controller.error) {
    return {
      statusText: controller.error.message,
      actionLabel: "Test microphone",
      actionDisabled: !controller.selectedDeviceId,
      onAction: controller.selectedDeviceId ? controller.startTest : undefined,
      selectorDisabled: false,
      refreshDisabled: false,
      guidance: null,
      tone: "error",
    };
  }

  return {
    statusText: "Ready to test locally.",
    actionLabel: "Test microphone",
    actionDisabled: !controller.selectedDeviceId,
    onAction: controller.selectedDeviceId ? controller.startTest : undefined,
    selectorDisabled: false,
    refreshDisabled: false,
    guidance: null,
    tone: "muted",
  };
}

const TONE_CLASS: Record<Presentation["tone"], string> = {
  muted: "text-[var(--text-secondary)]",
  success: "text-[var(--state-success)]",
  warning: "text-[var(--state-warning)]",
  error: "text-[var(--state-error)]",
};

export function MicrophoneControl(props: MicrophoneControlProps) {
  const { controller } = props;
  const presentation = buildPresentation(props);

  function handleSelectChange(event: ChangeEvent<HTMLSelectElement>) {
    controller.selectDevice(event.target.value);
  }

  return (
    <div className="flex flex-wrap items-center gap-2">
      <Mic aria-hidden="true" className="h-4 w-4" />
      <label htmlFor="microphone-select" className="sr-only">
        Microphone
      </label>
      <select
        id="microphone-select"
        value={controller.selectedDeviceId ?? ""}
        onChange={handleSelectChange}
        disabled={presentation.selectorDisabled || controller.devices.length === 0}
        className="rounded-md border border-[var(--border-default)] bg-[var(--bg-surface)] px-2 py-1 text-sm text-[var(--text-primary)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-primary)] disabled:cursor-not-allowed disabled:opacity-50"
      >
        {controller.devices.length === 0 ? (
          <option value="">No microphone available</option>
        ) : (
          controller.devices.map((device) => (
            <option key={device.id} value={device.id}>
              {device.label}
              {device.isDefault ? " (default)" : ""}
            </option>
          ))
        )}
      </select>

      <button
        type="button"
        onClick={controller.refresh}
        disabled={presentation.refreshDisabled}
        className="rounded-md border border-[var(--border-default)] px-2 py-1 text-xs font-medium text-[var(--text-secondary)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-primary)] disabled:cursor-not-allowed disabled:opacity-50"
      >
        Refresh microphones
      </button>

      <button
        type="button"
        onClick={presentation.onAction}
        disabled={presentation.actionDisabled}
        className="rounded-md border border-[var(--border-default)] px-2 py-1 text-xs font-medium text-[var(--text-secondary)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-primary)] disabled:cursor-not-allowed disabled:opacity-50"
      >
        {presentation.actionLabel}
      </button>

      <div role="status" aria-live="polite" className={`text-xs ${TONE_CLASS[presentation.tone]}`}>
        {presentation.statusText}
      </div>

      {presentation.guidance && (
        <div className="basis-full text-xs text-[var(--state-error)]">{presentation.guidance}</div>
      )}

      <div className="basis-full text-xs text-[var(--text-muted)]">
        Test only — no recording or transcription is saved.
      </div>
    </div>
  );
}
