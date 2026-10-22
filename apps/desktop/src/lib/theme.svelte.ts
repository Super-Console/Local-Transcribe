export type ThemePreference = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

const KEY = "lt.theme";

function readPreference(): ThemePreference {
  try {
    const value = localStorage.getItem(KEY);
    if (value === "light" || value === "dark" || value === "system") return value;
  } catch {
    /* private mode or unavailable storage */
  }
  return "system";
}

let preference = $state<ThemePreference>(readPreference());
let systemPrefersDark = $state(false);

function resolved(): ResolvedTheme {
  if (preference === "system") return systemPrefersDark ? "dark" : "light";
  return preference;
}

function apply(): void {
  const value = resolved();
  document.documentElement.dataset.theme = value;
  // 系统材质（NSVisualEffectView）与窗口外观必须跟随应用主题。
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    void import("@tauri-apps/api/core")
      .then(({ invoke }) => invoke("sync_window_theme", { dark: value === "dark" }))
      .catch(() => {
        /* 预览环境没有窗口 */
      });
  }
}

/** Theme state shared by the shell; the preference is persisted locally. */
export const theme = {
  get preference(): ThemePreference {
    return preference;
  },
  set preference(value: ThemePreference) {
    preference = value;
    try {
      localStorage.setItem(KEY, value);
    } catch {
      /* ignore quota/privacy errors */
    }
    apply();
  },
  get resolved(): ResolvedTheme {
    return resolved();
  },
  /** Follow the system appearance and paint the first frame with it. */
  init(): void {
    // 设计走查用：仅在 dev 服务器上可用 ?theme=dark|light 强制外观。
    if (import.meta.env.DEV) {
      const forced = new URLSearchParams(window.location.search).get("theme");
      if (forced === "light" || forced === "dark") preference = forced;
    }
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    systemPrefersDark = query.matches;
    query.addEventListener("change", (event) => {
      systemPrefersDark = event.matches;
      if (preference === "system") apply();
    });
    apply();
  },
};
