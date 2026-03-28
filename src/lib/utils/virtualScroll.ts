/** Visible slice for a fixed row height list. */
export function virtualRange(
  scrollTop: number,
  containerHeight: number,
  rowHeight: number,
  totalItems: number,
  bufferRows = 5,
): { start: number; end: number; offsetY: number; totalHeight: number } {
  const totalHeight = totalItems * rowHeight;
  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - bufferRows);
  const visible = Math.ceil(containerHeight / rowHeight) + 2 * bufferRows;
  const end = Math.min(totalItems, start + visible);
  return { start, end, offsetY: start * rowHeight, totalHeight };
}
