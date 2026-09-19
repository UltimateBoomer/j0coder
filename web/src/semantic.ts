import {MonacoLanguageClient} from 'monaco-languageclient';
import {WebSocketMessageReader,WebSocketMessageWriter,toSocket} from 'vscode-ws-jsonrpc';
import {CloseAction,ErrorAction} from 'vscode-languageclient/browser';
import {api} from './api';
export async function connect(_monaco:any,_editor:any,language:string,status:(s:string)=>void):Promise<()=>void>{
 const ticket=await api('/editor-ticket','POST',{language});
 const ws=new WebSocket(`${location.protocol==='https:'?'wss':'ws'}://${location.host}${ticket.path}?ticket=${encodeURIComponent(ticket.ticket)}`);
 await new Promise<void>((resolve,reject)=>{const timer=setTimeout(()=>{ws.close();reject(new Error('Connection timed out'))},10000);ws.onopen=()=>{clearTimeout(timer);resolve()};ws.onerror=()=>{clearTimeout(timer);reject(new Error('Semantic service unavailable'))}});
 const socket=toSocket(ws);const reader=new WebSocketMessageReader(socket);const writer=new WebSocketMessageWriter(socket);
 // Restrict initialize fields to the server's fixed workspace contract.
 const rawWrite=writer.write.bind(writer);writer.write=(message:any)=>{if(message.method==='initialize'){delete message.params.rootPath;delete message.params.initializationOptions;message.params.rootUri='file:///workspace';message.params.workspaceFolders=null}return rawWrite(message)};
 const client=new MonacoLanguageClient({name:`Practice ${language}`,clientOptions:{documentSelector:[language],errorHandler:{error:()=>({action:ErrorAction.Continue}),closed:()=>({action:CloseAction.DoNotRestart})}},messageTransports:{reader,writer}});
 ws.onclose=()=>status('Basic completion · disconnected; reconnect when ready');
 await client.start();status('Semantic completion connected');
 return ()=>{void client.stop().finally(()=>ws.close());reader.dispose();writer.dispose()};
}
