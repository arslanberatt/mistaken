/**
 * Root composition: the honest, unavailable-runtime transcript workspace.
 *
 * Native audio/ASR runtime work is not integrated yet, so the committed app
 * supplies empty transcript state, `captureStatus="idle"`, truthful
 * unavailable labels, `canStart=false`, and no start/stop callback. It does
 * not simulate readiness or seed sample transcript content. Later specs
 * (03 for typed IPC, 04 for microphone integration) map real native state
 * into this same presentation boundary.
 */
import { TranscriptWorkspace } from "./features/transcript/TranscriptWorkspace";
import { useTranscriptSession } from "./features/transcript/use-transcript-session";
import { writeTranscriptToClipboard } from "./features/transcript/transcript-clipboard";

function App() {
  const { state, clear } = useTranscriptSession();

  return (
    <TranscriptWorkspace
      segments={state.segments}
      sessionError={state.lastError}
      captureStatus="idle"
      modelStatusLabel="Local • Runtime unavailable"
      microphoneLabel="No microphone available"
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
