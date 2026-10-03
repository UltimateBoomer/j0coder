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
  for(const dark of [false,true]){
   const background=dark?'#19191c':'#fafafa',foreground=dark?'#e4e4e7':'#242424';
   monaco.editor.defineTheme(dark?'j0coder-dark':'j0coder-light',{
    base:dark?'vs-dark':'vs',inherit:true,
    rules:[{token:'',foreground:foreground.slice(1)},{token:'comment',foreground:dark?'92929c':'686870'},{token:'keyword',foreground:dark?'b7b1d6':'655b88'},{token:'string',foreground:dark?'a6bea8':'486a4c'},{token:'number',foreground:dark?'d0b68e':'806039'}],
    colors:{'editor.background':background,'editor.foreground':foreground,'editorLineNumber.foreground':dark?'#777780':'#77777f','editorLineNumber.activeForeground':foreground,'editorCursor.foreground':foreground,'editor.selectionBackground':dark?'#444450':'#d4d4df','editor.inactiveSelectionBackground':dark?'#33333b':'#e2e2e8','editor.lineHighlightBackground':dark?'#222225':'#ededed','editorIndentGuide.background1':dark?'#303034':'#d8d8d8','editorSuggestWidget.background':background,'editorSuggestWidget.foreground':foreground,'editorSuggestWidget.border':dark?'#444448':'#bdbdbd','editorSuggestWidget.selectedBackground':dark?'#343438':'#dddddd','editorWidget.background':background,'editorWidget.foreground':foreground,'editorWidget.border':dark?'#444448':'#bdbdbd','editorError.foreground':dark?'#f5a69a':'#ae4939','editorWarning.foreground':dark?'#edcf83':'#916d12','editorInfo.foreground':dark?'#a1a1aa':'#626262','focusBorder':dark?'#a1a1aa':'#707070'}
   });
  }
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
