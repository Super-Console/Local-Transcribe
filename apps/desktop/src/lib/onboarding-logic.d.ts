export type OnboardingStep = "welcome" | "models" | "permissions" | "privacy" | "finish";
export type RecommendationLanguage = "zh" | "multi";

export function onboardingSteps(platform: "macos" | "windows" | "other"): OnboardingStep[];
export function recommendModels(input: {
  memoryBytes?: number | null;
  availableBytes?: number | null;
  language: RecommendationLanguage;
  modelBytes: Record<string, number>;
}): {
  asrModelId: string;
  languageModelId: string | null;
  diskDowngraded: boolean;
};
