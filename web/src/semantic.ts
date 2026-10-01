import {MonacoLanguageClient} from 'monaco-languageclient';
import {WebSocketMessageReader,WebSocketMessageWriter,toSocket} from 'vscode-ws-jsonrpc';
import {CloseAction,ErrorAction} from 'vscode-languageclient/browser';
import {api} from './api';

export type Connection = {ready:Promise<void>,dispose:()=>void};

function waitForOpen(ws:WebSocket):Promise<void>{
 return new Promise((resolve,reject)=>{
  const finish=(error?:Error)=>{
   clearTimeout(timer);
   ws.removeEventListener('open',opened);
   ws.removeEventListener('error',failed);
   ws.removeEventListener('close',closed);
   if(error)reject(error);else resolve();
  };
  const opened=()=>finish();
  const failed=()=>finish(new Error('Semantic service unavailable'));
  const closed=()=>finish(new Error('Semantic connection closed'));
  const timer=setTimeout(()=>{ws.close();finish(new Error('Connection timed out'))},10000);
  ws.addEventListener('open',opened,{once:true});
  ws.addEventListener('error',failed,{once:true});
  ws.addEventListener('close',closed,{once:true});
 });
}

export function connect(language:string,onClosed:()=>void):Connection{
 const abort=new AbortController();
 let disposed=false,ws:WebSocket|undefined,client:MonacoLanguageClient|undefined;
 let reader:WebSocketMessageReader|undefined,writer:WebSocketMessageWriter|undefined;
 const dispose=()=>{
  if(disposed)return;
  disposed=true;
  abort.abort();
  ws?.close();
  if(client)void client.stop().catch(()=>{});
  reader?.dispose();writer?.dispose();
 };
 const ready=(async()=>{
  try{
   const ticket=await api('/editor-ticket','POST',{language},undefined,abort.signal);
   if(disposed)return;
   ws=new WebSocket(`${location.protocol==='https:'?'wss':'ws'}://${location.host}${ticket.path}?ticket=${encodeURIComponent(ticket.ticket)}`);
   ws.addEventListener('close',()=>{if(!disposed)onClosed()});
   await waitForOpen(ws);
   if(disposed)return;
   const socket=toSocket(ws);reader=new WebSocketMessageReader(socket);writer=new WebSocketMessageWriter(socket);
   // Keep initialization within the editor service's fixed workspace contract.
   const rawWrite=writer.write.bind(writer);
   writer.write=(message:any)=>{
    if(message.method==='initialize'){
     delete message.params.rootPath;delete message.params.initializationOptions;
     message.params.rootUri='file:///workspace';message.params.workspaceFolders=null;
    }
    return rawWrite(message);
   };
   client=new MonacoLanguageClient({name:`j0coder ${language}`,clientOptions:{documentSelector:[language],errorHandler:{error:()=>({action:ErrorAction.Continue}),closed:()=>({action:CloseAction.DoNotRestart})}},messageTransports:{reader,writer}});
   let timer:ReturnType<typeof setTimeout>;
   const timeout=new Promise<never>((_,reject)=>{
    timer=setTimeout(()=>{ws?.close();reject(new Error('Semantic initialization timed out'))},105000);
   });
   try{await Promise.race([client.start(),timeout])}finally{clearTimeout(timer!)}
   if(disposed)dispose();
  }catch(error){dispose();throw error}
 })();
 return {ready,dispose};
}
