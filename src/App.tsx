/**
 * Root composition: the transcript workspace wired to the real typed
 * runtime bridge and a real microphone controller.
 *
 * Native ASR is not integrated yet, so the committed app still supplies
 * empty transcript state, `captureStatus="idle"`, `canStart=false`, and no
 * start/stop callback for the production transcription action — that stays
 * honestly disabled. The microphone source bar, however, is now real:
 * device enumeration, selection, and a Test microphone / Stop test action
 * drive actual local PCM capture through `startCapture`/`stopCapture`. No
 * PCM, sample, or continuous level ever crosses into this component or the
 * transcript state below it.
 */
import { useState } from "react";
import { MicrophoneControl } from "./features/audio/MicrophoneControl";
import { useMicrophoneController } from "./features/audio/microphone-controller";
import { TranscriptWorkspace } from "./features/transcript/TranscriptWorkspace";
import { useTranscriptSession } from "./features/transcript/use-transcript-session";
import { writeTranscriptToClipboard } from "./features/transcript/transcript-clipboard";
import { useRuntimeBridge } from "./lib/tauri";

function App() {
  const { state, clear } = useTranscriptSession();
  const [overflowWarning, setOverflowWarning] = useState(false);

  const bridge = useRuntimeBridge({
    onCaptureError: (error) => {
      if (error.source === "microphone" && error.code === "audio_queue_overflow") {
        setOverflowWarning(true);
      }
    },
  });
  const microphoneStatus = bridge.snapshot?.microphone;
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

  return (
    <TranscriptWorkspace
      segments={state.segments}
      sessionError={state.lastError}
      captureStatus="idle"
      modelStatusLabel="Local • Runtime unavailable"
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
      canStart={false}
      onStartRequested={null}
      onStopRequested={null}
      onClearRequested={clear}
      writeClipboard={writeTranscriptToClipboard}
    />
  );
}

export default App;
