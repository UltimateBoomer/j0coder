import {selectValue} from './select-helpers';
import {test} from './account-fixtures';
import {expect} from '../../web/node_modules/@playwright/test/index';

test('editor appearance, privacy controls, resizing, and reset',async({page})=>{
 test.setTimeout(90000);
 const id='00000000-0000-4000-8000-000000000001';
 const version='00000000-0000-4000-8000-000000000002';
 const problem={id,version,problem:{title:'UI sample',statement:'Return the value.',hints:['Use **addition**. <script>alert(1)</script>','Return the result.'],difficulty:'medium',tags:['arrays'],interface:{kind:'function',name:'solve',params:[],returns:'int'},limits:{time_ms:2000,memory_mib:256},tests:[{args:[1],expected:1,hidden:false}]},starters:{cpp:'int solve() { return 1; }',python:'def solve():\n    return 1'}};
 const dropdownStyle=async(label:string)=>{
  const control=page.getByRole('combobox',{name:label,exact:true});
  await control.click();
  const style=await control.evaluate(node=>{const css=getComputedStyle(node);return {scheme:css.colorScheme,background:css.backgroundColor,text:css.color}});
  style.background=await page.locator('.select-popup:popover-open').evaluate(node=>getComputedStyle(node).backgroundColor);
  const option=await page.locator('.select-popup:popover-open .themed-option[aria-selected=false]:not(.active)').first().evaluate(node=>{const css=getComputedStyle(node);return {optionBackground:css.backgroundColor,optionText:css.color}});
  await control.press('Escape');
  return {...style,...option};
 };
 await page.route('**/api/v1/**',route=>{
  const path=new URL(route.request().url()).pathname;
  if(path.endsWith("/capabilities")||path.includes("/me/"))return route.fallback();
  if(path.includes('/solutions/'))return route.fulfill(route.request().method()==='GET'?{status:404,json:{error:'solution not found'}}:{status:204});
  const data=path.endsWith('/session')?{id:'user-id',username:'tester',admin:false,csrf:'csrf'}:path.endsWith(`/problems/${id}`)?problem:[];
  return route.fulfill({status:200,contentType:'application/json',body:JSON.stringify(data)});
 });
 await page.emulateMedia({colorScheme:'dark'});
 await page.goto(`/problems/${id}`);
 await expect(page.getByRole('heading',{name:'UI sample'})).toBeVisible();
 await expect(page.getByText('Use addition.')).toHaveCount(0);
 await page.getByRole('button',{name:'Reveal hint 1'}).click();
 await expect(page.getByText('Use addition.')).toBeVisible();
 await expect(page.locator('.hint-list script')).toHaveCount(0);
 await expect(page.getByText('Return the result.')).toHaveCount(0);
 await page.getByRole('button',{name:'Reveal hint 2'}).click();
 await expect(page.getByText('Return the result.')).toBeVisible();
 await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
 await page.emulateMedia({colorScheme:'light'});
 await expect(page.locator('html')).toHaveAttribute('data-theme','light');
 await expect(page.getByRole('combobox',{name:'Theme',exact:true})).not.toBeVisible();
 await page.getByLabel('User menu').click();
 await selectValue(page.getByRole('combobox',{name:'Theme',exact:true}),'dark');await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
 await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
 const darkDropdown=await dropdownStyle('Theme');
 expect(darkDropdown.scheme).toBe('dark');
 expect(darkDropdown.optionBackground).toBe(darkDropdown.background);
 expect(darkDropdown.optionText).toBe(darkDropdown.text);
 await page.getByLabel('User menu').click();
 await expect(page.locator('.problem-metadata .badge')).toContainText('medium');
 await expect(page.locator('.problem-metadata .tags')).toContainText('arrays');
 await expect(page.locator('.statement-pane')).not.toContainText('Standard libraries');
 await expect(page.getByRole('button',{name:'Reconnect'})).toHaveCount(0);
 await page.getByRole('button',{name:'Enable blind mode'}).click();
 await expect(page.locator('.problem-metadata')).toHaveCount(0);
 await expect(page.getByRole('button',{name:'Show problem details'})).toHaveAttribute('aria-pressed','true');
 await page.getByRole('button',{name:'Show problem details'}).click();
 await expect(page.locator('.problem-metadata')).toBeVisible();
 await expect(page.getByRole('button',{name:'Sign out'})).not.toBeVisible();
 await page.getByLabel('User menu').click();
 await expect(page.getByRole('button',{name:'Sign out'})).toBeVisible();
 await page.getByLabel('User menu').click();

 await expect(page.getByRole('button',{name:'Reset code'})).toBeEnabled({timeout:45000});
 await expect(page.locator('.editor-toolbar .filename-status .cloud-status')).toHaveAttribute('aria-label','Saved to cloud');
 await expect(page.locator('.editor-toolbar .filename-status .semantic')).toBeVisible();
 await expect(page.locator('.code-pane > .semantic')).toHaveCount(0);
 const editorBackground=()=>page.locator('.monaco-editor').first().evaluate(node=>getComputedStyle(node).backgroundColor);
 const darkEditorBackground=await editorBackground();
 expect(darkEditorBackground).toBe('rgb(25, 25, 28)');
 expect(await page.locator('.monaco-editor .view-lines').first().evaluate(node=>getComputedStyle(node).fontFamily)).toContain('JetBrains Mono');
 expect(await dropdownStyle('Language')).toEqual(darkDropdown);
 await page.getByLabel('User menu').click();
 await selectValue(page.getByRole('combobox',{name:'Theme',exact:true}),'light');await expect(page.locator('html')).toHaveAttribute('data-theme','light');
 await expect.poll(editorBackground).not.toBe(darkEditorBackground);
 const lightDropdown=await dropdownStyle('Theme');
 expect(lightDropdown.scheme).toBe('light');
 expect(lightDropdown.background).not.toBe(darkDropdown.background);
 expect(lightDropdown.optionBackground).toBe(lightDropdown.background);
 expect(await dropdownStyle('Language')).toEqual(lightDropdown);
 await selectValue(page.getByRole('combobox',{name:'Theme',exact:true}),'dark');await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
 await expect.poll(editorBackground).toBe(darkEditorBackground);
 await page.getByLabel('User menu').click();
 const source=page.locator('.monaco-editor .view-lines').first();
 await expect(source).toContainText('int solve()');
 await page.locator('.monaco-editor').first().click();
 await page.keyboard.press('ControlOrMeta+A');
 await page.keyboard.insertText('int solve() { return 7; }');
 await expect(source).toContainText('return 7');
 page.once('dialog',dialog=>void dialog.dismiss());
 await page.getByRole('button',{name:'Reset code'}).click();
 await expect(source).toContainText('return 7');
 page.once('dialog',dialog=>{expect(dialog.message()).toContain('C++');void dialog.accept()});
 await page.getByRole('button',{name:'Reset code'}).click();
 await expect(source).toContainText('return 1');
 await expect.poll(()=>page.evaluate(v=>localStorage.getItem(`practice:user-id:${v}:cpp`),version)).toBe(problem.starters.cpp);
 await page.reload();
 await expect(page.locator('.monaco-editor .view-lines').first()).toContainText('return 1',{timeout:45000});
 await selectValue(page.getByRole('combobox',{name:'Language',exact:true}),'python');
 await expect(source).toContainText('def solve()');
 await page.locator('.monaco-editor').first().click();
 await page.keyboard.press('ControlOrMeta+A');
 await page.keyboard.insertText('def solve():\n    return 7');
 await expect(source).toContainText('return 7');
 page.once('dialog',dialog=>{expect(dialog.message()).toContain('Python');void dialog.accept()});
 await page.getByRole('button',{name:'Reset code'}).click();
 await expect(source).toContainText('return 1');
 await expect.poll(()=>page.evaluate(v=>localStorage.getItem(`practice:user-id:${v}:python`),version)).toBe(problem.starters.python);

 for(const [name,panel,dx,dy] of [
  ['Resize description and code','.statement-pane',120,0],
  ['Resize code and results','.results-split',0,-80],
  ['Resize history and result details','.history-panel',60,0]
 ] as const){
  const handle=page.getByRole('button',{name});
  const before=await page.locator(panel).evaluate((node,vertical)=>vertical?node.getBoundingClientRect().height:node.getBoundingClientRect().width,dy!==0);
  const box=await handle.boundingBox();expect(box).not.toBeNull();
  await page.mouse.move(box!.x+box!.width/2,box!.y+box!.height/2);
  await page.mouse.down();
  await page.mouse.move(box!.x+box!.width/2+dx,box!.y+box!.height/2+dy,{steps:5});
  await page.mouse.up();
  const after=await page.locator(panel).evaluate((node,vertical)=>vertical?node.getBoundingClientRect().height:node.getBoundingClientRect().width,dy!==0);
  expect(after).toBeGreaterThan(before+30);
 }
 // Review the workspace at the same responsive widths as the catalog.
 await page.locator('.monaco-editor').first().click();
 await page.keyboard.press('ControlOrMeta+A');
 await page.keyboard.insertText('def solve():\n    # != == => <= >= -> ===\n    return 1');
 for(const width of [1440,1024,768,375]){
  await page.setViewportSize({width,height:900});
  await expect.poll(()=>page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  if(width<=800)expect(await page.locator('.workspace').evaluate(node=>node.getBoundingClientRect().height)).toBeGreaterThan(900);
  await page.screenshot({path:`/tmp/j0coder-editor-${width}.png`,fullPage:true});
 }
 await page.setViewportSize({width:1440,height:900});
 await page.getByRole('button',{name:'Problems',exact:true}).click();
 await expect(page.getByRole('combobox',{name:'Difficulty',exact:true})).toBeVisible();
 expect(await dropdownStyle('Difficulty')).toEqual(darkDropdown);
});
