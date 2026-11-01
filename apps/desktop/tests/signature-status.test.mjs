// 设置 → 关于 的构建标签必须来自运行时签名检查，不能写死。
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const read = (path) => readFile(new URL(path, import.meta.url), 'utf8');

test('the about row reads the signature state at runtime', async () => {
	const settings = await read('../src/lib/components/SettingsView.svelte');
	assert.match(settings, /invoke<\{ state: BuildState \}>\("signature_status"\)/);
	assert.match(settings, /settings\.buildNotarized/);
	assert.match(settings, /settings\.buildSigned/);
	assert.match(settings, /settings\.buildUnsigned/);
	// 旧的写死文案与它的 key 都不该再出现。
	assert.doesNotMatch(settings, /settings\.aboutBuild/);
	assert.doesNotMatch(settings, /未签名开发构建/);
});

test('the rust side asks Security for the identity and checks the staple ticket', async () => {
	const rust = await read('../src-tauri/src/signature.rs');
	assert.match(rust, /SecStaticCodeCreateWithPath/);
	assert.match(rust, /SecCodeCopySigningInformation/);
	assert.match(rust, /kSecCodeInfoTeamIdentifier/);
	assert.match(rust, /SecCertificateCopySubjectSummary/);
	// 公证只认 stapler 写入的票据，读不到就不声明。
	assert.match(rust, /fn has_stapled_ticket/);
	assert.match(rust, /\bs8ch\b/);
	const lib = await read('../src-tauri/src/lib.rs');
	assert.match(lib, /fn signature_status\(\) -> signature::SignatureInfo/);
	assert.match(lib, /signature_status,/);
});

test('all five locales carry the three build states', async () => {
	for (const locale of ['zh', 'en', 'de', 'es', 'fr']) {
		const messages = await read(`../src/lib/i18n/messages/${locale}.ts`);
		assert.match(messages, /settings\.buildUnsigned/, `${locale} missing unsigned`);
		assert.match(messages, /settings\.buildSigned/, `${locale} missing signed`);
		assert.match(messages, /settings\.buildNotarized/, `${locale} missing notarized`);
	}
});
