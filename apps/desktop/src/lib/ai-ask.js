// 询问 AI 卡片的纯逻辑：追问轮次、错误分类。
// 不依赖 Svelte 与 Tauri，便于 node --test 直接验证。

/** 追问在卡片内最多 3 轮；第 4 次提问提示将清空上文并重置。 */
export const MAX_ASK_TURNS = 3;

/**
 * @typedef {{ question: string, answer: string }} AskTurn
 * @typedef {{ allowed: boolean, reset: boolean, history: AskTurn[], count: number }} AskGuard
 */

/**
 * 判断下一次追问是否沿用上文。超出 3 轮时不允许携带旧上文（allowed: false），
 * 返回重置后的状态（清空历史），调用方据此提示「继续提问会清空上文」并以
 * 空上文重新发起。
 *
 * @param {{ turns: AskTurn[], count: number }} state
 * @returns {AskGuard}
 */
export function nextAskTurn({ turns, count }) {
  if (count < MAX_ASK_TURNS) {
    return { allowed: true, reset: false, history: turns, count: count + 1 };
  }
  return { allowed: false, reset: true, history: [], count: 1 };
}

/**
 * 追问历史只保留最近 3 轮（后端同样限制 3 轮上下文）。
 *
 * @param {AskTurn[]} history
 * @returns {AskTurn[]}
 */
export function trimHistory(history) {
  return history.slice(-MAX_ASK_TURNS);
}

/**
 * 卡片错误归类：模型未安装 / 内存不足 / 其它。
 *
 * @param {string} code
 * @returns {"no_model" | "low_memory" | "failed"}
 */
export function classifyLlmError(code) {
  if (code === "model_not_installed") return "no_model";
  if (code === "insufficient_memory") return "low_memory";
  return "failed";
}
