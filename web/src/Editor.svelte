<script lang="ts">
 import {onMount} from 'svelte';import {api,type ProblemDetail,type User} from './api';import {SolutionSync,type SolutionLanguage} from './solution-sync';import DOMPurify from 'dompurify';import {marked} from 'marked';
 export let problem:ProblemDetail;export let user:User;export let theme:'light'|'dark';export let onerror:(e:string)=>void;
 let host:HTMLDivElement, language:SolutionLanguage='cpp',modelLanguage:SolutionLanguage='cpp',editor:any,monaco:any,semantic='Starting editor…',busy=false,switching=false,history:any[]=[],selected:any=null,alive=true,subscription:any;
 let workspace:HTMLDivElement,resultsSplit:HTMLDivElement;
 let statementWidth=43,resultsHeight=36,historyWidth=30,blindMode=localStorage.getItem('locoder:blind-mode')==='true';
 $: if(monaco)monaco.editor.setTheme(theme==='dark'?'vs-dark':'vs');
 let connection:import('./semantic').Connection|undefined,connectionGeneration=0,retryCount=0,retryTimer:ReturnType<typeof setTimeout>|undefined;
 const sync=new SolutionSync(user.id,problem.version,problem.starters);
 const statement=DOMPurify.sanitize(marked.parse(problem.problem.statement,{async:false}) as string);
 const stateful=()=>problem.problem.interface?.kind==='data_structure';
 const visibleTests=():any[]=>problem.problem.tests.filter((test:any)=>!test.hidden).map((test:any)=>stateful()?{...test,args:{constructor_args:test.constructor_args,operations:test.operations.map((o:any)=>({method:o.method,args:o.args}))},expected:test.operations.map((o:any)=>o.expected)}:test);
 function toggleBlindMode(){blindMode=!blindMode;localStorage.setItem('locoder:blind-mode',String(blindMode))}
 function resetEditor(){if(editor&&window.confirm(`Replace your ${language==='cpp'?'C++':'Python'} code with the starter code?`)){editor.setValue(problem.starters[language]);save();editor.focus()}}
 type Split='statement'|'results'|'history';
 function resize(split:Split,clientX:number,clientY:number){
  if(window.innerWidth<=800)return;
  const rect=(split==='history'?resultsSplit:workspace).getBoundingClientRect();
  if(split==='statement')statementWidth=Math.max(20,Math.min(70,(clientX-rect.left)/rect.width*100));
  else if(split==='history')historyWidth=Math.max(15,Math.min(65,(clientX-rect.left)/rect.width*100));
  else resultsHeight=Math.max(20,Math.min(65,(rect.bottom-clientY)/rect.height*100));
 }
 function startResize(event:PointerEvent,split:Split){
  if(window.innerWidth<=800)return;
  const handle=event.currentTarget as HTMLElement;
  handle.setPointerCapture(event.pointerId);
  const move=(moveEvent:PointerEvent)=>resize(split,moveEvent.clientX,moveEvent.clientY);
  const end=()=>{handle.removeEventListener('pointermove',move);handle.removeEventListener('pointerup',end);handle.removeEventListener('pointercancel',end)};
  handle.addEventListener('pointermove',move);handle.addEventListener('pointerup',end);handle.addEventListener('pointercancel',end);
  resize(split,event.clientX,event.clientY);
 }
 function nudgeResize(event:KeyboardEvent,split:Split){
  const delta=(event.key==='ArrowRight'||event.key==='ArrowUp')?2:(event.key==='ArrowLeft'||event.key==='ArrowDown')?-2:0;
  if(!delta)return;
  event.preventDefault();
  if(split==='statement')statementWidth=Math.max(20,Math.min(70,statementWidth+delta));
  else if(split==='history')historyWidth=Math.max(15,Math.min(65,historyWidth+delta));
  else resultsHeight=Math.max(20,Math.min(65,resultsHeight+delta));
 }
 function save(){if(editor)sync.capture(modelLanguage,editor.getValue())}
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
 async function change(next:SolutionLanguage){
  if(!editor||switching||next===language)return;
  switching=true;editor.updateOptions({readOnly:true});save();sync.flushAll();
  try{
   const source=await sync.load(next);
   if(!alive)return;
   stopSemantic();retryCount=0;
   const old=editor.getModel();
   const model=monaco.editor.createModel(source,next==='cpp'?'cpp':'python',monaco.Uri.parse(`file:///workspace/solution.${next==='cpp'?'cpp':'py'}`));
   modelLanguage=next;editor.setModel(model);old?.dispose();language=next;
   void connect();
  }finally{switching=false;editor?.updateOptions({readOnly:false})}
 }
 async function historyLoad(selectId?:string){const updated=await api(`/submissions?version=${problem.version}`);history=updated;if(selectId!==undefined)selected=updated.find((h:any)=>h.id===selectId)||selected;else if(selected===null)selected=updated[0]||null}
 async function waitForCompletion(id:string){busy=true;try{for(let count=0;alive&&count<4000;count++){const s=await api(`/submissions/${id}`);if(s.status==='completed'){await historyLoad(id);break}await new Promise(resolve=>setTimeout(resolve,1000))}}finally{busy=false}}
 onMount(()=>{alive=true;(async()=>{try{const [setup,source]=await Promise.all([import('./monaco'),sync.load('cpp')]);await setup.initialize();if(!alive)return;monaco=setup.monaco;monaco.editor.setTheme(theme==='dark'?'vs-dark':'vs');editor=monaco.editor.create(host,{value:source,language:'cpp',theme:theme==='dark'?'vs-dark':'vs',automaticLayout:true,minimap:{enabled:false},fontSize:14,padding:{top:18},scrollBeyondLastLine:false,autoClosingBrackets:'always',autoClosingQuotes:'always',autoIndent:'full',insertSpaces:true,tabSize:4,bracketPairColorization:{enabled:true},formatOnType:false,formatOnPaste:false});subscription=editor.onDidChangeModelContent(save);void connect();await historyLoad();if(selected&&selected.status!=='completed')await waitForCompletion(selected.id)}catch(e){onerror(String(e))}})();const flush=()=>{if(document.visibilityState==='hidden')sync.flushAll()};document.addEventListener('visibilitychange',flush);return()=>{alive=false;save();sync.dispose();document.removeEventListener('visibilitychange',flush);stopSemantic();subscription?.dispose();editor?.getModel()?.dispose();editor?.dispose()}});
 async function execute(){busy=true;try{save();const r=await api('/submissions','POST',{version:problem.version,language,source:editor.getValue(),mode:'submit'},crypto.randomUUID());await waitForCompletion(r.id)}catch(e){onerror(String(e))}finally{busy=false}}
</script>

<style>
 .actions{align-items:center}.run-spinner>span:first-child{display:block;width:17px;height:17px;border:2px solid #c7d4ca;border-top-color:#236644;border-radius:50%;animation:spin .75s linear infinite}@keyframes spin{to{transform:rotate(360deg)}}
 .results-split{height:var(--results-height);min-height:0;border-top:1px solid var(--border);display:grid;grid-template-columns:var(--history-width) 8px minmax(0,1fr);overflow:hidden}.history-panel,.result-panel{min-width:0;overflow:auto;padding:12px 16px}.history-panel{background:var(--panel-soft)}.history-panel h3{margin:0 0 10px}.history{display:grid;grid-template-columns:auto 1fr;gap:4px 10px;width:100%;font-size:12px;margin:6px 0;text-align:left;padding:10px}.history strong{text-align:right;text-transform:capitalize}.history small{grid-column:1/-1;color:var(--muted)}.history.selected{border-color:var(--accent);background:var(--selected);box-shadow:inset 3px 0 var(--accent)}.result-heading{display:flex;align-items:baseline;gap:12px}.result-heading h3{text-transform:capitalize;margin:0}.result-heading span{font-size:12px;color:var(--muted)}.case-strip{display:flex;gap:7px;overflow-x:auto;padding:13px 1px}.case-box{display:inline-grid;place-items:center;flex:0 0 28px;height:28px;border-radius:5px;font-weight:750}.case-box.pass{background:var(--success-bg);color:var(--success-text)}.case-box.fail{background:var(--fail-bg);color:var(--fail-text)}.case-box.limit{background:var(--warning-bg);color:var(--warning-text)}.case-box.not-run{background:var(--tag-bg);color:var(--muted)}
 .description-heading{display:flex;align-items:flex-start;justify-content:space-between;gap:10px}.description-heading h1{margin-bottom:12px}.blind-toggle{display:grid;place-items:center;flex:none;width:28px;height:28px;padding:4px;border:0;background:transparent;color:var(--muted);border-radius:5px}.blind-toggle:hover,.blind-toggle[aria-pressed="true"]{background:var(--tag-bg);color:var(--text)}.blind-toggle svg{width:17px;height:17px;stroke:currentColor;fill:none;stroke-width:1.8;stroke-linecap:round;stroke-linejoin:round}.problem-metadata{display:flex;align-items:center;gap:10px;flex-wrap:wrap}.splitter{flex:none;background:var(--border);position:relative;touch-action:none;padding:0;border:0;border-radius:0}.splitter:hover,.splitter:focus-visible{background:var(--accent);outline:none}.splitter.vertical{width:8px;cursor:col-resize}.splitter.horizontal{height:8px;cursor:row-resize}.splitter::after{content:'';position:absolute;inset:0;margin:auto;background:var(--muted);border-radius:4px;opacity:.7}.splitter.vertical::after{width:2px;height:28px}.splitter.horizontal::after{width:28px;height:2px}
 @media(max-width:800px){.results-split{height:45%;grid-template-columns:1fr;grid-template-rows:minmax(150px,35%) 1fr}.splitter{display:none}}
</style>

<div class="workspace" bind:this={workspace} style={`--statement-width:${statementWidth}%;--results-height:${resultsHeight}%;--history-width:${historyWidth}%`}><section class="statement-pane"><div class="description-heading"><h1>{problem.problem.title}</h1><button class="blind-toggle" aria-label={blindMode?'Show problem details':'Enable blind mode'} title={blindMode?'Show problem details':'Hide tags and difficulty'} aria-pressed={blindMode} onclick={toggleBlindMode}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6Z"/><circle cx="12" cy="12" r="2.5"/>{#if !blindMode}<path d="M3 21 21 3"/>{/if}</svg></button></div>{#if !blindMode}<div class="problem-metadata"><span class={`badge ${problem.problem.difficulty}`}>{problem.problem.difficulty}</span><div class="tags">{#each problem.problem.tags as tag}<span>{tag}</span>{/each}</div></div>{/if}<article class="statement">{@html statement}</article><h3>Examples</h3>{#each visibleTests() as t,i}<div class="example"><b>Example {i+1}</b><pre>Arguments: {display(t.args)}<br/>Expected:  {display(t.expected)}</pre></div>{/each}<p class="muted small">{problem.problem.limits.time_ms} ms · {problem.problem.limits.memory_mib} MiB per test</p></section><button type="button" class="splitter vertical" aria-label="Resize description and code" onpointerdown={(e)=>startResize(e,'statement')} onkeydown={(e)=>nudgeResize(e,'statement')}></button><section class="code-pane"><div class="editor-toolbar"><label class="sr-only" for="language">Language</label><select id="language" value={language} onchange={(e)=>{void change(e.currentTarget.value as SolutionLanguage)}} disabled={busy||switching}><option value="cpp">C++20</option><option value="python">Python 3</option></select><span>solution.{language==='cpp'?'cpp':'py'}</span><div class="actions"><button onclick={resetEditor} disabled={!editor||busy||switching}>Reset code</button>{#if busy}<span class="run-spinner" role="status"><span aria-hidden="true"></span><span class="sr-only">Run in progress</span></span>{/if}<button class="primary" onclick={execute} disabled={busy||switching||!editor}>Run</button></div></div><div class="monaco-host" bind:this={host}></div><div class="semantic" role="status">● {semantic}</div><button type="button" class="splitter horizontal" aria-label="Resize code and results" onpointerdown={(e)=>startResize(e,'results')} onkeydown={(e)=>nudgeResize(e,'results')}></button><div class="results-split" bind:this={resultsSplit}><aside class="history-panel" aria-label="Run history"><h3>History</h3>{#each history as h}<button class="history" class:selected={selected?.id===h.id} aria-pressed={selected?.id===h.id} onclick={()=>selected=h}><span>{h.language}</span><strong>{words(h.result?.verdict||h.status)}</strong><small>{new Date(h.created_at).toLocaleString()}</small></button>{:else}<p class="muted">No runs yet.</p>{/each}</aside><button type="button" class="splitter vertical" aria-label="Resize history and result details" onpointerdown={(e)=>startResize(e,'history')} onkeydown={(e)=>nudgeResize(e,'history')}></button><section class="result-panel" aria-label="Selected run results">{#if selected}<div class="result-heading"><h3 class:notice={selected.result?.verdict==='accepted'}>{words(selected.result?.verdict||selected.status)}</h3>{#if selected.result}<span>{selected.result.passed} / {selected.result.total} passed</span>{/if}</div>{#if selected.status!=='completed'}<p role="status">This run is {selected.status}.</p>{:else if selected.result}{#if selected.result.diagnostic}<pre>{selected.result.diagnostic}</pre>{/if}<div class="case-strip" aria-label="Test case results">{#each selected.result.cases||[] as current,i}<span class="case-box {caseClass(current.verdict)}" role="img" aria-label={caseLabel(selected.result.cases,i)} title={caseLabel(selected.result.cases,i)}>{caseSymbol(current.verdict)}</span>{/each}</div>{@const shownCases=(selected.result.cases||[]).filter((c:any)=>!c.hidden)}{#each visibleTests() as test,i}{@const current=shownCases[i]}<div class="case"><b>Visible case {i+1}: {words(current?.verdict)}</b><div class="comparison-cards"><div class="comparison-card"><span>Input</span><pre>{display(test.args)}</pre></div><div class="comparison-card"><span>Expected output</span><pre>{display(test.expected)}</pre></div><div class="comparison-card"><span>Current output</span><pre>{currentOutput(current)}</pre></div></div>{#if current?.log}<pre>{current.log}</pre>{/if}</div>{/each}<p class="muted small">Hidden cases show only their designation and verdict.</p>{/if}{:else}<p class="muted">Select Run to execute all tests.</p>{/if}</section></div></section></div>
