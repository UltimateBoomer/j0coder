import {languageDescriptors} from './language-fixtures';
import {selectValue} from './select-helpers';
import {test,expect} from '../../web/node_modules/@playwright/test/index';
const defaults={theme:'system',default_language:'cpp',semantic_completion:true,font_size:14,tab_width:4,word_wrap:false,minimap:false,blind_mode:false};
const id='00000000-0000-4000-8000-000000000001',version='00000000-0000-4000-8000-000000000002';
const problem={id,version,problem:{title:'Guest sample',statement:'Return one.',difficulty:'easy',tags:[],interface:{kind:'function',name:'solve',params:[],returns:'int'},limits:{time_ms:2000,memory_mib:256},tests:[]},starters:{cpp:'int solve() { return 1; }',python:'def solve():\n    return 1'}};
test('guest editor remains local and settings persist across reload',async({page})=>{
 const protectedRequests:string[]=[];
 await page.route('**/api/v1/**',route=>{
  const path=new URL(route.request().url()).pathname;
  if(path.endsWith('/capabilities'))return route.fulfill({json:{languages:languageDescriptors,guest_browsing:true,registration:'invite',web_admin:false}});
  if(path.endsWith('/session'))return route.fulfill({status:401,json:{error:'authentication required'}});
  if(path.endsWith('/problems/'+id))return route.fulfill({json:problem});
  if(path.endsWith('/problems'))return route.fulfill({json:[]});
  protectedRequests.push(path);return route.fulfill({status:401,json:{error:'authentication required'}});
 });
 await page.goto('/settings');await selectValue(page.getByRole('combobox',{name:'Default language',exact:true}),'python');await selectValue(page.getByRole('combobox',{name:'Theme',exact:true}),'dark');await page.getByRole('button',{name:'Save preferences'}).click();
 await page.goto('/problems/'+id);await expect(page.getByRole('heading',{name:'Guest sample'})).toBeVisible();
 await expect(page.getByRole('combobox',{name:'Language',exact:true})).toHaveAttribute('data-value','python');await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
 await expect(page.getByRole('button',{name:'Run',exact:true})).toBeDisabled();
 const source=page.locator('.monaco-editor .view-lines');await expect(source).toContainText('def solve');
 await page.locator('.monaco-editor').click();await page.keyboard.press('Control+a');await page.keyboard.type('def solve(): return 7');
 await page.reload();await expect(source).toContainText('return 7');expect(protectedRequests).toEqual([]);
});
test('account preferences set editor defaults and disabling semantic cancels retries',async({page})=>{
 let prefs={...defaults,default_language:'python',theme:'dark',semantic_completion:false},tickets=0;
 await page.route('**/api/v1/**',route=>{
  const path=new URL(route.request().url()).pathname;
  if(path.endsWith('/capabilities'))return route.fulfill({json:{languages:languageDescriptors,guest_browsing:false,registration:'invite',web_admin:false}});
  if(path.endsWith('/session'))return route.fulfill({json:{id:'account',username:'tester',admin:false,csrf:'csrf'}});
  if(path.endsWith('/me/preferences')){if(route.request().method()==='PATCH')prefs={...prefs,...route.request().postDataJSON()};return route.fulfill({json:prefs})}
  if(path.endsWith('/me/limits'))return route.fulfill({json:{submissions_minute:6,submissions_day:100,pending:2,editor_sessions:1}});
  if(path.endsWith('/editor-ticket')){tickets++;return route.fulfill({status:503,json:{error:'unavailable'}})}
  if(path.includes('/solutions/'))return route.fulfill({status:404,json:{error:'no solution'}});
  if(path.endsWith('/problems/'+id))return route.fulfill({json:problem});
  return route.fulfill({json:[]});
 });
 await page.goto('/problems/'+id);await expect(page.getByRole('combobox',{name:'Language',exact:true})).toHaveAttribute('data-value','python');await expect(page.locator('.monaco-editor .view-lines')).toContainText('def solve');expect(tickets).toBe(0);
 await page.getByLabel('User menu').click();await page.getByRole('button',{name:'Enable semantic completion'}).click();await expect.poll(()=>tickets).toBeGreaterThan(0);
 await page.getByRole('button',{name:'Disable semantic completion'}).click();const stopped=tickets;await page.waitForTimeout(1600);expect(tickets).toBe(stopped);
 await page.goto('/settings');await selectValue(page.getByRole('combobox',{name:'Default language',exact:true}),'cpp');await page.getByRole('button',{name:'Save preferences'}).click();await expect(page.getByRole('status')).toContainText('saved to your account');expect(prefs.default_language).toBe('cpp');expect(prefs.semantic_completion).toBe(false);
});

test('metadata initialization is retryable and menus follow usable descriptors',async({page})=>{
 let attempts=0;
 await page.route('**/api/v1/**',route=>{
  const path=new URL(route.request().url()).pathname;
  if(path.endsWith('/capabilities'))return route.fulfill({json:{guest_browsing:true,registration:'invite',web_admin:false,...(++attempts>1?{languages:[languageDescriptors[1],{...languageDescriptors[0],id:'future',monaco_language:'future'}]}:{})}});
  if(path.endsWith('/session'))return route.fulfill({status:401,json:{error:'authentication required'}});
  return route.fulfill({json:[]});
 });
 await page.goto('/settings');
 await expect(page.getByRole('button',{name:'Retry initialization'})).toBeVisible();
 await page.getByRole('button',{name:'Retry initialization'}).click();
 await expect(page.getByRole('combobox',{name:'Default language'})).toHaveAttribute('data-value','python');
 await page.getByRole('button',{name:'Reset to defaults'}).click();
 await expect(page.getByRole('combobox',{name:'Default language'})).toHaveAttribute('data-value','python');
 await page.getByRole('combobox',{name:'Default language'}).click();
 await expect(page.getByRole('option').locator('span:first-child')).toHaveText(['Python']);
 await page.keyboard.press('Escape');
});

test('editor requires starters and falls back from an unavailable preference',async({page})=>{
 await page.addInitScript(()=>localStorage.setItem('j0coder:guest-preferences',JSON.stringify({...{theme:'system'},default_language:'java'})));
 await page.route('**/api/v1/**',route=>{
  const path=new URL(route.request().url()).pathname;
  if(path.endsWith('/capabilities'))return route.fulfill({json:{languages:languageDescriptors,guest_browsing:true,registration:'invite',web_admin:false}});
  if(path.endsWith('/session'))return route.fulfill({status:401,json:{error:'authentication required'}});
  if(path.endsWith('/problems/'+id))return route.fulfill({json:problem});
  return route.fulfill({json:[]});
 });
 await page.goto('/problems/'+id);
 await expect(page.getByRole('combobox',{name:'Language',exact:true})).toHaveAttribute('data-value','cpp');
 await expect(page.getByRole('combobox',{name:'Language',exact:true})).toBeEnabled();
 await page.getByRole('combobox',{name:'Language',exact:true}).click();
 await expect(page.getByRole('option').locator('span:first-child')).toHaveText(['C++20','Python 3']);
 await page.keyboard.press('Escape');
 await selectValue(page.getByRole('combobox',{name:'Language',exact:true}),'python');
 await expect(page.locator('.filename')).toHaveText('solution.py');
 await expect(page.locator('.monaco-editor .view-lines')).toContainText('def solve');
});

test('missing starters show a retryable error without mounting the editor',async({page})=>{
 let attempts=0;
 await page.route('**/api/v1/**',route=>{
  const path=new URL(route.request().url()).pathname;
  if(path.endsWith('/capabilities'))return route.fulfill({json:{languages:languageDescriptors,guest_browsing:true,registration:'invite',web_admin:false}});
  if(path.endsWith('/session'))return route.fulfill({status:401,json:{error:'authentication required'}});
  if(path.endsWith('/problems/'+id))return route.fulfill({json:{...problem,starters:++attempts===1?{}:problem.starters}});
  return route.fulfill({json:[]});
 });
 await page.goto('/problems/'+id);
 await expect(page.getByRole('alert')).toContainText('No usable language starters');
 await expect(page.locator('.monaco-editor')).toHaveCount(0);
 await page.getByRole('button',{name:'Retry initialization'}).click();
 await expect(page.locator('.monaco-editor .view-lines')).toContainText('int solve');
});

for(const stored of ['null','{broken'])test(`corrupt local preferences select an available default: ${stored}`,async({page})=>{
 await page.addInitScript(value=>localStorage.setItem('j0coder:guest-preferences',value),stored);
 await page.route('**/api/v1/**',route=>{
  const path=new URL(route.request().url()).pathname;
  if(path.endsWith('/capabilities'))return route.fulfill({json:{languages:[languageDescriptors[1]],guest_browsing:true,registration:'invite',web_admin:false}});
  if(path.endsWith('/session'))return route.fulfill({status:401,json:{error:'authentication required'}});
  return route.fulfill({json:[]});
 });
 await page.goto('/settings');
 await expect(page.getByRole('combobox',{name:'Default language'})).toHaveAttribute('data-value','python');
});
