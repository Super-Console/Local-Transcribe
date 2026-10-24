import { mount, tick, unmount } from 'svelte';
import Reader from '../../src/lib/components/Reader.svelte';
import History from '../../src/lib/components/HistoryView.svelte';
import Toolbar from '../../src/lib/components/AppToolbar.svelte';
import RecordView from '../../src/lib/components/RecordView.svelte';
import LanView from '../../src/lib/components/LanView.svelte';
import ModelView from '../../src/lib/components/ModelView.svelte';
import SettingsView from '../../src/lib/components/SettingsView.svelte';
import TranscribeView from '../../src/lib/components/TranscribeView.svelte';
import '../../src/app.css';

const target = document.createElement('div');
document.body.append(target);
target.style.cssText = 'display:flex;flex-direction:column;height:600px;background:var(--bg);padding:16px';
document.documentElement.dataset.glass = 'on';
const settle = async () => { await tick(); await new Promise(r => setTimeout(r, 350)); };
const props = { jobId: 'synthetic-reader-spec', initialFollow: false, onBack: () => {},
  segments: Array.from({length: 70}, (_, i) => ({startMs:i,endMs:i+1,text:`Synthetic text ${i}`})) };
const css = (selector: string, pseudo?: string) => {
  const el = target.querySelector<HTMLElement>(selector)!;
  if(!el) throw new Error(`Missing probe element: ${selector}`);
  const s = getComputedStyle(el, pseudo);
  const r = el.getBoundingClientRect();
  return { position:s.position,background:s.background,width:s.width,height:s.height,radius:s.borderRadius,bg:s.backgroundColor,
    image:s.backgroundImage,border:s.borderTop,shadow:s.boxShadow,blur:s.backdropFilter,
    pointer:s.pointerEvents,z:s.zIndex,gap:s.gap,opacity:s.opacity,overflow:s.overflow,
    paddingTop:s.paddingTop,mask:s.maskImage,outline:s.outline,top:r.top,bottom:r.bottom,
    isolation:s.isolation,transform:s.transform,fill:(()=>{const style=document.createElement('span').style;
      style.backgroundColor=s.getPropertyValue('--fill').trim();return style.backgroundColor;})(),
    color:s.color,scrollbarWidth:s.scrollbarWidth,scrollbarColor:s.scrollbarColor };
};
const point = (x:number,y:number) => {
  const el = document.elementFromPoint(x,y);
  return {x,y,tag:el?.tagName,class:el?.className};
};
let instance: ReturnType<typeof mount>;

export async function readerState() {
  await settle();
  const body = target.querySelector<HTMLElement>('.reader-body')!;
  let parent:HTMLElement|null=body;
  let effectiveBg='';
  while(parent) {
    const bg=getComputedStyle(parent).backgroundColor;
    if(bg !== 'rgba(0, 0, 0, 0)') {effectiveBg=bg;break;}
    parent=parent.parentElement;
  }
  return {home:css('.back'),find:css('.find-toggle'),bar:css('.reader-bar'),
    fade:css('.reader-fade'),bottomFade:css('.fade-bottom'),scroll:css('.reader-scroll'),shell:css('.reader-shell'),
    latest:css('.latest'),effectiveBg};
}

export async function reader() {
  if(instance) await unmount(instance);
  instance=mount(Reader,{target,props}); await settle();
  const normal=await readerState();
  const scroll=target.querySelector<HTMLElement>('.reader-scroll')!;
  const fades=[];
  for(const top of [0,2,3,160]) {
    scroll.scrollTop=top;scroll.dispatchEvent(new Event('scroll'));await settle();
    fades.push({top:scroll.scrollTop,opacity:getComputedStyle(target.querySelector('.reader-fade')!).opacity});
  }
  const centers=['.back','.find-toggle'].map(selector=>{
    const r=target.querySelector(selector)!.getBoundingClientRect();
    return {selector,...point(r.x+r.width/2,r.y+r.height/2)};
  });
  const barRect=target.querySelector('.reader-bar')!.getBoundingClientRect();
  const blank=point(barRect.x+barRect.width/2,barRect.y+20);
  target.querySelector<HTMLButtonElement>('.find-toggle')!.click();await settle();
  const pressed=css('.find-toggle');
  target.querySelector<HTMLInputElement>('.find-row input')!.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));
  await settle();
  document.documentElement.dataset.theme='dark';const dark=await readerState();
  document.documentElement.dataset.theme='light';
  document.documentElement.dataset.glass='off';const offLight=await readerState();
  document.documentElement.dataset.theme='dark';const offDark=await readerState();
  document.documentElement.dataset.theme='light';document.documentElement.dataset.glass='on';
  return {normal,dark,offLight,offDark,fades,centers,pressed,blank};
}

export async function history() {
  if(instance) await unmount(instance);
  instance=mount(History,{target,props:{entries:[],loading:false,onSelect:()=>{},onExport:()=>{}}});
  await settle();const home=css('.content');const list=css('.list');
  await unmount(instance);
  instance=mount(History,{target,props:{entries:[],loading:false,onSelect:()=>{},onExport:()=>{},
    detailJobId:'synthetic-spec',detail:{schemaVersion:'1',jobId:'synthetic-spec',sourceFileName:'Synthetic',
      modelId:'test',engine:'test',text:'Synthetic text',segments:props.segments}}});
  await settle();
  const scroller=target.querySelector<HTMLElement>('.reader-scroll')!;
  scroller.scrollTop=0;scroller.dispatchEvent(new Event('scroll'));await settle();
  const title=target.querySelector<HTMLElement>('.detail-head .file')!;
  const initial={scrollTop:scroller.scrollTop,titleTop:title.getBoundingClientRect().top,
    fadeBottom:target.querySelector('.reader-fade')!.getBoundingClientRect().bottom,
    opacity:getComputedStyle(title).opacity,
    ancestorEffects:[] as {opacity:string;mask:string}[]};
  for(let el:HTMLElement|null=title;el;el=el.parentElement){
    const s=getComputedStyle(el);initial.ancestorEffects.push({opacity:s.opacity,mask:s.maskImage});
  }
  const content=target.querySelector<HTMLElement>('.reader-content')!;
  const padding=getComputedStyle(content).paddingTop;
  const shell=target.querySelector<HTMLElement>('.reader-shell')!;
  shell.style.setProperty('--reader-fade-height','88px');await settle();
  const changedHeight={padding:getComputedStyle(content).paddingTop,
    titleTop:title.getBoundingClientRect().top,fadeBottom:target.querySelector('.reader-fade')!.getBoundingClientRect().bottom};
  shell.style.removeProperty('--reader-fade-height');await settle();
  scroller.scrollTop=scroller.scrollHeight;scroller.dispatchEvent(new Event('scroll'));await settle();
  const last=target.querySelector('.reader-body p:last-child')!.getBoundingClientRect();
  const bottom={gap:scroller.getBoundingClientRect().bottom-last.bottom,
    fadeHeight:css('.fade-bottom').height,topFadeHeight:css('.reader-fade').height,topFadeOpacity:css('.reader-fade').opacity};
  scroller.scrollTop=0;scroller.dispatchEvent(new Event('scroll'));await settle();
  const buttons=[...target.querySelectorAll<HTMLButtonElement>('.reader-bar .act')];
  const json=buttons[1].getBoundingClientRect();
  const scroll=target.querySelector('.reader-scroll')!.getBoundingClientRect();
  const bar=target.querySelector('.reader-bar')!.getBoundingClientRect();
  return {initial,bottom,padding,changedHeight,home,list,shell:css('.reader-shell'),scroll:css('.reader-scroll'),
    latest:target.querySelector('.latest')?css('.latest'):null,
    txt:css('.act'),json:css('.act:last-of-type'),
    exportGap:buttons[1].getBoundingClientRect().left-buttons[0].getBoundingClientRect().right,
    currentPoints:{json:point(json.x+json.width/2,json.y+json.height/2),
      topPresent:[...target.querySelectorAll('*')].some(el=>el.children.length===0 && el.textContent==='TOP'),
      scrollLane:point(scroll.right-4,scroll.top+20),headerLane:point(scroll.right-4,bar.top+20)},
    track:css('.reader-scroll','::-webkit-scrollbar-track')};
}

export async function toolbar() {
  if(instance) await unmount(instance);
  instance=mount(Toolbar,{target,props:{model:{id:'test',family:'test',displayName:'Long model '.repeat(50)},
    onSearchChange:()=>{},onSelectModel:()=>{},onManageModels:()=>{}}});await settle();
  const normal={search:css('.search'),moon:css('.theme'),model:css('.select')};
  const arrow=css('.select svg');
  const name=target.querySelector<HTMLElement>('.name')!;
  const longName={clientWidth:name.clientWidth,scrollWidth:name.scrollWidth,
    overflow:getComputedStyle(name).overflow,textOverflow:getComputedStyle(name).textOverflow,
    toolbarRight:target.querySelector('.toolbar')!.getBoundingClientRect().right,
    modelRight:target.querySelector('.select')!.getBoundingClientRect().right};
  target.querySelector<HTMLButtonElement>('.select')!.click();await settle();
  const light=css('.menu');document.documentElement.dataset.theme='dark';await settle();
  const dark={search:css('.search'),moon:css('.theme'),model:css('.select'),menu:css('.menu')};
  document.documentElement.dataset.theme='light';
  target.querySelector<HTMLButtonElement>('.select')!.click();await settle();
  const widths=[];
  for(const width of [436,320]) {
    target.style.width=`${width}px`;await settle();
    const frame=target.querySelector('.toolbar')!.getBoundingClientRect();
    const button=target.querySelector('.select')!.getBoundingClientRect();
    widths.push({width,toolbarRight:frame.right,modelRight:button.right,arrowWidth:css('.select svg').width});
  }
  target.style.removeProperty('width');
  return {normal,dark,menuLight:light,longName,arrow,widths};
}

export async function buttons() {
  if(instance) await unmount(instance);
  const noop=()=>{};
  const pages:unknown[]=[];
  const inspect=(name:string)=>{
    const style=(el:Element)=>{
      const s=getComputedStyle(el);
      return {class:el.className,height:s.height,radius:s.borderRadius,bg:s.backgroundColor,
        border:s.borderTopWidth,color:s.color,accent:s.getPropertyValue('--accent').trim(),
        red:s.getPropertyValue('--red').trim(),outline:s.outlineStyle};
    };
    const primary=[...target.querySelectorAll('.btn-primary')].map(style);
    const secondary=[...target.querySelectorAll('.btn-secondary')].map(style);
    const danger=[...target.querySelectorAll('.btn-danger')].map(style);
    const links=[...target.querySelectorAll('.link-action')].map(style);
    const primaryDisabled=[...target.querySelectorAll<HTMLButtonElement>('.btn-primary')].map(button=>{
      const wasDisabled=button.disabled,transition=button.style.transition;
      button.style.transition='none';button.disabled=true;
      const measured=style(button);button.disabled=wasDisabled;button.style.transition=transition;return measured;
    });
    pages.push({name,primary,primaryDisabled,secondary,danger,links,
      rowCapsules:[...target.querySelectorAll('.row')].map(row=>row.querySelectorAll('.btn-primary,.btn-secondary,.btn-danger').length),
      inlineIcons:target.querySelectorAll('.link-action svg').length});
  };
  instance=mount(RecordView,{target,props:{onRecorded:noop}});await settle();inspect('record');await unmount(instance);
  instance=mount(LanView,{target,props:{settings:null,onSettingsSaved:noop}});await settle();inspect('lan');await unmount(instance);
  instance=mount(ModelView,{target,props:{models:[{id:'test',displayName:'Synthetic',family:'test',bytes:1,selected:false}],
    catalog:[{id:'available',name:'Synthetic available',directory:'test',family:'test',kind:'asr',tier:'quality',languages:'en',bytes:1,installed:false},
      {id:'installed',name:'Synthetic installed',directory:'test2',family:'test',kind:'asr',tier:'speed',languages:'en',bytes:1,installed:true}],
    downloads:{},busy:false,onSelect:noop,onDelete:noop,onDeleteCatalog:noop,onDownload:noop,onCancelDownload:noop}});
  await settle();inspect('model');await unmount(instance);
  instance=mount(SettingsView,{target,props:{model:null,storage:null,service:null,appVersion:'test',retention:null,lanMode:null,lanSettings:null,
    onLanSettingsChange:async()=>{},onOpenLibrary:noop,onOpenHistory:noop,onOpenPath:noop,onOpenNotices:noop,onGoModels:noop,onGoLan:noop,onRefreshService:noop,onRetentionChange:noop}});
  await settle();inspect('settings');await unmount(instance);
  instance=mount(History,{target,props:{entries:[],loading:false,onSelect:noop,onExport:noop}});
  await settle();inspect('history');await unmount(instance);
  instance=mount(History,{target,props:{entries:[],loading:false,onSelect:noop,onExport:noop,detailJobId:'button-spec',
    detail:{schemaVersion:'1',jobId:'button-spec',sourceFileName:'Synthetic',modelId:'test',engine:'test',text:'Synthetic'}}});
  await settle();inspect('history-detail');await unmount(instance);
  const transcribeProps={modelId:null,fileName:null,job:null,segments:[],transcript:null,error:'',busy:false,dropping:false,
    onPickFile:noop,onChooseModel:noop,onCancel:noop,onExport:noop,onCopy:noop,onBack:noop};
  instance=mount(TranscribeView,{target,props:transcribeProps});
  await settle();inspect('transcribe-idle');await unmount(instance);
  instance=mount(TranscribeView,{target,props:{...transcribeProps,modelId:'test',segments:props.segments,
    transcript:{schemaVersion:'1',jobId:'button-spec-result',sourceFileName:'Synthetic',modelId:'test',engine:'test',text:'Synthetic'}}});
  await settle();inspect('transcribe-result');
  return pages;
}
