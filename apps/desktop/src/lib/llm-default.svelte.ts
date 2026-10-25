/** 询问 AI 的默认语言模型：模型页可切换，卡片头部显示其名称。 */

const STORAGE_KEY = "lt.llm.defaultModel";

function readStored(): string {
  try {
    return globalThis.localStorage?.getItem(STORAGE_KEY) || "qwen3-4b";
  } catch {
    return "qwen3-4b";
  }
}

class DefaultLlm {
  id = $state(readStored());

  select(next: string) {
    if (!next || next === this.id) return;
    this.id = next;
    try {
      globalThis.localStorage?.setItem(STORAGE_KEY, next);
    } catch {
      // 存储不可用时仅本次会话生效。
    }
  }
}

export const defaultLlm = new DefaultLlm();

/** 清理旧模型名中曾附带的用途后缀；新名称直接原样显示。 */
export function shortModelLabel(name: string): string {
  return name.split(" · ")[0] || name;
}
