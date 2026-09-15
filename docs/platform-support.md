# Mistaken Platform Support

This document records the official platform floors and operating system requirements for Mistaken, as established by Spec 07, Spec 08, and integrated in Spec 09.

## macOS

- **Minimum macOS Version:** macOS 13.0 (Ventura)
- **Supported Architecture:** Apple Silicon (`aarch64-apple-darwin`)
- **System Audio Requirement:** ScreenCaptureKit with audio capture properties (`capturesAudio`, `sampleRate`, `channelCount`, `excludesCurrentProcessAudio`), which Apple introduced in macOS 13.0.
- **Permission Model:** macOS grants system audio capture under the Screen Recording permission (`CGRequestScreenCaptureAccess` / `CGPreflightScreenCaptureAccess`). Mistaken captures system audio only, does not capture screen pixels or windows, writes no audio to disk, and transmits nothing over the network.
- **Bundle Configuration:** Configured via `bundle.macOS.minimumSystemVersion = "13.0"` in `src-tauri/tauri.conf.json`. Runtime checks enforce `NSProcessInfo.isOperatingSystemAtLeastVersion({13, 0, 0})`.

## Windows

- **Windows API Floor:** Windows 10 Version 1703 (Creators Update, build 15063). Event-driven WASAPI loopback capture requires build 15063 or later to avoid polling workarounds. Enforced at runtime via `RtlGetVersion`.
- **Supported and Tested Floor:** Windows 10 Version 22H2 (build 19045) and Windows 11.
- **Untested Range:** Builds between 15063 and 19045 execute the identical code path but are explicitly untested.
- **System Audio Architecture:** WASAPI loopback on the default render endpoint (`GetDefaultAudioEndpoint(eRender, eConsole)`).
- **Permission Model:** Windows has no per-application permission gate for loopback capture of render endpoints. Mistaken does not prompt for permissions on Windows and requires no third-party virtual audio cables or drivers.
- **Packaging Floor:** Spec 14 consumes these same versions for installer prerequisites.

## Unsupported Platforms

Any platform other than macOS 13.0+ and Windows 10 1703+ compiles with an honest unavailable system-audio backend that reports `unsupported_platform` and does not claim support.
