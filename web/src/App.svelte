<script lang="ts">
 import {onMount} from 'svelte';
 import {api,ApiError,setCsrf,type ProblemSummary} from './api';
 import {parseRoute,problemListUrl,push,replace,type Route} from './router';
 import Fuse from 'fuse.js';
 import DOMPurify from 'dompurify';import {marked} from 'marked';
 let user:any=null,loading=true,routeLoading=false,error='',username='',password='',allProblems:ProblemSummary[]=[],problems:ProblemSummary[]=[],active:any=null;
 let route:Route=parseRoute(window.location),query='',difficulty='',tag='',Editor:any=null;
 let drafts:any[]=[],draftId='',draftText='',preview:any=null,notice='',newUsername='',newPassword='',loadSequence=0;
 let catalog:any=null,catalogText='';
 const rowHeight=88,overscan=5;
 const quickRowHeight=72;
 const searchOptions={keys:[{name:'title',weight:0.65},{name:'summary',weight:0.2},{name:'tags',weight:0.15}],threshold:0.3,ignoreLocation:true,isCaseSensitive:false};
 let problemIndex:Fuse<ProblemSummary>|null=null,catalogLoaded=false,catalogLoading=false,catalogError='',catalogGeneration=0;
 let catalogRequest:Promise<ProblemSummary[]>|null=null;
 let quickQuery='',quickOpen=false,quickActiveIndex=0,quickScrollTop=0,quickViewportHeight=0;
 let quickRoot:HTMLElement|null=null,quickInput:HTMLInputElement|null=null,quickScrollElement:HTMLElement|null=null;
 let listScrollElement:HTMLElement|null=null,listScrollTop=0,listViewportHeight=0;
 function measureList(node:HTMLElement){
  const observer=new ResizeObserver(()=>listViewportHeight=node.clientHeight);
  observer.observe(node);
  listViewportHeight=node.clientHeight;
  return {destroy(){observer.disconnect()}};
 }
 function resetListScroll(){listScrollTop=0;if(listScrollElement)listScrollElement.scrollTop=0}
 function measureQuickList(node:HTMLElement){
  const observer=new ResizeObserver(()=>quickViewportHeight=node.clientHeight);
  observer.observe(node);
  quickViewportHeight=node.clientHeight;
  return {destroy(){observer.disconnect()}};
 }
 function resetQuickScroll(){quickScrollTop=0;if(quickScrollElement)quickScrollElement.scrollTop=0}
 type ThemeChoice='system'|'light'|'dark';
 const savedTheme=localStorage.getItem('locoder:theme');
 let themeChoice:ThemeChoice=savedTheme==='light'||savedTheme==='dark'||savedTheme==='system'?savedTheme:'system';
 let systemDark=window.matchMedia('(prefers-color-scheme: dark)').matches;
 $: resolvedTheme=themeChoice==='system'?(systemDark?'dark':'light'):themeChoice;
 $: if(typeof document!=='undefined')document.documentElement.dataset.theme=resolvedTheme;
 function chooseTheme(value:ThemeChoice){themeChoice=value;localStorage.setItem('locoder:theme',value)}
 const template={schema:3,title:'New problem',statement:'# New problem\n\nDescribe the task.',difficulty:'easy',tags:['arrays'],interface:{kind:'function',name:'solve',params:[{name:'values',ty:{array:'int'}}],returns:'int'},limits:{time_ms:2000,memory_mib:256,output_bytes:1048576},tests:[{args:[[1,2]],expected:3,hidden:false},{args:[[]],expected:0,hidden:true}]};
 const markdown=(s:string)=>DOMPurify.sanitize(marked.parse(s,{async:false}) as string);
 const routeTitle=()=>route.kind==='problem'&&active?`${active.problem.title} · Locoder`:route.kind.startsWith('admin')?'Authoring · Locoder':route.kind==='access-denied'?'Access denied · Locoder':route.kind==='not-found'?'Not found · Locoder':'Problems · Locoder';
 async function fetchList():Promise<ProblemSummary[]>{
  const items:ProblemSummary[]=[];
  let cursor='';
  while(true){
   const params=new URLSearchParams({limit:'100'});
   if(cursor)params.set('cursor',cursor);
   const page=await api(`/problems?${params}`) as (ProblemSummary&{cursor:string})[];
   items.push(...page);
   if(page.length<100)break;
   const nextCursor=page[page.length-1].cursor;
   if(!nextCursor||nextCursor===cursor)throw new Error('Problem list pagination did not advance');
   cursor=nextCursor;
  }
  return items;
 }
 function loadCatalog(refresh=false):Promise<ProblemSummary[]>{
  if(!refresh&&catalogRequest)return catalogRequest;
  if(!refresh&&catalogLoaded)return Promise.resolve(allProblems);
  const generation=++catalogGeneration;
  catalogLoading=true;catalogError='';
  const request=fetchList().then(items=>{
   if(generation===catalogGeneration){allProblems=items;problemIndex=new Fuse(items,searchOptions);catalogLoaded=true}
   return items;
  }).catch(e=>{if(generation===catalogGeneration)catalogError=String(e);throw e}).finally(()=>{
   if(generation===catalogGeneration){catalogLoading=false;catalogRequest=null}
  });
  catalogRequest=request;
  return request;
 }
 function clearCatalog(){catalogGeneration++;catalogRequest=null;catalogLoading=false;catalogLoaded=false;catalogError='';allProblems=[];problemIndex=null;closeQuickSearch()}
 function matches(q:string):ProblemSummary[]{return q.trim()&&problemIndex?problemIndex.search(q.trim()).map(result=>result.item):[]}
 function filterProblems(items:ProblemSummary[],q:string,difficultyFilter:string,tagFilter:string){
  const normalizedTag=tagFilter.trim().toLocaleLowerCase();
  const ranked=q.trim()?matches(q):items;
  return ranked.filter(p=>(!difficultyFilter||p.difficulty===difficultyFilter)&&(!normalizedTag||p.tags.some(t=>t.toLocaleLowerCase()===normalizedTag)));
 }
 $: problems=filterProblems(allProblems,query,difficulty,tag);
 $: quickResults=quickQuery.trim()&&problemIndex?problemIndex.search(quickQuery.trim()).map(result=>result.item):[];
 $: quickFirst=Math.max(0,Math.min(quickResults.length,Math.floor(quickScrollTop/quickRowHeight)-overscan));
 $: quickLast=Math.min(quickResults.length,quickFirst+Math.ceil(quickViewportHeight/quickRowHeight)+overscan*2);
 $: firstRow=Math.max(0,Math.min(problems.length,Math.floor(listScrollTop/rowHeight)-overscan));
 $: lastRow=Math.min(problems.length,firstRow+Math.ceil(listViewportHeight/rowHeight)+overscan*2);
 $: if(!loading&&!routeLoading&&route.kind==='problems')replace(problemListUrl({q:query,difficulty,tag}));
 function dismissQuickSearch(){resetQuickScroll();quickActiveIndex=0;quickOpen=false}
 function closeQuickSearch(){dismissQuickSearch();quickQuery=''}
 function retryQuickSearch(){void loadCatalog(true).catch(()=>{});quickInput?.focus()}
 function updateQuickSearch(value:string){quickQuery=value;quickOpen=!!value.trim();quickActiveIndex=0;resetQuickScroll();if(quickOpen)void loadCatalog().catch(()=>{})}
 function keepQuickActiveVisible(){
  if(!quickScrollElement)return;
  const top=quickActiveIndex*quickRowHeight,bottom=top+quickRowHeight;
  if(top<quickScrollElement.scrollTop)quickScrollElement.scrollTop=top;
  else if(bottom>quickScrollElement.scrollTop+quickScrollElement.clientHeight)quickScrollElement.scrollTop=bottom-quickScrollElement.clientHeight;
 }
 function onQuickKeydown(event:KeyboardEvent){
  if(event.key==='Escape'){event.preventDefault();dismissQuickSearch();return}
  if(!quickOpen&&event.key==='ArrowDown'&&quickQuery.trim()){event.preventDefault();quickOpen=true;void loadCatalog().catch(()=>{});return}
  if(!quickOpen||!quickResults.length)return;
  if(event.key==='ArrowDown'||event.key==='ArrowUp'){
   event.preventDefault();quickActiveIndex=Math.max(0,Math.min(quickResults.length-1,quickActiveIndex+(event.key==='ArrowDown'?1:-1)));
   keepQuickActiveVisible();
  }else if(event.key==='Enter'){event.preventDefault();openQuickProblem(quickResults[quickActiveIndex])}
 }
 function openQuickProblem(problem:ProblemSummary){closeQuickSearch();go(`/problems/${problem.id}`)}
 function edit(d:any){draftId=d?.id||'';draftText=JSON.stringify(d?.draft||template,null,2);preview=null;notice=''}
 async function resolveRoute(){
  const current=parseRoute(window.location),sequence=++loadSequence;routeLoading=true;error='';
  let next:Route=current,nextActive:any=null,nextDrafts:any[]=[],selected:any=null;
  try{
   if(current.kind==='problems'){query=current.filters.q;difficulty=current.filters.difficulty;tag=current.filters.tag;await loadCatalog()}
   else if(current.kind==='problem'){nextActive=await api(`/problems/${current.problemId}`);Editor=Editor||(await import('./Editor.svelte')).default}
   else if(current.kind==='admin'||current.kind==='admin-new'||current.kind==='admin-edit'){
    if(!user.admin)next={kind:'access-denied'};
    else{[nextDrafts,catalog]=await Promise.all([api('/admin/problems'),api('/admin/catalog')]);catalogText=catalog.settings?JSON.stringify(catalog.settings,null,2):'';if(current.kind==='admin-edit'){selected=nextDrafts.find(d=>d.id===current.problemId);if(!selected)next={kind:'not-found'}}}
   }
  }catch(e){if(e instanceof ApiError&&(e.status===400||e.status===404))next={kind:'not-found'};else if(e instanceof ApiError&&e.status===403)next={kind:'access-denied'};else if(e instanceof ApiError&&e.status===401){user=null;setCsrf('')}else error=String(e)}
  if(sequence!==loadSequence)return;
  route=next;active=nextActive;drafts=nextDrafts;
  if(next.kind==='problems')resetListScroll();
  if(next.kind==='admin-new')edit(null);else if(next.kind==='admin-edit'&&selected)edit(selected);else if(next.kind==='admin'){draftId='';draftText='';preview=null;notice=''}
  routeLoading=false;
 }
 async function init(){try{user=await api('/session');setCsrf(user.csrf);await resolveRoute()}catch(e){if(e instanceof ApiError&&e.status===401)user=null;else error=String(e)}finally{loading=false}}
 onMount(()=>{const media=window.matchMedia('(prefers-color-scheme: dark)');const updateSystem=()=>systemDark=media.matches;const outside=(event:PointerEvent)=>{if(quickRoot&&!quickRoot.contains(event.target as Node))dismissQuickSearch()};media.addEventListener('change',updateSystem);document.addEventListener('pointerdown',outside);const pop=()=>{dismissQuickSearch();if(user)resolveRoute();else route=parseRoute(window.location)};window.addEventListener('popstate',pop);init();return()=>{window.removeEventListener('popstate',pop);media.removeEventListener('change',updateSystem);document.removeEventListener('pointerdown',outside)}});
 async function login(){error='';try{await api('/session','POST',{username,password});password='';user=await api('/session');setCsrf(user.csrf);await resolveRoute()}catch(e){error=String(e)}}
 async function logout(){try{await api('/session','DELETE');loadSequence++;user=null;active=null;setCsrf('');clearCatalog()}catch(e){error=String(e)}}
 function go(url:string){push(url)}
 async function save(){notice='';try{const data=JSON.parse(draftText);if(draftId)await api(`/admin/problems/${draftId}`,'PUT',data);else{draftId=(await api('/admin/problems','POST',data)).id;replace(`/admin/problems/${draftId}`);route={kind:'admin-edit',problemId:draftId}}notice='Draft saved';drafts=await api('/admin/problems');return true}catch(e){error=String(e);return false}}
 async function publish(){if(!await save())return;try{const r=await api(`/admin/problems/${draftId}/publish`,'POST');notice=`Published immutable version ${r.version}`;void loadCatalog(true).catch(()=>{})}catch(e){error=String(e)}}
 function showPreview(){try{preview=JSON.parse(draftText)}catch(e){error=String(e)}}
 async function addUser(){try{await api('/admin/users','POST',{username:newUsername,password:newPassword});newUsername='';newPassword='';notice='User created'}catch(e){error=String(e)}}
 async function saveCatalog(){try{const data=JSON.parse(catalogText);await api('/admin/catalog','PUT',data);catalog=await api('/admin/catalog');catalogText=JSON.stringify(catalog.settings,null,2);notice='Catalog settings saved; reconciliation requested'}catch(e){error=String(e)}}
</script>
<svelte:head><title>{routeTitle()}</title></svelte:head>
<header>
 <button class="brand" onclick={()=>go('/')}>◈ <span>locoder</span></button>
 {#if user}
  <div class="header-search" bind:this={quickRoot} onfocusout={(event)=>{if(!quickRoot?.contains(event.relatedTarget as Node|null))dismissQuickSearch()}}>
   <input bind:this={quickInput} aria-label="Find a problem" role="combobox" aria-autocomplete="list" aria-haspopup="listbox" aria-controls="header-problem-results" aria-expanded={quickOpen} aria-activedescendant={quickOpen&&quickResults.length&&quickActiveIndex>=quickFirst&&quickActiveIndex<quickLast?`header-problem-${quickResults[quickActiveIndex].id}`:undefined} placeholder="Search problems…" value={quickQuery} oninput={(event)=>updateQuickSearch(event.currentTarget.value)} onfocus={()=>{if(quickQuery.trim())quickOpen=true;void loadCatalog().catch(()=>{})}} onkeydown={onQuickKeydown}/>
   {#if quickOpen}
    <div class="quick-dropdown">
     {#if catalogLoading&&!catalogLoaded}<div class="quick-state" role="status">Loading problems…</div>
     {:else if catalogError}<div class="quick-state" role="alert">Could not load problems. <button onclick={retryQuickSearch}>Retry</button></div>
     {:else if !quickResults.length}<div class="quick-state" role="status">No matching problems.</div>
     {:else}
      <div class="quick-results" id="header-problem-results" role="listbox" aria-label="Problem search results" tabindex="-1" bind:this={quickScrollElement} use:measureQuickList onscroll={(event)=>quickScrollTop=event.currentTarget.scrollTop}>
       <div style:height={`${quickFirst*quickRowHeight}px`} aria-hidden="true"></div>
       {#each quickResults.slice(quickFirst,quickLast) as p,i (p.id)}
        <button class="quick-option" id={`header-problem-${p.id}`} role="option" aria-selected={quickActiveIndex===quickFirst+i} aria-setsize={quickResults.length} aria-posinset={quickFirst+i+1} tabindex="-1" onclick={()=>openQuickProblem(p)} onmouseenter={()=>quickActiveIndex=quickFirst+i}>
         <span class="quick-option-copy"><strong>{p.title}</strong><small>{p.summary||p.tags.join(' · ')}</small></span>
         <span class={`badge ${p.difficulty}`}>{p.difficulty} · {p.difficulty_score}</span>
        </button>
       {/each}
       <div style:height={`${(quickResults.length-quickLast)*quickRowHeight}px`} aria-hidden="true"></div>
      </div>
     {/if}
    </div>
   {/if}
  </div>
 {/if}
 <nav>{#if user}<button onclick={()=>go('/')}>Problems</button>{#if user.admin}<button onclick={()=>go('/admin/problems')}>Authoring</button>{/if}<details class="user-menu"><summary aria-label="User menu">{user.username} <span aria-hidden="true">⌄</span></summary><div class="user-menu-items"><label class="theme-control" for="theme">Theme<select id="theme" aria-label="Theme" value={themeChoice} onchange={(e)=>chooseTheme(e.currentTarget.value as ThemeChoice)}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></label><button onclick={logout}>Sign out</button></div></details>{/if}</nav>
</header>
{#if error}<div class="alert" role="alert">{error}<button onclick={()=>error=''} aria-label="Dismiss error">×</button></div>{/if}
{#if loading}<main><p>Loading workspace…</p></main>
{:else if !user}<main class="login"><form onsubmit={(e)=>{e.preventDefault();login()}}><label>Username<input autocomplete="username" bind:value={username} required/></label><label>Password<input type="password" autocomplete="current-password" bind:value={password} required/></label><button class="primary">Sign in →</button></form><p class="muted small">Accounts are provided by your administrator.</p></main>
{:else if routeLoading}<main><p>Loading view…</p></main>
{:else if route.kind==='not-found'}<main class="route-state"><h1>Page not found</h1><p class="muted">The requested page or problem does not exist.</p><button class="primary" onclick={()=>go('/')}>Back to problems</button></main>
{:else if route.kind==='access-denied'}<main class="route-state"><h1>Access denied</h1><p class="muted">You do not have permission to open the authoring workspace.</p><button class="primary" onclick={()=>go('/')}>Back to problems</button></main>
{:else if route.kind==='admin'||route.kind==='admin-new'||route.kind==='admin-edit'}
 <main><h1>Problem studio</h1>{#if catalog}<section><h2>Git catalog</h2>{#if catalogText}<label>Catalog settings (JSON)<textarea class="definition" bind:value={catalogText} spellcheck="false"></textarea></label><button class="primary" onclick={saveCatalog}>Save catalog settings</button>{:else}<p class="muted">No catalog has been configured. Seed it through deployment settings on first start.</p>{/if}<p class="muted small">Discovered: {catalog.state.discovered_revision||'—'} · Applied: {catalog.state.applied_revision||'—'} · Last success: {catalog.state.last_successful_reconciliation||'—'}</p>{#if catalog.state.last_error}<p class="alert">{catalog.state.last_error}</p>{/if}<details><summary>Recent reconciliation runs</summary><pre>{JSON.stringify(catalog.runs,null,2)}</pre></details></section>{/if}<div class="studio"><aside><button class="primary" onclick={()=>go('/admin/problems/new')}>+ New problem</button>{#each drafts as d}<button class="draft" class:chosen={draftId===d.id} onclick={()=>go(`/admin/problems/${d.id}`)}>{d.draft.title}<small>{d.managed?'Git managed':d.version?'Published · editable draft':'Draft'}</small></button>{/each}<form onsubmit={(e)=>{e.preventDefault();addUser()}}><h3>Create user</h3><label>Username<input bind:value={newUsername} required/></label><label>Password<input type="password" bind:value={newPassword} minlength="12" required/></label><button>Create account</button></form></aside><section>{#if draftText}{#if drafts.find(d=>d.id===draftId)?.managed}<p class="muted">This definition is managed by Git and is read-only.</p>{:else}<div class="toolbar"><button onclick={save}>Save draft</button><button onclick={showPreview}>Preview statement</button><button class="primary" onclick={publish}>Validate & publish</button></div>{/if}<p class="muted small">Types: "int", "bool", "string", or {JSON.stringify({array:'int'})}. Tests require typed argument lists and expected results. Mark private cases with hidden: true.</p><label>Problem definition (JSON)<textarea class="definition" bind:value={draftText} readonly={drafts.find(d=>d.id===draftId)?.managed} spellcheck="false"></textarea></label>{#if preview}<article class="statement">{@html markdown(preview.statement||'')}</article>{/if}{:else}<p>Select a draft or create a new problem.</p>{/if}{#if notice}<p role="status" class="notice">{notice}</p>{/if}</section></div></main>
{:else if route.kind==='problem'&&active&&Editor}{#key `${active.id}:${active.version}`}<svelte:component this={Editor} problem={active} {user} theme={resolvedTheme} onerror={(e:string)=>error=e}/>{/key}
{:else if route.kind==='problems'}
 <main class="problems-page">
  <div class="list-heading"><h1>Problems</h1><span class="count">{problems.length} problems</span></div>
  <div class="filters">
   <input aria-label="Search problems" placeholder="Search problems…" bind:value={query} oninput={resetListScroll}/>
   <select aria-label="Difficulty" bind:value={difficulty} onchange={resetListScroll}><option value="">All difficulties</option><option>easy</option><option>medium</option><option>hard</option></select>
  </div>
  <div class="problem-list">
   <div class="table-heading"><span>PROBLEM</span><span>DIFFICULTY</span></div>
   <div class="problem-scroll" aria-label="Problem list" role="region" bind:this={listScrollElement} use:measureList onscroll={(event)=>listScrollTop=event.currentTarget.scrollTop}>
    {#if problems.length}
     <div style:height={`${firstRow*rowHeight}px`} aria-hidden="true"></div>
     {#each problems.slice(firstRow,lastRow) as p,i (p.id)}
      <button class="problem-row" onclick={()=>go(`/problems/${p.id}`)}>
       <span class="number">{String(firstRow+i+1).padStart(2,'0')}</span>
       <span class="problem-title">{p.title}<small>{p.summary||p.tags.join(' · ')}</small></span>
       <span class={`badge ${p.difficulty}`}>{p.difficulty} · {p.difficulty_score}</span>
       <span class="arrow">↗</span>
      </button>
     {/each}
     <div style:height={`${(problems.length-lastRow)*rowHeight}px`} aria-hidden="true"></div>
    {:else}
     <div class="empty">{#if allProblems.length}No matching problems.{:else}No problems yet. {user.admin?'Open Authoring to publish your first challenge.':'Ask your administrator to publish a challenge.'}{/if}</div>
    {/if}
   </div>
  </div>
 </main>
{/if}
