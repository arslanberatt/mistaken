/**
 * Public surface of the typed Tauri IPC/runtime spine.
 *
 * React components must import commands/events through this module (or its
 * constituent files) instead of calling `@tauri-apps/api` directly.
 */
export * from "./contracts";
export {
  getRuntimeSnapshot,
  listMicrophones,
  runtimeClient,
  startCapture,
  stopCapture,
} from "./runtime";
export type { RuntimeClient } from "./runtime";
export { useRuntimeBridge } from "./use-runtime-bridge";
export type { UseRuntimeBridgeResult } from "./use-runtime-bridge";
