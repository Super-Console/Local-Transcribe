const GIB = 1024 ** 3;

export function onboardingSteps(platform) {
  return platform === "windows"
    ? ["welcome", "models", "privacy", "finish"]
    : ["welcome", "models", "permissions", "privacy", "finish"];
}

/** Deterministic recommendation policy; all sizes come from the live catalogs. */
export function recommendModels({ memoryBytes, availableBytes, language, modelBytes }) {
  const memoryGiB = (memoryBytes ?? 0) / GIB;
  const tier = memoryGiB <= 8 ? 0 : memoryGiB < 24 ? 1 : 2;
  const asrTiers = language === "multi"
    ? ["sensevoice-small", "sensevoice-small", "x-asr-zh-en"]
    : ["zipformer-ctc-small-zh", "x-asr-zh-en", "fire-red-asr2"];
  let asrTier = tier;
  let asrModelId = asrTiers[asrTier];
  let diskDowngraded = false;

  if (availableBytes != null && (modelBytes[asrModelId] ?? 0) * 2 > availableBytes) {
    diskDowngraded = true;
    if (asrTier > 0) asrModelId = asrTiers[--asrTier];
  }

  let languageModelId = tier === 0 ? null : tier === 1 ? "qwen3-4b" : "qwen3-8b";
  if (
    languageModelId &&
    availableBytes != null &&
    (modelBytes[languageModelId] ?? 0) * 2 > availableBytes
  ) {
    diskDowngraded = true;
    languageModelId = languageModelId === "qwen3-8b" ? "qwen3-4b" : null;
  }

  return { asrModelId, languageModelId, diskDowngraded };
}
