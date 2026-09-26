import {api,ApiError} from './api';

export type SolutionLanguage='cpp'|'python';
type SaveState={timer?:ReturnType<typeof setTimeout>;inFlight?:Promise<boolean>;retryMs:number};

export class SolutionSync {
 private states:Record<SolutionLanguage,SaveState>={cpp:{retryMs:1000},python:{retryMs:1000}};
 private disposed=false;

 constructor(private userId:string,private version:string,private starters:Record<SolutionLanguage,string>){}

 private key(language:SolutionLanguage){return `practice:${this.userId}:${this.version}:${language}`}
 private pendingKey(language:SolutionLanguage){return `${this.key(language)}:unsent`}
 private path(language:SolutionLanguage){return `/solutions/${this.version}/${language}`}

 async load(language:SolutionLanguage):Promise<string>{
  // Finish older writes before comparing the browser cache with the server.
  await this.flush(language);
  try{
   const cloud=await api(this.path(language));
   const local=localStorage.getItem(this.key(language));
   const pending=localStorage.getItem(this.pendingKey(language))==='1';
   if(pending&&local!==null){
    this.schedule(language,0);
    return local;
   }
   localStorage.setItem(this.key(language),cloud.source);
   localStorage.removeItem(this.pendingKey(language));
   return cloud.source;
  }catch(error){
   const local=localStorage.getItem(this.key(language));
   const pending=localStorage.getItem(this.pendingKey(language))==='1';
   if(error instanceof ApiError&&error.status===404){
    if(local!==null&&local!==this.starters[language])this.markDirty(language,local,0);
   }else if(pending&&local!==null){
    this.schedule(language,this.states[language].retryMs);
   }
   return local??this.starters[language];
  }
 }

 markDirty(language:SolutionLanguage,source:string,delay=800){
  localStorage.setItem(this.key(language),source);
  localStorage.setItem(this.pendingKey(language),'1');
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
  state.inFlight=(async()=>{
   try{
    await api(this.path(language),'PUT',{source});
    state.retryMs=1000;
    if(localStorage.getItem(this.key(language))===source)localStorage.removeItem(this.pendingKey(language));
    return true;
   }catch{
    state.retryMs=Math.min(state.retryMs*2,30000);
    return false;
   }
  })();
  const succeeded=await state.inFlight;
  state.inFlight=undefined;
  if(localStorage.getItem(this.pendingKey(language))==='1'){
   if(this.disposed)return;
   this.schedule(language,succeeded?0:state.retryMs);
  }
 }

 flushAll(){void this.flush('cpp');void this.flush('python')}
 dispose(){this.disposed=true;for(const language of ['cpp','python'] as const){const timer=this.states[language].timer;if(timer)clearTimeout(timer)}this.flushAll()}
}
