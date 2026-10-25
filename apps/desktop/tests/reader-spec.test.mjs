import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import { setTimeout as wait } from 'node:timers/promises';
import { createServer } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const chrome=process.env.BROWSER_BIN??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
test('reader specifications: computed styles, media preferences and geometry', {
  skip:!existsSync(chrome)&&'Set BROWSER_BIN to an existing Chromium executable',timeout:90000,
},async()=>{
  const root=fileURLToPath(new URL('../',import.meta.url));
  const preferred=join(tmpdir(),'opencode');
  const scratch=mkdtempSync(join(existsSync(preferred)?preferred:tmpdir(),'lt-reader-spec-'));
  const server=await createServer({root,configFile:false,cacheDir:join(scratch,'vite'),
    plugins:[svelte({configFile:false}),{name:'reader-spec-page',configureServer(server){
      server.middlewares.use('/__spec__',(_req,res)=>{res.setHeader('Content-Type','text/html');
        res.end('<!doctype html><html><head><meta charset="utf-8"></head><body></body></html>');});}}],
    resolve:{alias:{$lib:`${root}src/lib`}},server:{host:'127.0.0.1',port:0}});
  let child,ws;
  try{
    await server.listen();
    child=spawn(chrome,['--headless=new','--disable-gpu','--no-first-run','--no-default-browser-check',
      '--remote-debugging-port=0',`--user-data-dir=${join(scratch,'chrome')}`,'about:blank'],{stdio:'ignore'});
    const active=join(scratch,'chrome','DevToolsActivePort');
    for(let i=0;i<100&&!existsSync(active);i++)await wait(100);
    assert.ok(existsSync(active),'Chrome debugging endpoint starts');
    const port=readFileSync(active,'utf8').split('\n')[0];
    const tabs=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    ws=new WebSocket(tabs.find(t=>t.type==='page').webSocketDebuggerUrl);
    await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
    let id=0;const pending=new Map();
    ws.onmessage=event=>{const data=JSON.parse(event.data);const call=pending.get(data.id);
      if(call){pending.delete(data.id);data.error?call.reject(data.error):call.resolve(data.result);}};
    const send=(method,params={})=>new Promise((resolve,reject)=>{
      const next=++id;pending.set(next,{resolve,reject});ws.send(JSON.stringify({id:next,method,params}));});
    const evaluate=async expression=>{
      const result=await send('Runtime.evaluate',{expression:`(async()=>(${expression}))()`,awaitPromise:true,returnByValue:true});
      assert.ok(!result.exceptionDetails,JSON.stringify(result.exceptionDetails));return result.result.value;};
    await send('Emulation.setDeviceMetricsOverride',{width:1000,height:800,deviceScaleFactor:1,mobile:false});
    await send('Page.navigate',{url:`http://127.0.0.1:${server.httpServer.address().port}/__spec__`});
    for(let i=0;i<100;i++){
      if(await evaluate('location.pathname === "/__spec__" && document.readyState === "complete"'))break;
      await wait(50);
    }
    await evaluate('window.probe = await import("/tests/browser/reader-spec-probe.ts")');
    const reader=await evaluate('await probe.reader()');
    await send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-transparency',value:'reduce'}]});
    const reduced=await evaluate('await probe.readerState()');
    await send('Emulation.setEmulatedMedia',{features:[{name:'prefers-contrast',value:'more'}]});
    const contrast=await evaluate('await probe.readerState()');
    await send('Emulation.setEmulatedMedia',{features:[]});
    const history=await evaluate('await probe.history()');
    const toolbar=await evaluate('await probe.toolbar()');
    const buttons=await evaluate('await probe.buttons()');
    const report={reader,reduced,contrast,history,toolbar,buttons};
    console.log(JSON.stringify(report));
    // Only implementation contracts, not appearance judgements.
    const failures=[];
    const check=(condition,label)=>{if(!condition)failures.push(label);};
    for(const page of buttons){
      check(page.primary.length<=1,`${page.name}: at most one primary`);
      check(page.inlineIcons===0,`${page.name}: no inline icons`);
      for(const b of page.primary)check(b.height==='30px'&&b.radius==='999px'&&b.bg==='rgb(193, 95, 60)',`${page.name}: primary computed contract`);
      for(const b of page.primaryDisabled)check(b.bg==='rgb(242, 239, 230)'&&b.color==='rgb(151, 146, 138)'&&b.outline==='none',`${page.name}: disabled primary uses fill/text-3/no outline`);
      for(const b of page.secondary)check(b.border==='1px'&&b.bg!=='rgb(193, 95, 60)',`${page.name}: secondary computed contract`);
      for(const b of page.danger)check(b.bg==='rgba(0, 0, 0, 0)'&&b.color==='rgb(179, 64, 46)',`${page.name}: danger computed contract`);
      if(page.name==='model')check(page.rowCapsules.every(n=>n<=1)&&page.secondary.length===1,'model: only download capsule');
      if(page.name==='settings'){
        check(page.links.length>0&&page.links.every(b=>b.bg==='rgba(0, 0, 0, 0)'&&b.radius!=='999px'),'settings: plain links');
        check(page.danger.length===1,'settings: only clear danger capsule');
      }
      if(page.name==='record')check(page.primary.length===1&&page.secondary.length===1,'record: start/add roles');
      if(page.name==='lan')check(page.primary.length===1,'lan: save role');
      if(page.name==='transcribe-idle')check(page.primary.length===1,'transcribe: idle primary role');
    }
    check(history.initial.scrollTop===0&&history.initial.titleTop>=history.initial.fadeBottom+8,'initial title clears fade by 8px');
    check(history.initial.opacity==='1'&&history.initial.ancestorEffects.every(s=>s.opacity==='1'&&s.mask==='none'),'initial title opacity and ancestor masks');
    check(history.padding==='40px'&&history.changedHeight.padding==='56px','content padding follows fade height variable');
    check(history.changedHeight.titleTop>=history.changedHeight.fadeBottom+8,'changed fade height retains title clearance');
    check(history.bottom.topFadeHeight==='72px'&&history.bottom.topFadeOpacity==='1','top fade remains at bottom');
    check(Math.abs(history.bottom.gap-40)<.1&&history.bottom.fadeHeight==='8px','existing bottom gap40 and fade8 unchanged (subpixel tolerance .1px)');
    check(reader.normal.fade.image.includes(reader.normal.effectiveBg),'fade start equals actual content background');
    check(reader.normal.fade.height==='72px','permanent top fade height 72');
    check(reader.normal.fade.image.includes('40px')&&reader.normal.fade.image.includes('56px')&&reader.normal.fade.image.includes('72px'),'multi-stop fade');
    check(reader.normal.scroll.bg==='rgba(0, 0, 0, 0)','scroll background transparent');
    check(reader.blank.class?.split(' ').includes('reader-shell'),'blank toolbar hits shell');
    check(reader.normal.scroll.z==='auto'&&reader.normal.fade.z==='2'&&reader.normal.bar.z==='3','layer order');
    check(reader.normal.shell.isolation==='isolate','shared local stacking context');
    for(const state of [reader.normal,reader.dark]){
      check(state.fade.image.includes(state.effectiveBg)&&state.bottomFade.image.includes(state.effectiveBg),'both fades use page color');
      check(state.fade.opacity==='1'&&state.scroll.bg==='rgba(0, 0, 0, 0)','both themes permanent fade and clear scroll');
    }
    check(reader.offLight.home.bg===reader.normal.effectiveBg&&reader.offDark.home.bg===reader.dark.effectiveBg,'solid fallback uses page color');
    check(reader.normal.fade.radius==='12px 12px 0px 0px','top gradient shares parent corner radius');
    check(reader.normal.bottomFade.radius==='0px 0px 12px 12px','bottom gradient shares parent corner radius');
    check(reader.normal.bar.bg==='rgba(0, 0, 0, 0)'&&reader.normal.bar.blur==='none'&&reader.normal.bar.shadow==='none','bar transparent/no blur/no shadow');
    check(reader.normal.bar.border.startsWith('0px none'),'bar has no border');
    check(reader.normal.scroll.mask==='none','reader has no mask');
    check(reader.normal.bar.gap==='8px','capsule gap 8');
    check(reader.pressed.bg==='rgba(255, 254, 251, 0.82)'&&reader.pressed.transform==='matrix(1, 0, 0, 1, 0, 1)','find pressed state');
    check(reader.fades.every(f=>f.opacity==='1'),'fade stays visible at 0/2/3/middle');
    check(history.exportGap===8,'export gap 8');
    check(toolbar.arrow.width==='12px'&&toolbar.widths.every(w=>w.arrowWidth==='12px'),'model arrow keeps 12px under flex shrink');
    check(toolbar.widths.every(w=>w.modelRight<=w.toolbarRight),'model does not overflow narrow toolbar');
    for(const control of [history.txt,history.json,...Object.values(toolbar.normal)]){
      check(control.height==='30px'&&control.radius==='999px','all secondary capsule dimensions');
      check(control.bg==='rgba(255, 254, 251, 0.66)','all secondary capsule fills');
    }
    for(const state of [reader.offLight,reader.offDark,reduced])for(const key of ['home','find']){
      const c=state[key];
      check(c.bg===c.fill,`${key}: glass-off/reduced uses --fill`);
      check(/^rgb\(/.test(c.bg)||/, 1\)$/.test(c.bg),`${key}: glass-off/reduced is opaque`);
      check(c.blur==='none',`${key}: glass-off/reduced blur none`);
    }
    for(const key of ['home','find']){
      check(contrast[key].color==='rgb(20, 20, 19)',`${key}: contrast text token`);
      check(contrast[key].border.includes('0.14'),`${key}: contrast hairline-2`);
    }
    for(const [width,label] of [[1280,'desktop'],[375,'mobile']]){
      await send('Emulation.setDeviceMetricsOverride',{width,height:800,deviceScaleFactor:1,mobile:width===375});
      const rows=await evaluate('await probe.modelMetadata()');
      check(rows.length===5,`${label}: all four catalog types and one imported model render`);
      for(const [index,row] of rows.entries()){
        check(/Estimated memory|预计内存|Geschätzter Speicher|Memoria estimada|Mémoire estimée/.test(row.text),
          `${label} row ${index}: memory estimate is visible`);
        check(/Suggested:|建议：|Empfehlung:|Recomendado:|Conseil :/.test(row.text),
          `${label} row ${index}: usage suggestion is visible`);
        check(!/License|Lizenz|Licencia|Licence|许可证/.test(row.text),
          `${label} row ${index}: license is not displayed`);
        check(row.width>0&&row.height>0,`${label} row ${index}: subtitle has layout`);
        check(row.scrollWidth<=row.width,`${label} row ${index}: subtitle wraps without horizontal overflow`);
      }
    }
    check(history.home.scrollbarWidth==='thin','home scrollbar thin');
    check(history.list.scrollbarWidth==='thin','actual home list scrollbar thin');
    assert.deepEqual(failures,[]);
  }finally{
    ws?.close();child?.kill('SIGTERM');await server.close();
  }
});
