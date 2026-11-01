// OOBE 是整窗视图，窗口带 macOS Overlay 标题栏，因此必须在留白区域也能拖动。
// 只把 onmousedown 绑在顶部 44px 的 header 上时，用户拖中间的空白是没反应的。
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const read = (path) => readFile(new URL(path, import.meta.url), 'utf8');

test('the whole onboarding screen can drag the window', async () => {
	const view = await read('../src/lib/components/OnboardingView.svelte');
	assert.match(
		view,
		/<section class="onboarding-screen"[^>]*onmousedown=\{dragWindow\}/,
		'OOBE 的整块留白都必须是拖动区域',
	);
	assert.match(view, /getCurrentWindow\(\)\.startDragging\(\)/, '拖动必须走 Tauri 的 startDragging');
	// 只绑一次：header 再绑一遍会让同一次按下触发两次 startDragging。
	assert.doesNotMatch(
		view,
		/<header class="topbar"[^>]*onmousedown=\{dragWindow\}/,
		'拖动处理只应绑定在顶层容器上',
	);
});

test('dragging never steals clicks, text selection or the scrollbar', async () => {
	const view = await read('../src/lib/components/OnboardingView.svelte');
	const handler = view.slice(view.indexOf('function dragWindow'), view.indexOf('function modelSize'));
	for (const selector of ['button', 'a', 'input', 'select', 'textarea', 'label', 'summary']) {
		assert.match(handler, new RegExp(selector), `交互控件 ${selector} 必须被排除`);
	}
	assert.match(handler, /\[contenteditable/, '可编辑区域必须被排除');
	assert.match(handler, /\[role="button"\]/, '伪装成按钮的元素必须被排除');
	assert.match(handler, /\.step-content/, '正文区域要保留原生文本选择');
	assert.match(handler, /getBoundingClientRect\(\)\.right - 16/, '可滚动区域右侧的滚动条要留给浏览器');
	assert.match(handler, /event\.preventDefault\(\)/, '拖动前要阻止默认行为');
});
