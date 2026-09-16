/**
 * Memoized single transcript row. `React.memo`'s default shallow prop
 * comparison bails out a row whose `segment` object identity is
 * unchanged, so an interim revision for one source re-renders only that
 * row (proven by the render-isolation test in `TranscriptWorkspace.test.tsx`
 * against a roughly 1 000-segment transcript) instead of the whole list.
 *
 * An interim row overrides the ambient `role="log"` polite live region
 * with `aria-live="off"` on its own subtree, so up to roughly 13 interim
 * mutations per second across two sources are never queued for
 * announcement while remaining fully readable by normal navigation. The
 * moment a segment finalizes, its row loses that override and its content
 * is announced exactly once by the log's ambient polite semantics.
 */
import { memo } from "react";
import type { TranscriptSegment } from "../../types/transcript";
import { formatTranscriptSegment } from "./transcript-domain";

export interface TranscriptRowProps {
  readonly segment: TranscriptSegment;
}

function TranscriptRowImpl({ segment }: TranscriptRowProps) {
  return (
    <li
      className="flex flex-col gap-1"
      aria-live={segment.isFinal ? undefined : "off"}
    >
      {!segment.isFinal && (
        <span className="text-xs font-semibold uppercase tracking-wide text-[var(--text-interim)]">
          Interim
        </span>
      )}
      <p
        className={`max-w-[75ch] whitespace-pre-wrap font-[var(--font-transcript)] text-base leading-[1.65] ${
          segment.isFinal
            ? "text-[var(--text-primary)]"
            : "text-[var(--text-interim)]"
        }`}
      >
        {formatTranscriptSegment(segment)}
      </p>
    </li>
  );
}

export const TranscriptRow = memo(TranscriptRowImpl);
