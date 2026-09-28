<script lang="ts">
 import DOMPurify from 'dompurify';
 import {marked} from 'marked';
 import type {components} from './api.generated';

 type Parameter=components['schemas']['Parameter'];
 type Interface=components['schemas']['Interface'];
 type Group={label:string;params:Parameter[]};

 export let interfaceDefinition:Interface|undefined;

 const constrained=(params:Parameter[]|undefined)=>
  (Array.isArray(params)?params:[]).filter(param=>typeof param?.constraints==='string'&&param.constraints.trim().length>0);
 function constraintGroups(definition:Interface|undefined):Group[]{
  if(!definition)return [];
  if(definition.kind==='function')return [{label:'',params:constrained(definition.params)}].filter(group=>group.params.length);
  if(definition.kind!=='data_structure')return [];
  return [
   {label:'Constructor',params:constrained(definition.constructor?.params)},
   ...(Array.isArray(definition.methods)?definition.methods:[]).map(method=>({label:`${method.name}()`,params:constrained(method.params)}))
  ].filter(group=>group.params.length);
 }
 const markdown=(value:string)=>DOMPurify.sanitize(marked.parse(value,{async:false}) as string);
 $: groups=constraintGroups(interfaceDefinition);
</script>

{#if groups.length}
 <section class="constraints" aria-label="Constraints">
  <h3>Constraints</h3>
  {#each groups as group}
   {#if group.label}<h4>{group.label}</h4>{/if}
   <dl>
    {#each group.params as param}
     <dt><code>{param.name}</code></dt>
     <dd>{@html markdown(param.constraints??'')}</dd>
    {/each}
   </dl>
  {/each}
 </section>
{/if}

<style>
 .constraints{margin:20px 0;font-size:14px;line-height:1.6}
 .constraints h3{margin:0 0 10px}
 .constraints h4{margin:15px 0 7px;font-size:13px}
 .constraints dl{margin:0}
 .constraints dt{margin:10px 0 2px;font-weight:600}
 .constraints dd{margin:0;overflow-wrap:anywhere}
 .constraints :global(dd p){margin:0}
</style>
