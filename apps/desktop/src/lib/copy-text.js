/**
 * 详情页「复制」的纯逻辑：选区判定、文本规范化、反馈计时。
 * 只依赖可注入的 DOM 引用与计时器，组件负责取选区、调剪贴板与 Tauri 回退。
 */

/**
 * 去首尾空白，把连续三个以上换行压成两个（保持段落感）。
 *
 * @param {string | null | undefined} text
 * @returns {string}
 */
export function normalizeCopyText(text) {
  return String(text ?? "")
    .trim()
    .replace(/\n{3,}/g, "\n\n");
}

/**
 * 决定复制内容：选区落在正文容器内才复制选区，否则复制全文。
 * 选区任一端落在正文外（例如界面上的标题）都按「没有选区」处理，避免把界面文字带进去。
 *
 * @param {object} params
 * @param {string} params.selectedText `selection.toString()` 的结果
 * @param {Node | null} params.anchorNode 选区起点节点
 * @param {Node | null} params.focusNode 选区终点节点
 * @param {HTMLElement | null} params.body 正文容器；null 表示取不到，一律退回全文
 * @param {string} params.fullText 正文纯文本
 * @returns {string} 规范化后的复制内容
 */
export function resolveCopyText({ selectedText, anchorNode, focusNode, body, fullText }) {
  const selection = String(selectedText ?? "");
  const inside =
    !!body && !!anchorNode && !!focusNode && body.contains(anchorNode) && body.contains(focusNode);
  if (inside && selection.trim()) return normalizeCopyText(selection);
  return normalizeCopyText(fullText ?? "");
}

/**
 * 复制按钮的反馈状态机：成功/失败各显示 1.5 秒后回到 idle，连续点击重置计时。
 * 计时器可注入，便于测试。
 *
 * @param {object} [options]
 * @param {number} [options.duration] 反馈持续毫秒数，默认 1500
 * @param {(state: "idle" | "copied" | "failed") => void} [options.onChange] 状态变化回调
 * @param {(fn: () => void, ms: number) => number} [options.schedule] 注入的 setTimeout
 * @param {(handle: number) => void} [options.cancel] 注入的 clearTimeout
 */
export function createCopyFeedback({
  duration = 1500,
  onChange,
  schedule = setTimeout,
  cancel = clearTimeout,
} = {}) {
  /** @type {number | null} */
  let handle = null;
  let state = /** @type {"idle" | "copied" | "failed"} */ ("idle");
  const emit = () => onChange?.(state);
  /** @param {"copied" | "failed"} next */
  const arm = (next) => {
    state = next;
    if (handle !== null) cancel(handle);
    handle = schedule(() => {
      handle = null;
      state = "idle";
      emit();
    }, duration);
    emit();
  };
  return {
    get state() {
      return state;
    },
    success: () => arm("copied"),
    failure: () => arm("failed"),
    dispose() {
      if (handle !== null) cancel(handle);
      handle = null;
    },
  };
}
