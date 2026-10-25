import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { clickOutside } from '../src/lib/click-outside.ts';

const source = await readFile(fileURLToPath(new URL('../src/lib/components/AppToolbar.svelte', import.meta.url)), 'utf8');

function setup() {
  const listeners = new Map();
  globalThis.window = {
    addEventListener(type, listener, options) {
      listeners.set(type, { listener, options });
    },
    removeEventListener(type, listener, options) {
      const current = listeners.get(type);
      if (current?.listener === listener && current.options === options) listeners.delete(type);
    },
    dispatchEvent(event) {
      Object.defineProperty(event, 'target', { value: this, configurable: true });
      listeners.get(event.type)?.listener(event);
      return true;
    },
  };
  let open = true;
  let closeCount = 0;
  const setOpen = (value) => {
    if (open && !value) closeCount += 1;
    open = value;
  };
  const target = (name) => ({ nodeType: 1, name });
  const triggerTarget = target('trigger');
  const panelTarget = target('panel');
  const panelChild = target('panel-child');
  const body = target('body');
  const trigger = { contains: (node) => node === triggerTarget, focus() { globalThis.document.activeElement = this; } };
  const document = globalThis.document = { activeElement: null };
  const element = { contains: (node) => node === panelTarget || node === panelChild };
  const action = clickOutside(element, {
    trigger: () => trigger,
    onOutside: () => setOpen(false),
    onEscape: () => { setOpen(false); trigger.focus(); },
    onBlur: () => setOpen(false),
  });
  const pointerdown = (targetNode) => {
    const event = new Event('pointerdown', { bubbles: true });
    Object.defineProperty(event, 'target', { value: targetNode, configurable: true });
    listeners.get('pointerdown')?.listener(event);
  };
  return {
    action, listeners, pointerdown, trigger, document,
    targets: { triggerTarget, panelTarget, panelChild, body },
    get open() { return open; },
    get closeCount() { return closeCount; },
    clickTrigger() { setOpen(!open); },
  };
}

test('outside pointerdown closes; panel pointerdown and panel scroll do not', () => {
  const state = setup();
  state.pointerdown(state.targets.body);
  assert.equal(state.open, false);
  assert.equal(state.closeCount, 1);

  const inside = setup();
  inside.pointerdown(inside.targets.panelChild);
  assert.equal(inside.open, true);
  assert.equal(inside.closeCount, 0);
  assert.equal(inside.listeners.get('pointerdown').options, true, 'listener uses capture phase');
  assert.equal(inside.listeners.has('scroll'), false, 'scrolling does not register a close listener');
  assert.equal(inside.open, true);
});

test('trigger pointerdown is excluded so its own toggle closes exactly once', () => {
  const state = setup();
  state.pointerdown(state.targets.triggerTarget);
  assert.equal(state.open, true, 'outside action must not consume the trigger event');
  state.clickTrigger();
  assert.equal(state.open, false);
  assert.equal(state.closeCount, 1, 'trigger toggle closes exactly once');
});

test('Escape closes and restores focus; window blur closes', () => {
  const escaped = setup();
  const keydown = new Event('keydown');
  Object.defineProperty(keydown, 'key', { value: 'Escape' });
  escaped.listeners.get('keydown')?.listener(keydown);
  assert.equal(escaped.open, false);
  assert.equal(escaped.document.activeElement, escaped.trigger);

  const blurred = setup();
  blurred.listeners.get('blur')?.listener(new Event('blur'));
  assert.equal(blurred.open, false);
});

test('destroy removes every listener and later pointerdown is harmless', () => {
  const state = setup();
  state.action.destroy();
  assert.equal(state.listeners.size, 0);
  assert.doesNotThrow(() => state.pointerdown(state.targets.body));
  assert.equal(state.open, true);
});

test('AppToolbar wires open-state action, trigger exclusions, and accessible expanded state', () => {
  assert.match(source, /aria-haspopup="listbox"/);
  assert.match(source, /aria-expanded=\{open\}/);
  assert.match(source, /bind:this=\{triggerEl\}/);
  assert.match(source, /use:clickOutside=/);
  assert.match(source, /trigger: \(\) => triggerEl/);
  assert.match(source, /onEscape: \(\) => \{\s*open = false;\s*triggerEl\?\.focus\(\);/);
});
