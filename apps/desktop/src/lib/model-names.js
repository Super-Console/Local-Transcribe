/**
 * Find the manifest entry a stored model id belongs to, or null.
 *
 * Installed ids are `family-directory`, so a catalog hit is either the
 * directory itself or the id ending with `-directory`. Kept separate from
 * [`prettyNameFrom`] so the model page can tell "in the manifest" from
 * "installed but no longer listed".
 *
 * @param {string|null|undefined} value
 * @param {Array<{ directory: string, name: string }>} catalog
 * @returns {{ directory: string, name: string } | null}
 */
export function catalogEntryFor(value, catalog = []) {
  if (!value) return null;
  return (
    catalog.find(
      (entry) =>
        entry.directory &&
        (value === entry.directory || value.endsWith(`-${entry.directory}`)),
    ) ?? null
  );
}

/**
 * Pure model-name resolver shared by the UI and the Node tests.
 *
 * @param {string} value a model id (`family-folder`) or a folder name
 * @param {Array<{ directory: string, name: string }>} catalog catalog entries
 * @returns {string}
 */
export function prettyNameFrom(value, catalog = []) {
  if (!value) return value ?? "";
  const hit = catalogEntryFor(value, catalog);
  if (hit) return hit.name;
  let name = value;
  const marker = name.indexOf("sherpa-onnx-");
  if (marker >= 0) {
    name = name.slice(marker + "sherpa-onnx-".length);
  }
  name = name.replace(/-\d{4}-\d{2}-\d{2}$/, "");
  return name || value;
}
