<script lang="ts">
 import {onMount} from 'svelte';
 import {api,setCsrf} from './api';
 import DOMPurify from 'dompurify';
 import {marked} from 'marked';
 let user:any=null, loading=true, error='', username='', password='', problems:any[]=[], active:any=null, query='', difficulty='', tag='', adminMode=false;
 let Editor:any=null;
 let drafts:any[]=[], draftId='', draftText='', preview:any=null, notice='';
 let newUsername='',newPassword='';
 const template={title:'New problem',statement:'# New problem\n\nDescribe the task.',difficulty:'easy',tags:['arrays'],signature:{method:'solve',params:[{name:'values',ty:{array:'int'}}],returns:'int'},limits:{time_ms:2000,memory_mib:256,output_bytes:1048576},tests:[{args:[[1,2]],expected:3,hidden:false},{args:[[]],expected:0,hidden:true}]};
 const markdown=(s:string)=>DOMPurify.sanitize(marked.parse(s,{async:false}) as string);
 async function list(){problems=await api(`/problems?${new URLSearchParams({q:query,difficulty,tag})}`)}
 async function init(){try{user=await api('/session');setCsrf(user.csrf);await list()}catch{user=null}finally{loading=false}}
 onMount(init);
 async function login(){error='';try{await api('/session','POST',{username,password});password='';await init()}catch(e){error=String(e)}}
 async function logout(){try{await api('/session','DELETE');user=null;active=null;setCsrf('')}catch(e){error=String(e)}}
 async function open(id:string){try{active=await api(`/problems/${id}`);adminMode=false;Editor=(await import('./Editor.svelte')).default}catch(e){error=String(e)}}
 async function author(){try{drafts=await api('/admin/problems');adminMode=true;active=null}catch(e){error=String(e)}}
 function edit(d:any){draftId=d?.id||'';draftText=JSON.stringify(d?.draft||template,null,2);preview=null;notice=''}
 async function save(){notice="";try{const data=JSON.parse(draftText);if(draftId)await api(`/admin/problems/${draftId}`,'PUT',data);else draftId=(await api('/admin/problems','POST',data)).id;notice='Draft saved';drafts=await api('/admin/problems');return true}catch(e){error=String(e);return false}}
 async function publish(){if(!await save())return;try{const r=await api(`/admin/problems/${draftId}/publish`,'POST');notice=`Published immutable version ${r.version}`;await list()}catch(e){error=String(e)}}
 function showPreview(){try{preview=JSON.parse(draftText)}catch(e){error=String(e)}}
 async function addUser(){try{await api('/admin/users','POST',{username:newUsername,password:newPassword});newUsername='';newPassword='';notice='User created'}catch(e){error=String(e)}}
</script>
<svelte:head><title>{active?active.problem.title+' · ':''}Practice</title></svelte:head>
<header>
 <button class="brand" onclick={()=>{active=null;adminMode=false}}>◈ <span>practice</span></button>
 <nav>{#if user}<button onclick={()=>{active=null;adminMode=false}}>Problems</button>{#if user.admin}<button onclick={author}>Authoring</button>{/if}<span>{user.username}</span><button onclick={logout}>Sign out</button>{/if}</nav>
</header>
{#if error}<div class="alert" role="alert">{error}<button onclick={()=>error=''} aria-label="Dismiss error">×</button></div>{/if}
{#if loading}
 <main><p>Loading workspace…</p></main>
{:else if !user}
 <main class="login"><form onsubmit={(e)=>{e.preventDefault();login()}}><label>Username<input autocomplete="username" bind:value={username} required/></label><label>Password<input type="password" autocomplete="current-password" bind:value={password} required/></label><button class="primary">Sign in →</button></form><p class="muted small">Accounts are provided by your administrator.</p></main>
{:else if adminMode}
 <main><h1>Problem studio</h1><div class="studio"><aside><button class="primary" onclick={()=>edit(null)}>+ New problem</button>{#each drafts as d}<button class="draft" onclick={()=>edit(d)}>{d.draft.title}<small>{d.version?'Published · editable draft':'Draft'}</small></button>{/each}<form onsubmit={(e)=>{e.preventDefault();addUser()}}><h3>Create user</h3><label>Username<input bind:value={newUsername} required/></label><label>Password<input type="password" bind:value={newPassword} minlength="12" required/></label><button>Create account</button></form></aside><section>{#if draftText}<div class="toolbar"><button onclick={save}>Save draft</button><button onclick={showPreview}>Preview statement</button><button class="primary" onclick={publish}>Validate & publish</button></div><p class="muted small">Types: "int", "bool", "string", or {JSON.stringify({array:'int'})}. Tests require typed argument lists and expected results. Mark private cases with hidden: true.</p><label>Problem definition (JSON)<textarea class="definition" bind:value={draftText} spellcheck="false"></textarea></label>{#if preview}<article class="statement">{@html markdown(preview.statement||'')}</article>{/if}{:else}<p>Select a draft or create a new problem.</p>{/if}{#if notice}<p role="status" class="notice">{notice}</p>{/if}</section></div></main>
{:else if active && Editor}
 <svelte:component this={Editor} problem={active} {user} onerror={(e:string)=>error=e}/>
{:else}
 <main><div class="list-heading"><h1>Problems</h1><span class="count">{problems.length} problems</span></div><form class="filters" onsubmit={(e)=>{e.preventDefault();list().catch(e=>error=String(e))}}><input aria-label="Search problems" placeholder="Search problems…" bind:value={query}/><select aria-label="Difficulty" bind:value={difficulty}><option value="">All difficulties</option><option>easy</option><option>medium</option><option>hard</option></select><input aria-label="Tag" placeholder="Filter by tag" bind:value={tag}/><button>Search</button></form><div class="problem-list"><div class="table-heading"><span>PROBLEM</span><span>DIFFICULTY</span></div>{#each problems as p,i}<button class="problem-row" onclick={()=>open(p.id)}><span class="number">{String(i+1).padStart(2,'0')}</span><span class="problem-title">{p.title}<small>{p.tags.join(' · ')}</small></span><span class={`badge ${p.difficulty}`}>{p.difficulty}</span><span class="arrow">↗</span></button>{:else}<div class="empty">No problems yet. {user.admin?'Open Authoring to publish your first challenge.':'Ask your administrator to publish a challenge.'}</div>{/each}</div></main>
{/if}
