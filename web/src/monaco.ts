import {languages} from './languages';
import {syntaxProviders} from './syntax-providers';
import * as monaco from 'monaco-editor';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker.js?worker';
import { MonacoVscodeApiWrapper } from 'monaco-languageclient/vscodeApiWrapper';
const services = new MonacoVscodeApiWrapper({
 $type:'classic', viewsConfig:{$type:'EditorService'},
 monacoWorkerFactory:()=>{(self as any).MonacoEnvironment={...(self as any).MonacoEnvironment,getWorker:()=>new EditorWorker()}},
 userConfiguration:{json:JSON.stringify({'editor.semanticHighlighting.enabled':true,'telemetry.telemetryLevel':'off'})}
});
let initialized:Promise<void>|undefined;
export function initialize(){
 return initialized??=(async()=>{
  await services.start();
  for(const metadata of languages()){
   const language=metadata.id;
   if(!syntaxProviders[language])continue;
   const monacoLanguage=metadata.monaco_language;
   monaco.languages.register({id:monacoLanguage,extensions:[metadata.file_extension]});
   const brackets:[string,string][]=[['{','}'],['[',']'],['(',')']];
   const quotes:[string,string][]=[['"','"'],["'","'"]];
   monaco.languages.setMonarchTokensProvider(monacoLanguage,{
    keywords:syntaxProviders[language].keywords,
    tokenizer:{root:[[/\b[a-zA-Z_]\w*\b/,{cases:{'@keywords':'keyword','@default':'identifier'}}],[/"([^"\\]|\\.)*"/,'string'],[/'([^'\\]|\\.)*'/,'string'],[/\d+/,'number'],language==='python'?[/#.*$/,'comment']:[/\/\/.*$/,'comment']]}
   });
   monaco.languages.setLanguageConfiguration(monacoLanguage,{
    comments:{lineComment:metadata.line_comment},
    brackets,
    colorizedBracketPairs:brackets,
    autoClosingPairs:[...brackets,...quotes].map(([open,close])=>({open,close})),
    surroundingPairs:[...brackets,...quotes].map(([open,close])=>({open,close})),
    indentationRules:language!=='python'?{
     increaseIndentPattern:/\{[^}"']*$/,
     decreaseIndentPattern:/^\s*\}/
    }:{
     increaseIndentPattern:/:\s*(?:#.*)?$/,
     decreaseIndentPattern:/^\s*(?:elif|else|except|finally)\b.*:/
    }
   });
  }
 })();
}
export {monaco};
