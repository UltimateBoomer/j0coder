import {api} from './api';
export type Preferences={theme:'system'|'light'|'dark';default_language:'cpp'|'python'|'java'|'kotlin';semantic_completion:boolean;font_size:number;tab_width:number;word_wrap:boolean;minimap:boolean;blind_mode:boolean};
export const defaults:Preferences={theme:'system',default_language:'cpp',semantic_completion:true,font_size:14,tab_width:4,word_wrap:false,minimap:false,blind_mode:false};
export function localPreferences(user?:string):Preferences{
 try{
  const raw=JSON.parse(localStorage.getItem(user?`j0coder:preferences:${user}`:'j0coder:guest-preferences')||'{}'),p={...defaults};
  if(!raw||typeof raw!=='object')return p;
  if(['system','light','dark'].includes(raw.theme))p.theme=raw.theme;
  if(['cpp','python','java','kotlin'].includes(raw.default_language))p.default_language=raw.default_language;
  for(const field of ['semantic_completion','word_wrap','minimap','blind_mode'] as const)if(typeof raw[field]==='boolean')p[field]=raw[field];
  if(Number.isInteger(raw.font_size)&&raw.font_size>=10&&raw.font_size<=24)p.font_size=raw.font_size;
  if([2,4,8].includes(raw.tab_width))p.tab_width=raw.tab_width;
  return p;
 }catch{return {...defaults}}
}
export function cachePreferences(p:Preferences,user?:string){localStorage.setItem(user?`j0coder:preferences:${user}`:'j0coder:guest-preferences',JSON.stringify(p))}
export async function loadPreferences(user:string){const p:Preferences=await api('/me/preferences');cachePreferences(p,user);return p}
