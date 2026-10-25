import type { Action } from "svelte/action";

export type ClickOutsideOptions = {
  /** Optional element that also counts as inside (usually the trigger button). */
  trigger?: HTMLElement | null | (() => HTMLElement | null);
  onOutside: (event: PointerEvent) => void;
  onEscape?: (event: KeyboardEvent) => void;
  onBlur?: () => void;
};

function resolveTrigger(trigger: ClickOutsideOptions["trigger"]): HTMLElement | null {
  return typeof trigger === "function" ? trigger() : (trigger ?? null);
}

function containsTarget(element: HTMLElement | null, event: Event): boolean {
  if (!element) return false;
  const path = typeof event.composedPath === "function" ? event.composedPath() : [];
  if (path.includes(element)) return true;
  const target = event.target;
  return target !== null && typeof target === "object" && "nodeType" in target
    ? element.contains(target as Node)
    : false;
}

/** Close an attached floating panel for outside pointer, Escape, or window blur. */
export const clickOutside: Action<HTMLElement, ClickOutsideOptions> = (node, initialOptions) => {
  let options = initialOptions;

  const onPointerDown = (event: PointerEvent) => {
    if (containsTarget(node, event) || containsTarget(resolveTrigger(options.trigger), event)) return;
    options.onOutside(event);
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Escape" || !options.onEscape) return;
    event.preventDefault();
    options.onEscape(event);
  };

  const onWindowBlur = () => {
    options.onBlur?.();
  };

  window.addEventListener("pointerdown", onPointerDown, true);
  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("blur", onWindowBlur);

  return {
    update(nextOptions) {
      options = nextOptions;
    },
    destroy() {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("blur", onWindowBlur);
    },
  };
};
