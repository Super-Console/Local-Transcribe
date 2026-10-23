import { mount, tick, unmount } from 'svelte';
import Reader from '../../src/lib/components/Reader.svelte';
import Toolbar from '../../src/lib/components/AppToolbar.svelte';
import History from '../../src/lib/components/HistoryView.svelte';
import '../../src/app.css';

const failures: string[] = [];
const evidence: unknown[] = [];
const check = (value: unknown, message: string) => { if (!value) failures.push(message); };
const settle = async () => { await tick(); await new Promise(resolve => setTimeout(resolve, 300)); };
const target = document.querySelector<HTMLElement>('#fixture')!;
target.style.cssText = 'display:flex;flex-direction:column;height:600px;background:var(--bg);padding:16px';
const hit = (element: Element, label: string) => {
  const rect = element.getBoundingClientRect();
  const x = rect.x + rect.width / 2, y = rect.y + rect.height / 2;
  const found = document.elementFromPoint(x, y);
  evidence.push({ label, x, y, hit: found?.outerHTML.slice(0, 250),
    button: found?.closest('button')?.className });
  return found;
};

try {
  let backs = 0;
  const reader = mount(Reader, { target, props: {
    jobId: null, initialFollow: false,
    segments: Array.from({ length: 70 }, (_,i) => ({ text: `Synthetic paragraph ${i} alpha beta.`, startMs: i, endMs: i + 1 })),
    onBack: () => { backs++; },
  } });
  await settle();
  const back = target.querySelector<HTMLButtonElement>('.back')!;
  const find = target.querySelector<HTMLButtonElement>('.find-toggle')!;
  const bar = target.querySelector<HTMLElement>('.reader-bar')!;
  const fade = target.querySelector<HTMLElement>('.reader-fade')!;
  const scroll = target.querySelector<HTMLElement>('.reader-scroll')!;
  // Controlled reproduction of the previous inherited pointer-events bug,
  // not a claim to have captured the user's original screenshot coordinates.
  bar.style.pointerEvents = 'none';
  back.style.pointerEvents = 'none';
  check(hit(back, 'controlled-before/back')?.closest('button') !== back, 'old pointer-events reproduces blocked hit');
  bar.style.removeProperty('pointer-events');
  back.style.removeProperty('pointer-events');
  for (let attempt = 0; attempt < 2; attempt++) {
    check(hit(back, `after/back/${attempt}`) === back, 'back hit test returns button itself');
    (hit(back, `click/back/${attempt}`) as HTMLElement).click();
    check(backs === attempt + 1, 'back invokes supplied callback');
    check(hit(find, `after/find/${attempt}`) === find, 'find hit test returns button itself');
    (hit(find, `click/find/${attempt}`) as HTMLElement).click();
    await settle();
    const input = target.querySelector<HTMLInputElement>('.find-row input');
    check(input && document.activeElement === input, 'find opens and focuses input');
    input?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await settle();
    check(!target.querySelector('.find-row'), 'Escape closes search');
  }
  check(getComputedStyle(fade).pointerEvents === 'none', 'fade ignores pointer');
  check(+getComputedStyle(bar).zIndex > +getComputedStyle(fade).zIndex, 'controls above fade');
  check(scroll.getBoundingClientRect().top >= bar.getBoundingClientRect().bottom, 'scrollbar starts below toolbar');
  scroll.scrollTop = 160;
  scroll.dispatchEvent(new Event('scroll'));
  await settle();
  check(getComputedStyle(fade).opacity !== '0', 'top fade appears after scrolling');
  const r = bar.getBoundingClientRect();
  evidence.push({ label: 'top-row/control-free', x: r.x + r.width / 2, y: r.y + 20,
    hit: document.elementFromPoint(r.x + r.width / 2, r.y + 20)?.outerHTML.slice(0, 200) });
  document.documentElement.dataset.glass = 'off';
  await settle();
  check(getComputedStyle(back).backdropFilter === 'none', 'glass off removes capsule blur');
  check(getComputedStyle(fade).opacity !== '0', 'glass off preserves top fade');
  document.documentElement.dataset.glass = 'on';
  await unmount(reader);

  const history = mount(History, { target, props: {
    entries: [], loading: false, detailJobId: 'synthetic-ui-test',
    detail: { schemaVersion: '1', jobId: 'synthetic-ui-test', sourceFileName: 'Synthetic',
      modelId: 'test', engine: 'test', text: 'Synthetic text' },
    onSelect: () => {}, onExport: () => {},
  } });
  await settle();
  for (const button of target.querySelectorAll<HTMLElement>('.reader-bar .act')) {
    const style = getComputedStyle(button);
    check(style.height === '30px', 'export capsule height 30');
    check(style.borderRadius === '999px', 'export capsule full radius');
    check(style.backdropFilter.includes('blur(18px)'), 'export capsule glass');
  }
  await unmount(history);

  const toolbar = mount(Toolbar, { target, props: {
    model: { id: 'test', family: 'test', displayName: 'Long model name '.repeat(20) },
    onSearchChange: () => {}, onSelectModel: () => {}, onManageModels: () => {},
  } });
  await settle();
  for (const selector of ['.search', '.theme', '.select']) {
    const element = target.querySelector<HTMLElement>(selector)!;
    const style = getComputedStyle(element);
    check(style.height === '30px', `${selector} height 30`);
    check(style.backdropFilter.includes('blur(18px)'), `${selector} capsule glass`);
    if (selector !== '.select') check(style.width === '30px', `${selector} circular width`);
  }
  const name = target.querySelector<HTMLElement>('.name')!;
  check(name.scrollWidth > name.clientWidth && getComputedStyle(name).textOverflow === 'ellipsis', 'long model name truncates');
  target.querySelector<HTMLButtonElement>('.select')!.click();
  await settle();
  const menu = target.querySelector<HTMLElement>('.menu')!;
  for (const theme of ['light', 'dark']) {
    document.documentElement.dataset.theme = theme;
    await settle();
    const style = getComputedStyle(menu);
    const color = style.backgroundColor;
    evidence.push({ theme, menuBackground: color, opacity: style.opacity });
    check(/^rgb\(/.test(color) || /,\s*1\)$/.test(color), `${theme} menu opaque alpha`);
    check(style.opacity === '1', `${theme} menu opacity`);
    check(style.backdropFilter === 'none', `${theme} menu has no glass`);
  }
  await unmount(toolbar);
} catch (error) {
  failures.push(String(error));
}
document.querySelector('#result')!.textContent = JSON.stringify({ failures, evidence });
