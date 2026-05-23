interface MiddleEllipsisProps {
  text: string;
  /** Number of characters from the end kept visible after the ellipsis.
   *  The head fills the remaining width and truncates with `…` on the
   *  right; visually the ellipsis sits between head and tail, i.e. in
   *  the middle of the original string. Default 10 — long enough to
   *  surface a typical filename extension + a couple of leading chars,
   *  short enough to leave room for the head on a ~200px card. */
  tail?: number;
  /** Classes applied to the outer block; should NOT include `truncate`
   *  (the component owns truncation internally). The block uses flex
   *  layout, so callers shouldn't override `display`. */
  className?: string;
  /** Tooltip override. Defaults to the full `text` so users can see the
   *  whole name on hover regardless of truncation. */
  title?: string;
}

/** Title-style text block that truncates with an ellipsis in the middle
 *  rather than at the tail. Pure CSS — splits the string at
 *  `text.length - tail`, lets the head flex-shrink with `truncate`, and
 *  pins the tail as a non-shrinking sibling. No measurement, no resize
 *  listeners; the browser handles the width math.
 *
 *  Short strings (length ≤ tail) render as-is with no ellipsis. */
export function MiddleEllipsis({
  text,
  tail = 10,
  className,
  title,
}: MiddleEllipsisProps) {
  const fullTitle = title ?? text;
  if (text.length <= tail) {
    return (
      <p className={`min-w-0 truncate ${className ?? ''}`} title={fullTitle}>
        {text}
      </p>
    );
  }
  const head = text.slice(0, -tail);
  const end = text.slice(-tail);
  return (
    <p className={`flex min-w-0 ${className ?? ''}`} title={fullTitle}>
      <span className="truncate min-w-0">{head}</span>
      <span className="whitespace-pre flex-shrink-0">{end}</span>
    </p>
  );
}
