import {hasLanguage,preferredLanguage,languages} from './languages';
import type {components} from './api.generated';
import {api} from './api';
export type Preferences=components['schemas']['Preferences'];
export const defaults:Preferences={theme:'system',default_language:'cpp',semantic_completion:true,font_size:14,tab_width:4,word_wrap:false,minimap:false,blind_mode:false};
function availableDefaults():Preferences{return {...defaults,default_language:languages().length?preferredLanguage(defaults.default_language):defaults.default_language}}
export function localPreferences(user?:string):Preferences{
 try{
  const raw=JSON.parse(localStorage.getItem(user?`j0coder:preferences:${user}`:'j0coder:guest-preferences')||'{}'),p=availableDefaults();
  if(!raw||typeof raw!=='object')return p;
  if(['system','light','dark'].includes(raw.theme))p.theme=raw.theme;
  if(hasLanguage(raw.default_language))p.default_language=raw.default_language;
  for(const field of ['semantic_completion','word_wrap','minimap','blind_mode'] as const)if(typeof raw[field]==='boolean')p[field]=raw[field];
  if(Number.isInteger(raw.font_size)&&raw.font_size>=10&&raw.font_size<=24)p.font_size=raw.font_size;
  if([2,4,8].includes(raw.tab_width))p.tab_width=raw.tab_width;
  if(languages().length)p.default_language=preferredLanguage(p.default_language);
  return p;
 }catch{return availableDefaults()}
}
export function cachePreferences(p:Preferences,user?:string){localStorage.setItem(user?`j0coder:preferences:${user}`:'j0coder:guest-preferences',JSON.stringify(p))}
export async function loadPreferences(user:string){const p:Preferences=await api('/me/preferences');p.default_language=preferredLanguage(p.default_language);cachePreferences(p,user);return p}
