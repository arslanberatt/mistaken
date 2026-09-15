/**
 * Root composition: the transcript workspace wired to the real typed
 * runtime bridge, the real microphone source bar, and — as of Spec 06 —
 * real local development-adapter transcription.
 *
 * `Start Listening` / `Stop` now call the production `startCapture` /
 * `stopCapture` commands directly with the selected microphone device;
 * native transcript events flow straight into the Spec 02 reducer. The
 * top bar always carries the `Development ASR • Not release approved`
 * label: the pinned temporary adapter has not passed Spec 05's production
 * gates, so every transcript-quality result it produces is architecture
 * evidence only. No PCM, sample, or continuous level ever crosses into
 * this component or the transcript state below it.
 */
import { useState } from "react";
import { MicrophoneControl } from "./features/audio/MicrophoneControl";
import { useMicrophoneController } from "./features/audio/microphone-controller";
import { TranscriptWorkspace } from "./features/transcript/TranscriptWorkspace";
import { useTranscriptSession } from "./features/transcript/use-transcript-session";
import { writeTranscriptToClipboard } from "./features/transcript/transcript-clipboard";
import { useRuntimeBridge, type ModelStatus } from "./lib/tauri";

const NON_RELEASE_LABEL = "Development ASR • Not release approved";

function modelStatusLabel(modelStatus: ModelStatus | undefined, isListening: boolean): string {
  if (!modelStatus) return `${NON_RELEASE_LABEL} — Connecting…`;
  if (isListening) return `${NON_RELEASE_LABEL} — Listening`;
  switch (modelStatus.status) {
    case "missing":
      return `${NON_RELEASE_LABEL} — Model missing`;
    case "loading":
      return `${NON_RELEASE_LABEL} — Loading model…`;
    case "ready":
      return NON_RELEASE_LABEL;
    case "failed":
      return `${NON_RELEASE_LABEL} — Model failed to load`;
    case "unsupported":
      return `${NON_RELEASE_LABEL} — Model not supported`;
  }
}

function App() {
  const { state, receiveSegment, clear } = useTranscriptSession();
  const [overflowWarning, setOverflowWarning] = useState(false);

  const bridge = useRuntimeBridge({
    onCaptureError: (error) => {
      if (
        error.source === "microphone" &&
        (error.code === "audio_queue_overflow" || error.code === "inference_lagging")
      ) {
        setOverflowWarning(true);
      }
    },
    onTranscriptSegment: receiveSegment,
  });
  const snapshot = bridge.snapshot;
  const microphoneStatus = snapshot?.microphone;
  const microphoneController = useMicrophoneController(bridge.client, bridge.bridgeReady);

  // Reset the transient overflow banner whenever the microphone leaves
  // "capturing" (a fresh Start should not inherit a stale warning). This
  // adjusts state during render in response to a changed prop, matching
  // React's guidance for state that must reset when an external value
  // changes; it intentionally does not use an effect.
  const [previousStatus, setPreviousStatus] = useState(microphoneStatus?.status);
  if (microphoneStatus?.status !== previousStatus) {
    setPreviousStatus(microphoneStatus?.status);
    if (microphoneStatus?.status !== "capturing") {
      setOverflowWarning(false);
    }
  }

  const captureStatus = snapshot?.captureStatus ?? "idle";
  const canStart =
    bridge.bridgeReady &&
    snapshot?.modelStatus.status === "ready" &&
    microphoneController.selectedDeviceId !== null;

  function handleStart(): void {
    const deviceId = microphoneController.selectedDeviceId;
    if (!deviceId) return;
    void bridge.client.startCapture({
      microphoneDeviceId: deviceId,
      systemAudioEnabled: false,
    });
  }

  function handleStop(): void {
    void bridge.client.stopCapture();
  }

  return (
    <TranscriptWorkspace
      segments={state.segments}
      sessionError={state.lastError}
      captureStatus={captureStatus}
      modelStatusLabel={modelStatusLabel(snapshot?.modelStatus, captureStatus === "listening")}
      microphoneControl={
        <MicrophoneControl
          bridgeReady={bridge.bridgeReady}
          controller={microphoneController}
          microphoneStatus={microphoneStatus}
          overflowWarning={overflowWarning}
        />
      }
      systemAudioLabel="Not connected"
      elapsedMs={0}
      canStart={canStart}
      onStartRequested={handleStart}
      onStopRequested={handleStop}
      onClearRequested={clear}
      writeClipboard={writeTranscriptToClipboard}
    />
  );
}

export default App;
