/** @type {Record<string, Array<{ titleKey: string; bodyKey: string }>>} */
export const whatsNewByVersion = {
  // Finalized 1.8.0 release notes, in display order.
  "1.8.0": [
    { titleKey: "whatsNew.windowsTitle", bodyKey: "whatsNew.windowsBody" },
    { titleKey: "whatsNew.modelsTitle", bodyKey: "whatsNew.modelsBody" },
    { titleKey: "whatsNew.cardTitle", bodyKey: "whatsNew.cardBody" },
  ],
};

export const WHATS_NEW_READ_VERSION_KEY = "lt.whats-new.read-version";

/** @param {string} left @param {string} right */
function compareVersions(left, right) {
  const a = String(left).split(".").map((part) => Number.parseInt(part, 10) || 0);
  const b = String(right).split(".").map((part) => Number.parseInt(part, 10) || 0);
  for (let i = 0; i < Math.max(a.length, b.length); i += 1) {
    if ((a[i] ?? 0) !== (b[i] ?? 0)) return (a[i] ?? 0) < (b[i] ?? 0) ? -1 : 1;
  }
  return 0;
}

/** @param {string | null} previousVersion @param {string} currentVersion */
export function shouldShowWhatsNew(previousVersion, currentVersion) {
  return Boolean(
    previousVersion &&
      whatsNewByVersion[currentVersion] &&
      compareVersions(previousVersion, currentVersion) < 0,
  );
}

/** @param {Pick<Storage, "setItem">} storage @param {string} currentVersion */
export function markWhatsNewRead(storage, currentVersion) {
  storage.setItem(WHATS_NEW_READ_VERSION_KEY, currentVersion);
}
