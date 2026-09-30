import {test,expect,type BrowserContext,type Page} from '../../web/node_modules/@playwright/test/index';

const problemId='00000000-0000-4000-8000-000000000011';
const firstVersion='00000000-0000-4000-8000-000000000012';
const secondVersion='00000000-0000-4000-8000-000000000013';
const userId='00000000-0000-4000-8000-000000000014';
const starters={cpp:'int solve() { return 1; }',python:'def solve():\n    return 1',java:'class Solution { int solve() { return 1; } }',kotlin:'fun solve(): Int = 1'};
type Backend={drafts:Map<string,string>;version:string;failWrites:number};

async function mockBackend(context:BrowserContext,backend:Backend){
 await context.route('**/api/v1/**',async route=>{
  const request=route.request();
  const path=new URL(request.url()).pathname;
  const solution=path.match(/^\/api\/v1\/solutions\/([^/]+)\/(cpp|python|java|kotlin)$/);
  if(solution){
   const key=`${solution[1]}:${solution[2]}`;
   if(request.method()==='PUT'){
    if(backend.failWrites>0){backend.failWrites--;return route.fulfill({status:503,contentType:'application/json',body:JSON.stringify({error:'temporarily unavailable'})})}
    backend.drafts.set(key,request.postDataJSON().source);
    return route.fulfill({status:204});
   }
   const source=backend.drafts.get(key);
   return route.fulfill(source===undefined?{status:404,contentType:'application/json',body:JSON.stringify({error:'solution not found'})}:{status:200,contentType:'application/json',body:JSON.stringify({source,updated_at:'2026-09-25T00:00:00Z'})});
  }
  if(path.endsWith('/editor-ticket'))return route.fulfill({status:503,contentType:'application/json',body:JSON.stringify({error:'semantic unavailable'})});
  const data=path.endsWith('/session')?{id:userId,username:'tester',admin:false,csrf:'csrf'}:
   path.endsWith(`/problems/${problemId}`)?{id:problemId,version:backend.version,problem:{title:'Cloud sample',statement:'Return the value.',difficulty:'easy',tags:[],interface:{kind:'function',name:'solve',params:[],returns:'int'},limits:{time_ms:2000,memory_mib:256},tests:[]},starters}:[];
  return route.fulfill({status:200,contentType:'application/json',body:JSON.stringify(data)});
 });
}

async function openEditor(page:Page){
 await page.goto(`/problems/${problemId}`);
 await expect(page.getByRole('button',{name:'Run',exact:true})).toBeEnabled({timeout:45000});
}
async function replaceCode(page:Page,source:string){
 await page.locator('.monaco-editor').first().click();
 await page.keyboard.press('ControlOrMeta+A');
 await page.keyboard.insertText(source);
}

test('saves both languages and restores them in a fresh browser setup',async({browser,page})=>{
 test.setTimeout(90000);
 const backend:Backend={drafts:new Map(),version:firstVersion,failWrites:0};
 await mockBackend(page.context(),backend);
 await openEditor(page);
 await replaceCode(page,'int solve() { return 7; }');
 await expect.poll(()=>backend.drafts.get(`${firstVersion}:cpp`)).toBe('int solve() { return 7; }');
 await page.getByLabel('Language').selectOption('python');
 await replaceCode(page,'def solve(): return 9');
 await expect.poll(()=>backend.drafts.get(`${firstVersion}:python`)).toBe('def solve(): return 9');
 await page.getByLabel('Language').selectOption('java');
 await expect(page.locator('.semantic')).toHaveAttribute('aria-label',/Basic completion/);
 await replaceCode(page,'class Solution { int solve() { return 11; } }');
 await expect.poll(()=>backend.drafts.get(`${firstVersion}:java`)).toBe('class Solution { int solve() { return 11; } }');
 await page.getByLabel('Language').selectOption('kotlin');
 await replaceCode(page,'fun solve(): Int = 13');
 await expect.poll(()=>backend.drafts.get(`${firstVersion}:kotlin`)).toBe('fun solve(): Int = 13');
 const second=await browser.newContext({baseURL:new URL(page.url()).origin});
 try{
  await mockBackend(second,backend);
  const other=await second.newPage();
  await openEditor(other);
  await expect(other.locator('.monaco-editor .view-lines').first()).toContainText('return 7');
  await other.getByLabel('Language').selectOption('python');
  await expect(other.locator('.monaco-editor .view-lines').first()).toContainText('return 9');
  await other.getByLabel('Language').selectOption('java');
  await expect(other.locator('.monaco-editor .view-lines').first()).toContainText('return 11');
  await expect(other.locator('.filename')).toContainText('Solution.java');
  await other.getByLabel('Language').selectOption('kotlin');
  await expect(other.locator('.monaco-editor .view-lines').first()).toContainText('= 13');
  await expect(other.locator('.filename')).toContainText('solution.kt');
 }finally{await second.close()}
});

test('migrates a browser draft only when cloud has no copy',async({page})=>{
 test.setTimeout(90000);
 const backend:Backend={drafts:new Map(),version:firstVersion,failWrites:0};
 await mockBackend(page.context(),backend);
 await page.goto('/');
 await page.evaluate(([key,value])=>localStorage.setItem(key,value),[`practice:${userId}:${firstVersion}:cpp`,'int solve() { return 3; }']);
 await openEditor(page);
 await expect(page.locator('.monaco-editor .view-lines').first()).toContainText('return 3');
 await expect.poll(()=>backend.drafts.get(`${firstVersion}:cpp`)).toBe('int solve() { return 3; }');
 backend.drafts.set(`${firstVersion}:cpp`,'int solve() { return 8; }');
 await page.evaluate(([key,value])=>localStorage.setItem(key,value),[`practice:${userId}:${firstVersion}:cpp`,'int solve() { return 2; }']);
 await page.reload();
 await expect(page.locator('.monaco-editor .view-lines').first()).toContainText('return 8',{timeout:45000});
 await expect.poll(()=>page.evaluate(key=>localStorage.getItem(key),`practice:${userId}:${firstVersion}:cpp`)).toBe('int solve() { return 8; }');
});

test('retries an unsent draft and keeps versions separate',async({page})=>{
 test.setTimeout(90000);
 const backend:Backend={drafts:new Map(),version:firstVersion,failWrites:1};
 await mockBackend(page.context(),backend);
 await openEditor(page);
 const failed=page.waitForResponse(response=>response.url().includes(`/solutions/${firstVersion}/cpp`)&&response.request().method()==='PUT'&&response.status()===503);
 await replaceCode(page,'int solve() { return 5; }');
 await failed;
 await expect.poll(()=>page.evaluate(key=>localStorage.getItem(key),`practice:${userId}:${firstVersion}:cpp:unsent`)).toBe('1');
 await page.reload();
 await expect(page.locator('.monaco-editor .view-lines').first()).toContainText('return 5',{timeout:45000});
 await expect.poll(()=>backend.drafts.get(`${firstVersion}:cpp`)).toBe('int solve() { return 5; }');
 backend.version=secondVersion;
 await page.reload();
 await expect(page.locator('.monaco-editor .view-lines').first()).toContainText('return 1',{timeout:45000});
 await page.goto('/');
 await page.waitForTimeout(1000);
 expect(backend.drafts.get(`${firstVersion}:cpp`)).toBe('int solve() { return 5; }');
 expect(backend.drafts.has(`${secondVersion}:cpp`)).toBe(false);
});

test('serializes writes while edits continue',async({page})=>{
 test.setTimeout(90000);
 const backend:Backend={drafts:new Map(),version:firstVersion,failWrites:0};
 await mockBackend(page.context(),backend);
 let releaseFirst!:()=>void,markStarted!:()=>void;
 const firstStarted=new Promise<void>(resolve=>markStarted=resolve);
 const gate=new Promise<void>(resolve=>releaseFirst=resolve);
 let writes=0;
 await page.route(`**/api/v1/solutions/${firstVersion}/cpp`,async route=>{
  if(route.request().method()==='PUT'){
   writes++;
   if(writes===1){markStarted();await gate}
  }
  await route.fallback();
 });
 await openEditor(page);
 const cloudStatus=page.locator('.cloud-status');
 await expect(cloudStatus).toHaveAttribute('aria-label','Saved to cloud');
 await replaceCode(page,'int solve() { return 2; }');
 await firstStarted;
 await expect(cloudStatus).toHaveAttribute('aria-label','Saving to cloud');
 await replaceCode(page,'int solve() { return 3; }');
 await expect(cloudStatus).toHaveAttribute('aria-label','Changes waiting to save');
 await page.waitForTimeout(1000);
 expect(writes).toBe(1);
 releaseFirst();
 await expect.poll(()=>backend.drafts.get(`${firstVersion}:cpp`)).toBe('int solve() { return 3; }');
 await expect(cloudStatus).toHaveAttribute('aria-label','Saved to cloud');
 expect(writes).toBe(2);
});

test('does not clear an unsent draft while an older write is in flight',async({page})=>{
 test.setTimeout(90000);
 const original='int solve() { return 4; }';
 const backend:Backend={drafts:new Map([[`${firstVersion}:cpp`,original]]),version:firstVersion,failWrites:0};
 await mockBackend(page.context(),backend);
 let releaseFirst!:()=>void,markStarted!:()=>void;
 const firstStarted=new Promise<void>(resolve=>markStarted=resolve);
 const gate=new Promise<void>(resolve=>releaseFirst=resolve);
 let writes=0;
 await page.route(`**/api/v1/solutions/${firstVersion}/cpp`,async route=>{
  if(route.request().method()==='PUT'&&++writes===1){markStarted();await gate}
  await route.fallback();
 });
 await openEditor(page);
 await expect(page.locator('.monaco-editor .view-lines').first()).toContainText('return 4');
 await replaceCode(page,'int solve() { return 5; }');
 await firstStarted;
 await replaceCode(page,original);
 await page.getByLabel('Language').selectOption('python');
 await page.getByLabel('Language').selectOption('cpp');
 releaseFirst();
 await expect.poll(()=>writes).toBe(2);
 await expect.poll(()=>backend.drafts.get(`${firstVersion}:cpp`)).toBe(original);
 await expect.poll(()=>page.evaluate(key=>localStorage.getItem(key),`practice:${userId}:${firstVersion}:cpp:unsent`)).toBeNull();
 await page.reload();
 await expect(page.locator('.monaco-editor .view-lines').first()).toContainText('return 4',{timeout:45000});
});

test('submits Java and Kotlin source with basic editor fallback',async({page})=>{
 test.setTimeout(90000);
 const backend:Backend={drafts:new Map(),version:firstVersion,failWrites:0};
 await mockBackend(page.context(),backend);
 const submitted:{language:string;source:string}[]=[];
 await page.route('**/api/v1/submissions**',async route=>{
  const request=route.request();
  const path=new URL(request.url()).pathname;
  if(request.method()==='POST'){
   const body=request.postDataJSON();submitted.push({language:body.language,source:body.source});
   return route.fulfill({json:{id:'run-'+submitted.length}});
  }
  if(path.match(/\/submissions\/run-\d+$/))return route.fulfill({json:{status:'completed',result:{verdict:'accepted',passed:0,total:0,cases:[]}}});
  return route.fulfill({json:[]});
 });
 await openEditor(page);
 for(const [language,source] of [['java','class Solution { int solve() { return 7; } }'],['kotlin','fun solve(): Int = 9']] as const){
  await page.getByLabel('Language').selectOption(language);
  await expect(page.locator('.semantic')).toHaveAttribute('aria-label',/Basic completion/);
  await replaceCode(page,source);
  await page.getByRole('button',{name:'Run',exact:true}).click();
  await expect.poll(()=>submitted.length).toBe(language==='java'?1:2);
  await expect(page.getByRole('button',{name:'Run',exact:true})).toBeEnabled();
 }
 expect(submitted).toEqual([{language:'java',source:'class Solution { int solve() { return 7; } }'},{language:'kotlin',source:'fun solve(): Int = 9'}]);
});
