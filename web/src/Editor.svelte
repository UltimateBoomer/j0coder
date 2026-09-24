<script lang="ts">
 import {onMount} from 'svelte';import {api,type ProblemDetail,type User} from './api';import DOMPurify from 'dompurify';import {marked} from 'marked';
 export let problem:ProblemDetail;export let user:User;export let onerror:(e:string)=>void;
 let host:HTMLDivElement, language:'cpp'|'python'='cpp',editor:any,monaco:any,semantic='Starting editor…',busy=false,history:any[]=[],selected:any=null,alive=true,subscription:any;
 let connection:import('./semantic').Connection|undefined,connectionGeneration=0,retryCount=0,retryTimer:ReturnType<typeof setTimeout>|undefined;
 const key=()=>`practice:${user.id}:${problem.version}:${language}`;
 const statement=DOMPurify.sanitize(marked.parse(problem.problem.statement,{async:false}) as string);
 const stateful=()=>problem.problem.interface?.kind==='data_structure';
 const visibleTests=():any[]=>problem.problem.tests.filter((test:any)=>!test.hidden).map((test:any)=>stateful()?{...test,args:{constructor_args:test.constructor_args,operations:test.operations.map((o:any)=>({method:o.method,args:o.args}))},expected:test.operations.map((o:any)=>o.expected)}:test);
 const interfaceGuide=()=>stateful()?`Stateful class ${problem.problem.interface?.name}; each test uses one fresh instance`:`Top-level function ${problem.problem.interface?.name}`;
 function save(){if(editor)localStorage.setItem(key(),editor.getValue())}
 function display(value:unknown,missing='Not provided'){return value===undefined?missing:JSON.stringify(value)}
 function currentOutput(test:any){return test?test.output===null||test.output===undefined?'No output':display(test.output):'Not executed'}
 function words(value:string|null|undefined){return value?.replaceAll('_',' ')||'not run'}
 function caseNumber(cases:any[],index:number,hidden:boolean){return cases.slice(0,index+1).filter(c=>Boolean(c.hidden)===hidden).length}
 function caseLabel(cases:any[],index:number){const item=cases[index];return `${item.hidden?'Hidden':'Visible'} case ${caseNumber(cases,index,Boolean(item.hidden))}: ${words(item.verdict)}`}
 function caseClass(verdict:string|null){if(verdict==='accepted')return 'pass';if(['time_limit','memory_limit','output_limit'].includes(verdict||''))return 'limit';if(verdict===null||verdict===undefined)return 'not-run';return 'fail'}
 function caseSymbol(verdict:string|null){if(verdict==='accepted')return '✓';if(['time_limit','memory_limit','output_limit'].includes(verdict||''))return '⚠';if(verdict===null||verdict===undefined)return '–';return '×'}
 function stopSemantic(){
  connectionGeneration++;
  if(retryTimer)clearTimeout(retryTimer);
  retryTimer=undefined;
  connection?.dispose();connection=undefined;
 }
 function retrySemantic(generation:number){
  if(!alive||generation!==connectionGeneration)return;
  stopSemantic();
  const delay=Math.min(10000,250*2**Math.min(retryCount++,6))*(0.75+Math.random()*0.5);
  semantic='Basic completion · reconnecting semantic service…';
  retryTimer=setTimeout(()=>{retryTimer=undefined;void connect()},delay);
 }
 async function connect(){
  stopSemantic();
  if(!alive||!editor)return;
  const generation=connectionGeneration,currentLanguage=language;
  semantic='Connecting semantic completion…';
  try{
   const lsp=await import('./semantic');
   if(generation!==connectionGeneration||!alive)return;
   const current=lsp.connect(currentLanguage,()=>retrySemantic(generation));
   connection=current;
   await current.ready;
   if(generation!==connectionGeneration||!alive)return;
   retryCount=0;
   semantic='Semantic completion connected';
  }catch{retrySemantic(generation)}
 }
 async function change(){stopSemantic();retryCount=0;const old=editor.getModel();const model=monaco.editor.createModel(localStorage.getItem(key())||problem.starters[language],language==='cpp'?'cpp':'python',monaco.Uri.parse(`file:///workspace/solution.${language==='cpp'?'cpp':'py'}`));editor.setModel(model);old?.dispose();await connect()}
 async function historyLoad(selectId?:string){const updated=await api(`/submissions?version=${problem.version}`);history=updated;if(selectId!==undefined)selected=updated.find((h:any)=>h.id===selectId)||selected;else if(selected===null)selected=updated[0]||null}
 async function waitForCompletion(id:string){busy=true;try{for(let count=0;alive&&count<4000;count++){const s=await api(`/submissions/${id}`);if(s.status==='completed'){await historyLoad(id);break}await new Promise(resolve=>setTimeout(resolve,1000))}}finally{busy=false}}
 onMount(()=>{alive=true;(async()=>{try{const setup=await import('./monaco');await setup.initialize();if(!alive)return;monaco=setup.monaco;editor=monaco.editor.create(host,{value:localStorage.getItem(key())||problem.starters[language],language:'cpp',theme:'vs-dark',automaticLayout:true,minimap:{enabled:false},fontSize:14,padding:{top:18},scrollBeyondLastLine:false,autoClosingBrackets:'always',autoClosingQuotes:'always',autoIndent:'full',insertSpaces:true,tabSize:4,bracketPairColorization:{enabled:true},formatOnType:false,formatOnPaste:false});subscription=editor.onDidChangeModelContent(save);await change();await historyLoad();if(selected&&selected.status!=='completed')await waitForCompletion(selected.id)}catch(e){onerror(String(e))}})();return()=>{alive=false;save();stopSemantic();subscription?.dispose();editor?.getModel()?.dispose();editor?.dispose()}});
 async function execute(){busy=true;try{save();const r=await api('/submissions','POST',{version:problem.version,language,source:editor.getValue(),mode:'submit'},crypto.randomUUID());await waitForCompletion(r.id)}catch(e){onerror(String(e))}finally{busy=false}}
</script>

<style>
 .actions{align-items:center}.run-spinner>span:first-child{display:block;width:17px;height:17px;border:2px solid #c7d4ca;border-top-color:#236644;border-radius:50%;animation:spin .75s linear infinite}@keyframes spin{to{transform:rotate(360deg)}}
 .results-split{height:36%;min-height:220px;border-top:1px solid #dce3db;display:grid;grid-template-columns:minmax(180px,30%) 1fr;overflow:hidden}.history-panel,.result-panel{min-width:0;overflow:auto;padding:12px 16px}.history-panel{border-right:1px solid #dce3db;background:#fafbf9}.history-panel h3{margin:0 0 10px}.history{display:grid;grid-template-columns:auto 1fr;gap:4px 10px;width:100%;font-size:12px;margin:6px 0;text-align:left;padding:10px}.history strong{text-align:right;text-transform:capitalize}.history small{grid-column:1/-1;color:#7b877e}.history.selected{border-color:#468962;background:#eaf4ed;box-shadow:inset 3px 0 #236644}.result-heading{display:flex;align-items:baseline;gap:12px}.result-heading h3{text-transform:capitalize;margin:0}.result-heading span{font-size:12px;color:#637269}.case-strip{display:flex;gap:7px;overflow-x:auto;padding:13px 1px}.case-box{display:inline-grid;place-items:center;flex:0 0 28px;height:28px;border-radius:5px;font-weight:750}.case-box.pass{background:#daf1e1;color:#267447}.case-box.fail{background:#f8dfda;color:#ae4939}.case-box.limit{background:#fff0c7;color:#916d12}.case-box.not-run{background:#e9ece9;color:#778078}
 @media(max-width:800px){.results-split{height:45%;grid-template-columns:1fr;grid-template-rows:minmax(150px,35%) 1fr}.history-panel{border-right:0;border-bottom:1px solid #dce3db}}
</style>

<div class="workspace"><section class="statement-pane"><h1>{problem.problem.title}</h1><div class="tags">{#each problem.problem.tags as tag}<span>{tag}</span>{/each}</div><article class="statement">{@html statement}</article><h3>Examples</h3>{#each visibleTests() as t,i}<div class="example"><b>Example {i+1}</b><pre>Arguments: {display(t.args)}<br/>Expected:  {display(t.expected)}</pre></div>{/each}<p class="muted small">{problem.problem.limits.time_ms} ms · {problem.problem.limits.memory_mib} MiB per test<br/>Standard libraries · {interfaceGuide()}</p></section><section class="code-pane"><div class="editor-toolbar"><label class="sr-only" for="language">Language</label><select id="language" value={language} onchange={async(e)=>{save();language=e.currentTarget.value as 'cpp'|'python';await change()}} disabled={busy}><option value="cpp">C++20</option><option value="python">Python 3</option></select><span>solution.{language==='cpp'?'cpp':'py'}</span><div class="actions">{#if busy}<span class="run-spinner" role="status"><span aria-hidden="true"></span><span class="sr-only">Run in progress</span></span>{/if}<button class="primary" onclick={execute} disabled={busy||!editor}>Run</button></div></div><div class="monaco-host" bind:this={host}></div><div class="semantic" role="status">● {semantic}<button onclick={connect}>Reconnect</button></div><div class="results-split"><aside class="history-panel" aria-label="Run history"><h3>History</h3>{#each history as h}<button class="history" class:selected={selected?.id===h.id} aria-pressed={selected?.id===h.id} onclick={()=>selected=h}><span>{h.language}</span><strong>{words(h.result?.verdict||h.status)}</strong><small>{new Date(h.created_at).toLocaleString()}</small></button>{:else}<p class="muted">No runs yet.</p>{/each}</aside><section class="result-panel" aria-label="Selected run results">{#if selected}<div class="result-heading"><h3 class:notice={selected.result?.verdict==='accepted'}>{words(selected.result?.verdict||selected.status)}</h3>{#if selected.result}<span>{selected.result.passed} / {selected.result.total} passed</span>{/if}</div>{#if selected.status!=='completed'}<p role="status">This run is {selected.status}.</p>{:else if selected.result}{#if selected.result.diagnostic}<pre>{selected.result.diagnostic}</pre>{/if}<div class="case-strip" aria-label="Test case results">{#each selected.result.cases||[] as current,i}<span class="case-box {caseClass(current.verdict)}" role="img" aria-label={caseLabel(selected.result.cases,i)} title={caseLabel(selected.result.cases,i)}>{caseSymbol(current.verdict)}</span>{/each}</div>{@const shownCases=(selected.result.cases||[]).filter((c:any)=>!c.hidden)}{#each visibleTests() as test,i}{@const current=shownCases[i]}<div class="case"><b>Visible case {i+1}: {words(current?.verdict)}</b><div class="comparison-cards"><div class="comparison-card"><span>Input</span><pre>{display(test.args)}</pre></div><div class="comparison-card"><span>Expected output</span><pre>{display(test.expected)}</pre></div><div class="comparison-card"><span>Current output</span><pre>{currentOutput(current)}</pre></div></div>{#if current?.log}<pre>{current.log}</pre>{/if}</div>{/each}<p class="muted small">Hidden cases show only their designation and verdict.</p>{/if}{:else}<p class="muted">Select Run to execute all tests.</p>{/if}</section></div></section></div>
