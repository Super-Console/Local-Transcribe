/**
 * Pure model-name resolver shared by the UI and the Node tests.
 *
 * @param {string} value a model id (`family-folder`) or a folder name
 * @param {Array<{ directory: string, name: string }>} catalog catalog entries
 * @returns {string}
 */
export function prettyNameFrom(value, catalog = []) {
  if (!value) return value ?? "";
  const hit = catalog.find(
    (entry) =>
      entry.directory &&
      (value === entry.directory || value.endsWith(`-${entry.directory}`)),
  );
  if (hit) return hit.name;
  let name = value;
  const marker = name.indexOf("sherpa-onnx-");
  if (marker >= 0) {
    name = name.slice(marker + "sherpa-onnx-".length);
  }
  name = name.replace(/-\d{4}-\d{2}-\d{2}$/, "");
  return name || value;
}
