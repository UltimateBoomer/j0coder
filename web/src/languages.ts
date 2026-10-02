import type {components} from './api.generated';
export type Language=components['schemas']['Language'];
export type LanguageDescriptor=components['schemas']['LanguageDescriptor'];
import {syntaxProviders} from './syntax-providers';
let descriptors:LanguageDescriptor[]=[];
export function languages(){return descriptors}
export function initializeLanguages(input:unknown){
 if(!Array.isArray(input))throw new Error('Language metadata is missing. Retry initialization.');
 const seen=new Set<string>();
 const usable:LanguageDescriptor[]=[];
 for(const raw of input){
  if(!raw||typeof raw!=='object'||typeof raw.id!=='string'||seen.has(raw.id))throw new Error('Invalid language metadata. Retry initialization.');
  seen.add(raw.id);
  for(const key of ['label','editor_label','monaco_language','file_extension','line_comment','editor_filename','editor_uri'])if(typeof raw[key]!=='string'||!raw[key].trim())throw new Error('Invalid language metadata. Retry initialization.');
  if(!raw.file_extension.startsWith('.')||!raw.editor_uri.startsWith('file:///workspace/')||raw.editor_uri.split('/').pop()!==raw.editor_filename)throw new Error('Invalid language metadata. Retry initialization.');
  if(Object.hasOwn(syntaxProviders,raw.id))usable.push(raw as LanguageDescriptor);
 }
 if(!usable.length)throw new Error('No supported editor languages. Retry initialization.');
 descriptors=usable;
}
export function descriptor(id:Language){const value=descriptors.find(d=>d.id===id);if(!value)throw new Error('Language metadata unavailable');return value}
export function preferredLanguage(preferred:Language,available=descriptors):Language{
 if(!available.length)throw new Error('No usable language starters. Retry initialization.');
 return available.find(d=>d.id===preferred)?.id??available.find(d=>d.id==='cpp')?.id??available[0].id;
}
export function hasLanguage(value:unknown):value is Language{return descriptors.some(d=>d.id===value)}

export function starterLanguages(starters:unknown){
 if(!starters||typeof starters!=='object')throw new Error('Language starters are missing. Retry initialization.');
 const usable=descriptors.filter(d=>typeof (starters as Record<string,unknown>)[d.id]==='string');
 if(!usable.length)throw new Error('No usable language starters. Retry initialization.');
 return usable;
}
