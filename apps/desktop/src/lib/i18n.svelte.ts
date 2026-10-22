import { zh } from "./i18n/messages/zh";
import type { MessageKey } from "./i18n/messages/zh";
import { en } from "./i18n/messages/en";
import { es } from "./i18n/messages/es";
import { fr } from "./i18n/messages/fr";
import { de } from "./i18n/messages/de";
import type { Message } from "./i18n/messages/types";

export type Locale = "zh-CN" | "en" | "es" | "fr" | "de";
export type LocalePreference = "system" | Locale;

const KEY = "lt.locale.v1";

const CATALOGS: Record<Locale, Record<MessageKey, Message>> = {
  "zh-CN": zh,
  en,
  es,
  fr,
  de,
};

/** 语言名始终按各自母语显示，不随界面语言变化。 */
export const LOCALE_NAMES: Record<Locale, string> = {
  "zh-CN": "简体中文",
  en: "English",
  es: "Español",
  fr: "Français",
  de: "Deutsch",
};

/** 设置页语言选择器的顺序：跟随系统 + 各语言母语名。 */
export const LOCALE_OPTIONS: LocalePreference[] = ["system", "zh-CN", "en", "es", "fr", "de"];

function detectSystem(): Locale {
  if (typeof navigator === "undefined") return "zh-CN";
  const tags =
    navigator.languages && navigator.languages.length > 0
      ? navigator.languages
      : [navigator.language || "zh-CN"];
  for (const tag of tags) {
    const lower = String(tag).toLowerCase();
    if (lower.startsWith("zh")) return "zh-CN";
    if (lower.startsWith("en")) return "en";
    if (lower.startsWith("es")) return "es";
    if (lower.startsWith("fr")) return "fr";
    if (lower.startsWith("de")) return "de";
  }
  // 第一轮范围之外的系统语言回退英语。
  return "en";
}

function readPreference(): LocalePreference {
  try {
    const value = localStorage.getItem(KEY);
    if (value === "system" || (value !== null && value in CATALOGS)) {
      return value as LocalePreference;
    }
  } catch {
    /* private mode or unavailable storage */
  }
  return "system";
}

/** 系统语言在启动时读取一次（macOS 改语言需重启应用才生效）。 */
const SYSTEM_LOCALE = detectSystem();

let preference = $state<LocalePreference>(readPreference());

function resolved(): Locale {
  return preference === "system" ? SYSTEM_LOCALE : preference;
}

function applyDocumentLang(): void {
  if (typeof document !== "undefined") document.documentElement.lang = resolved();
}

applyDocumentLang();

function render(
  msg: Message,
  code: Locale,
  params?: Record<string, string | number>,
): string {
  let text: string;
  if (typeof msg === "string") {
    text = msg;
  } else {
    const count = typeof params?.count === "number" ? params.count : undefined;
    let category: string | undefined;
    if (count !== undefined) {
      category = pluralRules(code).select(count);
    }
    text =
      (category !== undefined ? msg[category] : undefined) ??
      msg.other ??
      Object.values(msg)[0] ??
      "";
  }
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in params ? String(params[name]) : whole,
  );
}

const pluralCache = new Map<Locale, Intl.PluralRules>();
function pluralRules(code: Locale): Intl.PluralRules {
  let rules = pluralCache.get(code);
  if (!rules) {
    rules = new Intl.PluralRules(code);
    pluralCache.set(code, rules);
  }
  return rules;
}

const formatCache = new Map<string, Intl.DateTimeFormat>();
function dateFormatter(options: Intl.DateTimeFormatOptions): Intl.DateTimeFormat {
  const code = resolved();
  const cacheKey = `${code}|${JSON.stringify(options)}`;
  let format = formatCache.get(cacheKey);
  if (!format) {
    format = new Intl.DateTimeFormat(code, options);
    formatCache.set(cacheKey, format);
  }
  return format;
}

/** 取一条文案；带 count 时按当前语言的复数规则选形，并替换 {name} 占位符。 */
export function t(key: MessageKey, params?: Record<string, string | number>): string {
  const code = resolved();
  const msg = CATALOGS[code][key] ?? zh[key] ?? key;
  return render(msg, code, params);
}

/** 历史分组日期：zh「10 月 22 日」/ en「October 22」/ de「22. Oktober」。 */
export function formatDayLabel(timestamp: number): string {
  return dateFormatter({ month: "long", day: "numeric" }).format(new Date(timestamp));
}

/** 列表时间：按语言习惯的小时制。 */
export function formatClock(timestamp: number): string {
  return dateFormatter({ hour: "2-digit", minute: "2-digit" }).format(new Date(timestamp));
}

/** 界面语言偏好：持久化在本地，选「跟随系统」时取启动时的 macOS 语言。 */
export const locale = {
  get preference(): LocalePreference {
    return preference;
  },
  set preference(value: LocalePreference) {
    preference = value;
    try {
      localStorage.setItem(KEY, value);
    } catch {
      /* ignore quota/privacy errors */
    }
    applyDocumentLang();
  },
  get resolved(): Locale {
    return resolved();
  },
};
