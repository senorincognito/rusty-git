import { Fragment, type ReactNode } from "react";

/**
 * Fills a text from the language file whose `{name}` placeholders stand for markup, e.g.
 * `fill(t.pull.lead, { branch: <strong>{b}</strong> })`. Plain text placeholders use functions in the
 * language file instead; this is only for sentences that contain elements.
 */
export function fill(template: string, values: Record<string, ReactNode>): ReactNode {
  return template.split(/\{(\w+)\}/).map((piece, i) =>
    // Odd pieces are the placeholder names.
    i % 2 === 1 ? <Fragment key={i}>{values[piece] ?? `{${piece}}`}</Fragment> : piece,
  );
}
