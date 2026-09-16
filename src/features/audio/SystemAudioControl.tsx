/**
 * System audio control for the source bar.
 *
 * Exposes an accessible toggle for system/computer audio capture, honest
 * status/permission/availability text, and platform-specific guidance.
 *
 * Rules:
 * - Disabled while capture is active or in transition.
 * - Keyed off error codes, not message strings:
 *   - macOS screen recording denied
 *   - macOS restart required
 *   - Windows no render endpoint
 *   - Unsupported OS version
 * - Short truthful privacy note: Mistaken captures system audio only,
 *   records nothing to disk, and uploads nothing.
 */
import { MonitorSpeaker, TriangleAlert } from "lucide-react";
import type { AudioSourceStatus } from "../../lib/tauri";
import { useSourceRecoveryPresentation } from "./recovery-presentation";
import type { RecoveryPresentation } from "./recovery-presentation";

export interface SystemAudioControlProps {
  readonly enabled: boolean;
  readonly onChange: (enabled: boolean) => void;
  readonly disabled: boolean;
  readonly systemAudioStatus: AudioSourceStatus | undefined;
}

interface Presentation {
  readonly statusText: string;
  readonly toggleDisabled: boolean;
  readonly guidance: string | null;
  readonly tone: "muted" | "success" | "warning" | "error";
  readonly icon: "none" | "alert";
}


const MACOS_DENIED_GUIDANCE =
  "macOS grants system audio through Screen Recording. Enable Mistaken in System Settings → Privacy & Security → Screen Recording.";
const MACOS_RESTART_GUIDANCE =
  "macOS grants system audio through Screen Recording. Enable Mistaken in System Settings → Privacy & Security → Screen Recording. Then relaunch Mistaken.";
const WINDOWS_NO_ENDPOINT_GUIDANCE =
  "No audio output device is available. Connect or enable an output device.";

function platformGuidanceForError(code: string, message?: string): string {
  const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad|iPod/i.test(navigator.userAgent);
  if (code === "system_audio_permission_denied") {
    if (message && message.toLowerCase().includes("relaunch")) {
      return MACOS_RESTART_GUIDANCE;
    }
    return MACOS_DENIED_GUIDANCE;
  }
  if (code === "unsupported_platform") {
    return `System audio needs a newer version of ${isMac ? "macOS" : "Windows"}.`;
  }
  if (code === "system_audio_unavailable") {
    if (isMac) {
      return "System audio is currently unavailable: no capture display found.";
    }
    return WINDOWS_NO_ENDPOINT_GUIDANCE;
  }
  if (code === "device_disconnected") {
    return isMac
      ? "System audio stream stopped unexpectedly."
      : "The default audio output device was unplugged or changed.";
  }
  return message ?? "System audio encountered an error.";
}

function buildPresentation(
  props: SystemAudioControlProps,
  recovery: RecoveryPresentation,
): Presentation {
  const { enabled, disabled, systemAudioStatus } = props;

  if (!systemAudioStatus) {
    return {
      statusText: enabled ? "Ready" : "Off",
      toggleDisabled: disabled,
      guidance: null,
      tone: "muted",
      icon: "none",
    };
  }

  switch (systemAudioStatus.status) {
    case "unavailable": {
      const guidance = platformGuidanceForError(
        systemAudioStatus.error.code,
        systemAudioStatus.error.message,
      );
      return {
        statusText: "Unavailable",
        toggleDisabled: true,
        guidance,
        tone: "muted",
        icon: "none",
      };
    }
    case "error": {
      const isRecoveryExhausted = systemAudioStatus.error.message.includes(
        "Automatic reconnection stopped.",
      );
      const guidance = isRecoveryExhausted
        ? systemAudioStatus.error.message
        : platformGuidanceForError(systemAudioStatus.error.code, systemAudioStatus.error.message);
      return {
        statusText: "Error",
        toggleDisabled: disabled,
        guidance,
        tone: "error",
        icon: "alert",
      };
    }
    case "starting":
      if (recovery.recovering) {
        return {
          statusText: `Reconnecting system audio… attempt ${recovery.recovering.attempt} of ${recovery.recovering.max}`,
          toggleDisabled: true,
          guidance: null,
          tone: "warning",
          icon: "alert",
        };
      }
      return {
        statusText: "Starting…",
        toggleDisabled: true,
        guidance: null,
        tone: "muted",
        icon: "none",
      };
    case "capturing": {
      const isReceiving = systemAudioStatus.activity === "receiving";
      if (recovery.degraded) {
        return {
          statusText:
            "System audio is transcribing slower than real time. Some audio is being skipped.",
          toggleDisabled: true,
          guidance: null,
          tone: "warning",
          icon: "none",
        };
      }
      return {
        statusText: isReceiving ? "Capturing" : "Waiting for audio…",
        toggleDisabled: true,
        guidance: null,
        tone: isReceiving ? "success" : "muted",
        icon: "none",
      };
    }
    case "idle":
    default:
      return {
        statusText: enabled ? "Ready" : "Off",
        toggleDisabled: disabled,
        guidance: null,
        tone: "muted",
        icon: "none",
      };
  }
}

const TONE_CLASS: Record<Presentation["tone"], string> = {
  muted: "text-[var(--text-secondary)]",
  success: "text-[var(--state-success)]",
  warning: "text-[var(--state-warning)]",
  error: "text-[var(--state-error)]",
};

export function SystemAudioControl(props: SystemAudioControlProps) {
  const { enabled, onChange } = props;
  const recovery = useSourceRecoveryPresentation("system", props.systemAudioStatus);
  const presentation = buildPresentation(props, recovery);

  return (
    <div className="flex flex-wrap items-center gap-2">
      <MonitorSpeaker aria-hidden="true" className="h-4 w-4 text-[var(--text-secondary)]" />
      <label
        htmlFor="system-audio-toggle"
        className="flex cursor-pointer items-center gap-2 text-sm font-medium text-[var(--text-primary)]"
      >
        <input
          id="system-audio-toggle"
          type="checkbox"
          checked={enabled}
          onChange={(e) => onChange(e.target.checked)}
          disabled={presentation.toggleDisabled}
          className="h-4 w-4 rounded border-[var(--border-default)] bg-[var(--bg-surface)] text-[var(--accent-primary)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-primary)] disabled:cursor-not-allowed disabled:opacity-50"
        />
        <span>System Audio:</span>
      </label>

      <div
        role="status"
        aria-live="polite"
        className={`flex items-center gap-1 text-xs ${TONE_CLASS[presentation.tone]}`}
      >
        {presentation.icon === "alert" && (
          <TriangleAlert aria-hidden="true" className="h-3.5 w-3.5" />
        )}
        {presentation.statusText}
      </div>

      {presentation.guidance && (
        <div className="basis-full text-xs text-[var(--state-error)]">
          {presentation.guidance}
        </div>
      )}

      <div className="basis-full text-xs text-[var(--text-muted)]">
        Mistaken captures system audio only, records nothing to disk, and uploads nothing.
      </div>
    </div>
  );
}
