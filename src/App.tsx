/**
 * Honest desktop bootstrap surface.
 *
 * Spec 01 owns only this static, truthful shell: the desktop runtime is
 * ready, audio capture and transcription are not configured yet. There is
 * no working-looking control, fake transcript, or hidden startup work.
 * Spec 02 replaces this component with the real transcript workspace.
 */
function App() {
  return (
    <div className="flex min-h-screen flex-col bg-[var(--bg-base)] text-[var(--text-primary)]">
      <header className="border-b border-[var(--border-default)] px-6 py-4">
        <h1 className="text-lg font-semibold tracking-tight">Mistaken</h1>
      </header>
      <main className="flex flex-1 flex-col items-center justify-center gap-2 px-6 text-center">
        <p className="text-base font-medium text-[var(--text-primary)]">
          Desktop runtime ready
        </p>
        <p className="max-w-md text-sm text-[var(--text-secondary)]">
          Audio capture and transcription are not configured yet.
        </p>
      </main>
    </div>
  );
}

export default App;
