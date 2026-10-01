/** True when every word of the filter occurs (case-insensitively) in one of the texts; an empty filter matches all. */
export function matchesFilter(filter: string, ...texts: string[]): boolean {
  const words = filter.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return true;
  const haystack = texts.join("\n").toLowerCase();
  return words.every((w) => haystack.includes(w));
}
