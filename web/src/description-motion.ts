// Only virtualized, overflowing descriptions animate; transforms preserve row geometry.
export function descriptionMotion(node:HTMLElement,text:string){
 const content=node.querySelector('span')!;
 const row=node.closest('.problem-row') as HTMLElement;
 const reduced=window.matchMedia('(prefers-reduced-motion: reduce)');
 let animation:Animation|undefined,alive=true,hovered=false;
 function measure(){
  animation?.cancel();animation=undefined;
  const distance=content.offsetWidth-node.clientWidth;
  if(!hovered||reduced.matches||distance<=0)return;
  const travel=distance/25*1000,startHold=1000,endHold=2000,duration=travel*2+startHold+endHold;
  animation=content.animate([
   {transform:'translateX(0)',offset:0},
   {transform:'translateX(0)',offset:startHold/duration},
   {transform:`translateX(-${distance}px)`,offset:(startHold+travel)/duration},
   {transform:`translateX(-${distance}px)`,offset:(startHold+endHold+travel)/duration},
   {transform:'translateX(0)',offset:1}
  ],{duration,iterations:Infinity,easing:'linear'});
 }
 const observer=new ResizeObserver(measure);observer.observe(node);observer.observe(content);
 const start=()=>{hovered=true;measure()};
 const reset=()=>{hovered=false;animation?.cancel();animation=undefined};
 row.addEventListener('pointerenter',start);row.addEventListener('pointerleave',reset);row.addEventListener('focusout',reset);
 reduced.addEventListener('change',measure);
 void document.fonts.ready.then(()=>{if(alive)measure()});
 return {
  update(next:string){if(next!==text){text=next;queueMicrotask(()=>{if(alive)measure()})}},
  destroy(){alive=false;observer.disconnect();animation?.cancel();reduced.removeEventListener('change',measure);row.removeEventListener('pointerenter',start);row.removeEventListener('pointerleave',reset);row.removeEventListener('focusout',reset)}
 };
}
