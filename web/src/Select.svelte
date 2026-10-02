<script context="module" lang="ts">
 let nextId=0;
</script>
<script lang="ts">
 import {tick,onMount} from 'svelte';
 export let label:string;
 export let value:any;
 export let options:{value:string|number;label:string}[];
 export let disabled=false;
 export let onchange:(value:any)=>void=()=>{};
 const id=`themed-select-${++nextId}`;
 let trigger:HTMLButtonElement,popup:HTMLDivElement,open=false,active=0,prefix='',lastTyped=0;
 $: selected=options.find(option=>option.value===value)?.label||'';
 $: if(disabled&&open)close();
 function close(){open=false;popup?.hidePopover()}
 function position(){
  const box=trigger.getBoundingClientRect();
  const width=Math.min(Math.max(box.width,180),window.innerWidth-16);
  popup.style.width=`${width}px`;
  popup.style.maxHeight=`${Math.max(100,window.innerHeight-16)}px`;
  const height=popup.getBoundingClientRect().height;
  popup.style.left=`${Math.max(8,Math.min(box.left,window.innerWidth-width-8))}px`;
  popup.style.top=`${Math.max(8,box.bottom+height+4<=window.innerHeight-8?box.bottom+4:box.top-height-4)}px`;
 }
 async function show(index=options.findIndex(option=>option.value===value)){
  if(disabled)return;
  active=Math.max(0,index);open=true;await tick();
  if(!open)return;
  popup.showPopover();position();scrollActive();
 }
 function scrollActive(){void tick().then(()=>popup?.querySelector(`#${id}-${active}`)?.scrollIntoView({block:'nearest'}))}
 function choose(index:number){value=options[index].value;close();trigger.focus();onchange(value)}
 function keyboard(event:KeyboardEvent){
  if(disabled)return;
  const key=event.key;
  if(key==='Tab'){close();return}
  if(key==='Escape'){if(open){event.preventDefault();event.stopPropagation();close()}return}
  if(key==='Enter'||key===' '){event.preventDefault();if(open)choose(active);else void show();return}
  if(['ArrowDown','ArrowUp','Home','End'].includes(key)){
   event.preventDefault();
   if(!open){void show(key==='Home'?0:key==='End'?options.length-1:undefined);return}
   active=key==='Home'?0:key==='End'?options.length-1:(active+(key==='ArrowDown'?1:-1)+options.length)%options.length;scrollActive();return;
  }
  if(key.length===1&&!event.ctrlKey&&!event.metaKey&&!event.altKey){
   event.preventDefault();const now=Date.now();prefix=now-lastTyped<700?prefix+key.toLowerCase():key.toLowerCase();lastTyped=now;
   const index=options.findIndex(option=>option.label.toLowerCase().startsWith(prefix));
   if(index>=0){if(!open)void show(index);else{active=index;scrollActive()}}
  }
 }
 onMount(()=>{
  const outside=(event:PointerEvent)=>{if(open&&!trigger.contains(event.target as Node)&&!popup.contains(event.target as Node))close()};
  const reposition=()=>{if(open)position()};
  document.addEventListener('pointerdown',outside,true);
  window.addEventListener('resize',reposition);window.addEventListener('scroll',reposition,true);
  return()=>{document.removeEventListener('pointerdown',outside,true);window.removeEventListener('resize',reposition);window.removeEventListener('scroll',reposition,true)};
 });
</script>
<div class="themed-select">
 <button bind:this={trigger} type="button" class="select-trigger" role="combobox" aria-label={label} aria-haspopup="listbox" aria-expanded={open} aria-controls={id} aria-activedescendant={open?`${id}-${active}`:undefined} data-value={value} {disabled} onclick={()=>open?close():void show()} onkeydown={keyboard} onblur={()=>close()}><span>{selected}</span><span aria-hidden="true">⌄</span></button>
 <div bind:this={popup} {id} popover="manual" class="select-popup" role="listbox" aria-label={`${label} options`}>
  {#each options as option,i}
   <button id={`${id}-${i}`} type="button" role="option" tabindex="-1" class="themed-option" class:active={active===i} aria-selected={value===option.value} data-value={option.value} onpointerdown={(event)=>event.preventDefault()} onpointermove={()=>active=i} onclick={()=>choose(i)}><span>{option.label}</span><span aria-hidden="true">{value===option.value?'✓':''}</span></button>
  {/each}
 </div>
</div>
