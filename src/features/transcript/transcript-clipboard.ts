/**
 * Write-only clipboard boundary. Production wires the official Tauri
 * clipboard-manager plugin (`clipboard-manager:allow-write-text` only);
 * tests inject a deterministic writer instead of touching a real clipboard
 * or falling back to `navigator.clipboard`/`document.execCommand`.
 */
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

export type ClipboardWriter = (text: string) => Promise<void>;

export const writeTranscriptToClipboard: ClipboardWriter = (text) =>
  writeText(text);
