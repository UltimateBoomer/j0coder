import {api,ApiError} from './api';

export type SolutionLanguage=import('./languages').Language;
export type SaveStatus='loading'|'pending'|'saving'|'saved'|'retrying'|'unavailable';
type SaveState={timer?:ReturnType<typeof setTimeout>;inFlight?:Promise<boolean>;retryMs:number};

export class SolutionSync {
 private states:Record<SolutionLanguage,SaveState>;
 private statuses:Record<SolutionLanguage,SaveStatus>;
 private disposed=false;

 constructor(private userId:string,private version:string,private starters:Partial<Record<SolutionLanguage,string>>,private onStatus?:(language:SolutionLanguage,status:SaveStatus)=>void){
  this.states=Object.fromEntries(Object.keys(starters).map(id=>[id,{retryMs:1000}])) as Record<SolutionLanguage,SaveState>;
  this.statuses=Object.fromEntries(Object.keys(starters).map(id=>[id,'loading'])) as Record<SolutionLanguage,SaveStatus>;
 }

 private setStatus(language:SolutionLanguage,status:SaveStatus){
  if(this.statuses[language]===status)return;
  this.statuses[language]=status;
  this.onStatus?.(language,status);
 }

 private key(language:SolutionLanguage){return `practice:${this.userId}:${this.version}:${language}`}
 private pendingKey(language:SolutionLanguage){return `${this.key(language)}:unsent`}
 private path(language:SolutionLanguage){return `/solutions/${this.version}/${language}`}

 async load(language:SolutionLanguage):Promise<string>{
  this.setStatus(language,'loading');
  // Finish older writes before comparing the browser cache with the server.
  await this.flush(language);
  try{
   const cloud=await api(this.path(language));
   const local=localStorage.getItem(this.key(language));
   const pending=localStorage.getItem(this.pendingKey(language))==='1';
   if(pending&&local!==null){
    this.setStatus(language,'pending');
    this.schedule(language,0);
    return local;
   }
   localStorage.setItem(this.key(language),cloud.source);
   localStorage.removeItem(this.pendingKey(language));
   this.setStatus(language,'saved');
   return cloud.source;
  }catch(error){
   const local=localStorage.getItem(this.key(language));
   const pending=localStorage.getItem(this.pendingKey(language))==='1';
   if(error instanceof ApiError&&error.status===404){
    if(local!==null&&local!==this.starters[language])this.markDirty(language,local,0);
    else this.setStatus(language,'saved');
   }else if(pending&&local!==null){
    this.setStatus(language,'retrying');
    this.schedule(language,this.states[language].retryMs);
   }else this.setStatus(language,'unavailable');
   return local??this.starters[language]!;
  }
 }

 markDirty(language:SolutionLanguage,source:string,delay=800){
  localStorage.setItem(this.key(language),source);
  localStorage.setItem(this.pendingKey(language),'1');
  this.setStatus(language,'pending');
  this.schedule(language,delay);
 }

 capture(language:SolutionLanguage,source:string){
  const cached=localStorage.getItem(this.key(language));
  if(cached===null&&source===this.starters[language])return;
  if(cached!==source)this.markDirty(language,source);
 }

 private schedule(language:SolutionLanguage,delay:number){
  if(this.disposed)return;
  const state=this.states[language];
  if(state.timer)clearTimeout(state.timer);
  state.timer=setTimeout(()=>{state.timer=undefined;void this.flush(language)},delay);
 }

 async flush(language:SolutionLanguage):Promise<void>{
  const state=this.states[language];
  if(state.timer)clearTimeout(state.timer);
  state.timer=undefined;
  if(state.inFlight){
   const succeeded=await state.inFlight;
   if(succeeded&&localStorage.getItem(this.pendingKey(language))==='1')await this.flush(language);
   return;
  }
  if(localStorage.getItem(this.pendingKey(language))!=='1')return;
  const source=localStorage.getItem(this.key(language));
  if(source===null)return;
  this.setStatus(language,'saving');
  state.inFlight=(async()=>{
   try{
    await api(this.path(language),'PUT',{source});
    state.retryMs=1000;
    if(localStorage.getItem(this.key(language))===source)localStorage.removeItem(this.pendingKey(language));
    return true;
   }catch(error){
    state.retryMs=Math.max(error instanceof ApiError?error.retryAfter:0,Math.min(state.retryMs*2,30000));
    return false;
   }
  })();
  const succeeded=await state.inFlight;
  state.inFlight=undefined;
  if(localStorage.getItem(this.pendingKey(language))==='1'){
   this.setStatus(language,succeeded?'pending':'retrying');
   if(this.disposed)return;
   this.schedule(language,succeeded?0:state.retryMs);
  }else this.setStatus(language,'saved');
 }

 flushAll(){for(const language of Object.keys(this.starters) as SolutionLanguage[])void this.flush(language)}
 dispose(){this.disposed=true;for(const language of Object.keys(this.starters) as SolutionLanguage[]){const timer=this.states[language].timer;if(timer)clearTimeout(timer)}this.flushAll()}
}
