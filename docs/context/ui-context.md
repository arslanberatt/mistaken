# UI Context

## Theme

Mistaken uses a dark, minimal desktop-workspace visual language. It should feel closer to a focused notepad or developer tool than an AI chat product. The transcript is the dominant element. Controls are compact, surfaces are flat, and visual noise is intentionally low.

V1 is dark-only. Do not build a light theme unless explicitly requested later.

Avoid "AI-looking" presentation patterns such as oversized gradient headings, glowing cards, decorative blobs, excessive pills, animated backgrounds, or chatbot bubbles.

## Colors

All components must use semantic CSS custom properties. Do not introduce arbitrary hardcoded color values inside feature components.

Suggested initial token set:

| Role | CSS Variable | Value |
| --- | --- | --- |
| Page background | `--bg-base` | `#0B0D10` |
| Surface | `--bg-surface` | `#111419` |
| Elevated surface | `--bg-elevated` | `#171B21` |
| Primary text | `--text-primary` | `#F3F4F6` |
| Secondary text | `--text-secondary` | `#C2C7D0` |
| Muted text | `--text-muted` | `#7C8491` |
| Primary accent | `--accent-primary` | `#8BDF9B` |
| Accent hover | `--accent-hover` | `#A3E8AF` |
| Border | `--border-default` | `#252A32` |
| Strong border | `--border-strong` | `#363D48` |
| Error | `--state-error` | `#F07178` |
| Warning | `--state-warning` | `#E8B86D` |
| Success / listening | `--state-success` | `#8BDF9B` |
| Interim transcript | `--text-interim` | `#8E96A3` |
| Selection | `--selection` | `rgba(139, 223, 155, 0.20)` |

The accent color is used sparingly for active/listening actions, focus, and primary interaction. It must not flood the interface.

## Typography

Prefer system fonts so Mistaken does not need to ship a custom font asset for V1.

| Role | Font | Variable |
| --- | --- | --- |
| UI text | `system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif` | `--font-sans` |
| Transcript | `system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif` | `--font-transcript` |
| Code/technical values | `ui-monospace, SFMono-Regular, Menlo, Consolas, monospace` | `--font-mono` |

Transcript text should normally be `16px` to `18px` with relaxed line height around `1.65` so long conversations remain readable.

Do not use monospace for the actual transcript.

## Border Radius

| Context | Radius |
| --- | --- |
| Inline / small controls | `rounded-md` / 6px |
| Buttons / inputs | `rounded-lg` / 8px |
| Panels | `rounded-xl` / 12px |
| Dialogs | `rounded-xl` / 12px |

Avoid excessive pill-shaped controls. Reserve full pills for tiny status indicators only.

## Component Library

Use shadcn/ui on top of Tailwind only where it saves time and preserves accessibility.

Good candidates:

- Button
- Select
- Tooltip
- Dialog / AlertDialog
- DropdownMenu if needed
- Separator

Do not turn the workspace into a collection of shadcn cards. The transcript surface should remain mostly custom and flat.

Generated primitives live in `src/components/ui/`. Feature behavior belongs outside that folder.

## Main Layout

Mistaken V1 is a single-window application.

```text
┌───────────────────────────────────────────────────────────┐
│ Mistaken                              ● Local • Listening │
├───────────────────────────────────────────────────────────┤
│ Mic: MacBook Microphone ▾       System Audio: Connected  │
├───────────────────────────────────────────────────────────┤
│                                                           │
│ I actually have went there yesterday.                     │
│                                                           │
│ - Why did you go there?                                   │
│                                                           │
│ Um, because my friend invited me and I didn't knew        │
│ anyone there.                                             │
│                                                           │
│ - Oh, okay.                                               │
│                                                           │
│ I was thinking maybe it will be fun...                    │
│                              ↑ interim / muted             │
│                                                           │
├───────────────────────────────────────────────────────────┤
│ 00:12:43                         Clear   Stop   Copy All   │
└───────────────────────────────────────────────────────────┘
```

### Top Bar

Contains:

- `Mistaken` product name
- Local model state
- Capture/listening state

Status examples:

- `Local • Ready`
- `Local • Listening`
- `Model missing`
- `Microphone permission required`

Do not display cloud/account controls because none exist.

### Source Bar

Contains compact capture configuration/status:

- Microphone selector
- System-audio status

Do not show dozens of technical audio parameters in the default UI.

### Transcript Surface

- Occupies most of the window.
- Scrolls vertically.
- Uses generous horizontal padding and readable line length.
- User/microphone speech is normal text.
- System speech starts with `- `.
- Do not use chat bubbles.
- Do not put avatars beside speakers.
- Do not use different colors to identify the two sources; the textual `- ` convention is the source indicator.
- Interim text appears slightly muted and is replaced/finalized in place.
- Final text uses primary text color.
- Keep paragraph spacing sufficient to see turn boundaries.

### Bottom Action Bar

Contains:

- Elapsed capture time
- `Clear`
- `Start Listening` or `Stop`
- `Copy All`

Primary action behavior:

- Idle -> `Start Listening` uses primary accent.
- Listening -> `Stop` is visually strong but should not look destructive like `Clear`.
- `Clear` should require confirmation when transcript content exists, unless the user later explicitly requests one-click clearing.
- `Copy All` shows brief non-blocking success feedback such as `Copied`.

## Empty State

Keep the empty state practical, not promotional.

Suggested content:

```text
Ready to transcribe locally.

Choose your microphone, then start listening.
System audio will appear with a "-" prefix.
```

Optional small technical line:

```text
No audio or transcript is uploaded.
```

## Permission States

Permission errors should appear close to the relevant source control.

Examples:

- `Microphone access is required to transcribe your voice.`
- `System audio permission is required to capture computer audio.`

Offer one clear action when the OS permits it, such as opening the appropriate settings page. Do not bury permission errors in toasts only.

## Model States

Required states:

- Loading model
- Ready
- Failed to load
- Missing model
- Unsupported model/runtime

The app must never imply that a cloud fallback will be used.

## Transcript Formatting

Clipboard output is plain text.

Required speaker convention:

```text
Microphone:
I was there yesterday.

System:
- Really?
```

Do not include hidden metadata, JSON, speaker IDs, confidence values, or markdown formatting in `Copy All` unless explicitly requested later.

## Interaction Details

- `Space` should not globally toggle recording because it conflicts with text interaction.
- Keyboard shortcut proposals:
  - Start/Stop: `Cmd/Ctrl + Enter`
  - Copy All: `Cmd/Ctrl + Shift + C`
  - Clear: no dangerous single-key shortcut
- Auto-scroll while listening only if the user is already near the bottom.
- If the user scrolls upward, do not forcibly pull them back to the newest line.
- Restore auto-follow when they return near the bottom or press a small `Jump to latest` control.

## Responsive Desktop Behavior

This is a desktop app, but the window may be resized.

- Minimum width target: about 720px unless implementation constraints require more.
- Collapse source labels before hiding critical controls.
- Transcript remains the highest-priority area.
- Avoid fixed heights that clip content.

## Icons

Use Lucide React.

Suggested icons:

- Microphone -> `Mic`
- Listening -> `AudioLines`
- Stop -> `Square`
- Copy -> `Copy`
- Clear -> `Trash2`
- Local model -> `Cpu`
- System audio -> `MonitorSpeaker`
- Error -> `TriangleAlert`
- Success -> `Check`

Typical sizing:

- Inline: `h-4 w-4`
- Main buttons: `h-4 w-4` or `h-5 w-5`

Icons supplement text; critical actions should not be icon-only in V1.

## Accessibility

- Maintain WCAG-appropriate contrast for text and controls.
- All interactive controls require visible keyboard focus.
- Status must not rely on color alone.
- Buttons need accessible names.
- Interim text must remain readable despite being visually muted.
- Respect reduced-motion preferences; avoid decorative animation.

## Visual Invariants

1. Transcript is the dominant visual surface.
2. No chat bubbles.
3. No avatars.
4. No AI-gradient aesthetic.
5. No account/cloud UI.
6. System speech is indicated by `- `, not by speculative speaker styling.
7. Interim text is visually distinguishable from final text.
8. The UI clearly communicates that transcription is local.
9. Errors are actionable and appear near the affected control when possible.
10. The interface remains usable without decorative animation.
