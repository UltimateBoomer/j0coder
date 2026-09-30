import * as monaco from 'monaco-editor';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker.js?worker';
import { MonacoVscodeApiWrapper } from 'monaco-languageclient/vscodeApiWrapper';
// C++20 keywords and alternative operator spellings.
const cppKeywords = [
 'alignas','alignof','and','and_eq','asm','auto','bitand','bitor','bool','break',
 'case','catch','char','char8_t','char16_t','char32_t','class','compl','concept',
 'const','const_cast','consteval','constexpr','constinit','continue','co_await',
 'co_return','co_yield','decltype','default','delete','do','double','dynamic_cast',
 'else','enum','explicit','export','extern','false','float','for','friend','goto',
 'if','import','inline','int','long','module','mutable','namespace','new',
 'noexcept','not','not_eq','nullptr','operator','or','or_eq','private',
 'protected','public','register',
 'reinterpret_cast','requires','return','short','signed','sizeof','static',
 'static_assert','static_cast','struct','switch','template','this','thread_local',
 'throw','true','try','typedef','typeid','typename','union','unsigned','using',
 'virtual','void','volatile','wchar_t','while','xor','xor_eq'
];
const services = new MonacoVscodeApiWrapper({
 $type:'classic', viewsConfig:{$type:'EditorService'},
 monacoWorkerFactory:()=>{(self as any).MonacoEnvironment={...(self as any).MonacoEnvironment,getWorker:()=>new EditorWorker()}},
 userConfiguration:{json:JSON.stringify({'editor.semanticHighlighting.enabled':true,'telemetry.telemetryLevel':'off'})}
});
let initialized:Promise<void>|undefined;
export function initialize(){
 return initialized??=(async()=>{
  await services.start();
  for(const language of ['cpp','python','java','kotlin']){
   monaco.languages.register({id:language,extensions:[({cpp:'.cpp',python:'.py',java:'.java',kotlin:'.kt'} as Record<string,string>)[language]]});
   const brackets:[string,string][]=[['{','}'],['[',']'],['(',')']];
   const quotes:[string,string][]=[['"','"'],["'","'"]];
   monaco.languages.setMonarchTokensProvider(language,{
    keywords:language==='cpp'?cppKeywords:language==='python'?['class','def','return','if','else','elif','for','while','in','not','and','or','True','False','None','pass','import','from']:language==='java'?['class','interface','public','private','static','void','int','long','double','boolean','new','return','if','else','for','while','null','true','false','import']:['class','fun','val','var','return','if','else','for','while','when','null','true','false','import','object'],
    tokenizer:{root:[[/\b[a-zA-Z_]\w*\b/,{cases:{'@keywords':'keyword','@default':'identifier'}}],[/"([^"\\]|\\.)*"/,'string'],[/'([^'\\]|\\.)*'/,'string'],[/\d+/,'number'],language==='python'?[/#.*$/,'comment']:[/\/\/.*$/,'comment']]}
   });
   monaco.languages.setLanguageConfiguration(language,{
    comments:{lineComment:language==='python'?'#':'//'},
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
