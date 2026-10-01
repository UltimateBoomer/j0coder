import {test,expect} from '../../web/node_modules/@playwright/test/index';

const id='00000000-0000-4000-8000-000000000031';
const detail={id,version:'00000000-0000-4000-8000-000000000032',problem:{title:'Rename sample',statement:'Return a value.',difficulty:'easy',tags:['arrays'],interface:{kind:'function',name:'solve',params:[],returns:'int'},limits:{time_ms:2000,memory_mib:256},tests:[]},starters:{cpp:'int solve() { return 1; }',python:'def solve():\n    return 1'}};

for(const scenario of [
 {name:'migrates legacy preferences',legacy:true,current:false,theme:'dark',blind:true},
 {name:'keeps new preferences over legacy values',legacy:true,current:true,theme:'light',blind:false},
 {name:'starts with defaults without saved preferences',legacy:false,current:false,theme:'light',blind:false}
] as const){
 test(scenario.name,async({page})=>{
  await page.emulateMedia({colorScheme:'light'});
  await page.route('**/api/v1/**',route=>{
   const path=new URL(route.request().url()).pathname;
   if(path.endsWith('/editor-ticket'))return route.fulfill({status:503,json:{error:'editor unavailable'}});
   if(path.includes('/solutions/'))return route.fulfill(route.request().method()==='GET'?{status:404,json:{error:'not found'}}:{status:204});
   return route.fulfill({json:path.endsWith('/session')?{id:'rename-user',username:'tester',admin:false,csrf:'csrf'}:path.endsWith(`/problems/${id}`)?detail:[]});
  });
  await page.addInitScript(({legacy,current})=>{
   if(sessionStorage.getItem('rename-seeded'))return;
   sessionStorage.setItem('rename-seeded','true');
   if(legacy){localStorage.setItem('locoder:theme','dark');localStorage.setItem('locoder:blind-mode','true')}
   if(current){localStorage.setItem('j0coder:theme','light');localStorage.setItem('j0coder:blind-mode','false')}
  },scenario);
  await page.goto(`/problems/${id}`);
  await expect(page).toHaveTitle(/ · j0coder$/);
  await expect(page.getByRole('button',{name:'◈ j0coder'})).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme',scenario.theme);
  const blind=page.getByRole('button',{name:scenario.blind?'Show problem details':'Enable blind mode'});
  await expect(blind).toHaveAttribute('aria-pressed',String(scenario.blind));
  if(scenario.legacy){
   expect(await page.evaluate(()=>localStorage.getItem('j0coder:theme'))).toBe(scenario.theme);
   expect(await page.evaluate(()=>localStorage.getItem('j0coder:blind-mode'))).toBe(String(scenario.blind));
  }
  const nextTheme=scenario.theme==='dark'?'light':'dark';
  await page.getByLabel('User menu').click();
  await page.getByLabel('Theme').selectOption(nextTheme);
  await page.getByLabel('User menu').click();
  await blind.click();
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme',nextTheme);
  await expect(page.getByRole('button',{name:scenario.blind?'Enable blind mode':'Show problem details'})).toHaveAttribute('aria-pressed',String(!scenario.blind));
  const settings=await page.evaluate(()=>Object.fromEntries(['locoder:theme','locoder:blind-mode','j0coder:theme','j0coder:blind-mode'].map(key=>[key,localStorage.getItem(key)])));
  expect(settings).toEqual({'locoder:theme':scenario.legacy?'dark':null,'locoder:blind-mode':scenario.legacy?'true':null,'j0coder:theme':nextTheme,'j0coder:blind-mode':String(!scenario.blind)});
 });
}
