<script lang="ts">
 import {languages} from "./languages";
 import {onMount} from 'svelte';import {api} from './api';import {defaults,type Preferences} from './preferences';
 export let privateAdmin=false;export let preferences:Preferences;export let signedIn:boolean;export let onpreferences:(p:Preferences)=>void;export let onlogout:()=>void;
 let draft:Preferences={...preferences},busy=false,status='',error='',limits:any=null,currentPassword='',newPassword='',adminPassword='';
 onMount(()=>{if(signedIn)api('/me/limits').then(v=>limits=v).catch(e=>error=String(e))});
 async function save(){busy=true;error='';try{const p=signedIn?await api('/me/preferences','PATCH',draft):draft;onpreferences(p);status=signedIn?'Preferences saved to your account.':'Preferences saved on this browser.'}catch(e){error=String(e)}finally{busy=false}}
 async function password(){busy=true;error='';try{await api('/me/password','POST',{current_password:currentPassword,new_password:newPassword});currentPassword='';newPassword='';onlogout()}catch(e){error=String(e)}finally{busy=false}}
 async function reauthenticate(){try{await api('/me/reauthenticate','POST',{username:'',password:adminPassword});adminPassword='';status='Authoring reauthenticated for five minutes.'}catch(e){error=String(e)}}
 async function logoutAll(){busy=true;try{await api('/me/logout-all','POST');onlogout()}catch(e){error=String(e)}finally{busy=false}}
</script>
<main class="settings"><h1>Settings</h1><p class="muted">{signedIn?'Preferences follow your account across devices.':'Guest preferences stay on this browser.'}</p>
 {#if error}<p role="alert">{error}</p>{/if}
 <form onsubmit={(e)=>{e.preventDefault();void save()}}>
 <label>Theme<select aria-label="Theme" bind:value={draft.theme}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></label>
 <label>Default language<select aria-label="Default language" bind:value={draft.default_language}>{#each languages() as language}<option value={language.id}>{language.label}</option>{/each}</select></label>
 <label class="check"><input type="checkbox" bind:checked={draft.semantic_completion} disabled={!signedIn}/>Semantic completion</label><p class="muted small">{signedIn?'Uses an isolated language server. Turning it off releases server resources; basic editing stays available.':'Sign in to use semantic completion.'}</p>
 <label>Font size<input type="number" min="10" max="24" bind:value={draft.font_size}/></label>
 <label>Tab width<select aria-label="Tab width" bind:value={draft.tab_width}><option value={2}>2</option><option value={4}>4</option><option value={8}>8</option></select></label>
 <label class="check"><input type="checkbox" bind:checked={draft.word_wrap}/>Word wrap</label><label class="check"><input type="checkbox" bind:checked={draft.minimap}/>Minimap</label><label class="check"><input type="checkbox" bind:checked={draft.blind_mode}/>Hide problem difficulty and tags</label>
 <div class="toolbar"><button class="primary" disabled={busy}>Save preferences</button><button type="button" onclick={()=>{draft={...defaults};status='Defaults selected; save to apply.'}}>Reset to defaults</button></div><p role="status">{status}</p>
 </form>
 {#if signedIn}{#if localStorage.getItem('j0coder:theme')}<button onclick={()=>{draft.theme=(localStorage.getItem('j0coder:theme')||'system') as Preferences['theme'];status='Browser theme selected; save to import.'}}>Import previous browser theme</button>{/if}
<h2>Account</h2>{#if privateAdmin}<form onsubmit={(e)=>{e.preventDefault();void reauthenticate()}}><label>Reauthenticate for authoring<input type="password" autocomplete="current-password" bind:value={adminPassword} required maxlength="256"/></label><button>Reauthenticate</button></form>{/if}<form onsubmit={(e)=>{e.preventDefault();void password()}}><label>Current password<input type="password" autocomplete="current-password" bind:value={currentPassword} required maxlength="256"/></label><label>New password<input type="password" autocomplete="new-password" bind:value={newPassword} required minlength="15" maxlength="256"/></label><button disabled={busy}>Change password and sign out</button></form><button disabled={busy} onclick={logoutAll}>Sign out all devices</button>
 {#if limits}<h2>Usage limits</h2><p>{limits.submissions_minute} runs per minute · {limits.submissions_day} per UTC day · {limits.pending} pending runs · {limits.editor_sessions} semantic sessions</p><p class="muted small">Contact the host operator to adjust these limits.</p>{/if}{/if}
</main>
<style>.settings{max-width:680px}.settings form{display:grid;gap:16px;margin:24px 0}.check{flex-direction:row;align-items:center}.settings h2{margin-top:32px}</style>
