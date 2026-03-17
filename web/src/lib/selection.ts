export function reconcileSelection<K extends string | number, T>(
  current: K | null,
  items: T[],
  getKey: (item: T) => K,
) {
  if (items.length === 0) {
    return null;
  }
  if (current !== null && items.some((item) => getKey(item) === current)) {
    return current;
  }
  return getKey(items[0]);
}
