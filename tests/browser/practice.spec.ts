import {selectValue} from './select-helpers';
import {test,createProblem,createUser,adminOrigin} from './host-fixtures';
import {expect} from '../../web/node_modules/@playwright/test/index';
test('admin publishes sanitized statements and edits both languages',async({page})=>{
 const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(adminOrigin+'/');await expect(page.locator('body')).not.toContainText('YOUR SPACE TO THINK');await expect(page.locator('body')).not.toContainText('Make progress.');await expect(page.locator('body')).not.toContainText('A private workspace');await page.getByLabel('Username',{exact:true}).fill('admin');await page.getByLabel('Password',{exact:true}).fill('Integration-password-123');await page.getByRole('button',{name:'Sign in →',exact:true}).click();
 await expect(page.getByRole('heading',{name:'Problems'})).toBeVisible();await expect(page.locator('body')).not.toContainText('THE PRACTICE ROOM');await expect(page.locator('body')).not.toContainText('Find your next challenge.');await expect(page.locator('body')).not.toContainText('Read carefully.');await page.getByRole('button',{name:'Authoring',exact:true}).click();await expect(page.locator('body')).not.toContainText('ADMINISTRATION');await expect(page.locator('body')).not.toContainText('Save a working draft, validate its contract');await page.getByRole('button',{name:'+ New problem'}).click();
 const title='Browser sample '+Date.now();const definition={title,statement:'## A safe statement\nReturn the input. <img src=x onerror="window.pwned=true"><script>window.pwned=true</script>',difficulty:'easy',tags:['browser'],schema:3,interface:{kind:'function',name:'echo',params:[{name:'value',ty:'string'}],returns:'string'},limits:{time_ms:2000,memory_mib:256,output_bytes:1048576},tests:[{args:['你好'],expected:'你好',hidden:false},{args:['secret-hidden-case'],expected:'secret-hidden-case',hidden:true}]};
 await page.getByLabel('Problem definition (JSON)').fill(JSON.stringify(definition));await page.getByRole('button',{name:'Preview statement'}).click();await expect(page.getByRole('heading',{name:'A safe statement'})).toBeVisible();expect(await page.evaluate(()=>(window as any).pwned)).toBeUndefined();await page.getByRole('button',{name:'Validate & publish'}).click();await expect(page.getByRole('status')).toContainText('Published immutable version');
 await page.getByRole('button',{name:'Problems',exact:true}).click();await page.getByLabel('Search problems').fill(title);await page.getByRole('button',{name:new RegExp(title)}).click();await expect(page.getByRole('heading',{name:title})).toBeVisible();await expect(page.locator('body')).not.toContainText('Run examples to explore your solution');await expect(page.locator('.monaco-editor').first()).toBeVisible();await expect(page.getByRole('button',{name:'Run',exact:true})).toBeEnabled({timeout:45000});await expect(page.getByRole('button',{name:/Submit/})).toHaveCount(0);
 const editor=page.locator('.monaco-editor').first();const editorText=async()=>editor.locator('.view-line').allTextContents().then(lines=>lines.join('\n').replaceAll('\u00a0',' '));const clearEditor=async()=>{await editor.click();await page.keyboard.press('ControlOrMeta+A');await page.keyboard.press('Backspace')};
 for(const language of ['python','cpp']){
  await selectValue(page.getByRole('combobox',{name:'Language',exact:true}),language);await expect(editor).toBeVisible();await expect(page.getByRole('combobox',{name:'Language',exact:true})).toBeEnabled();await expect(page.locator('.filename')).toHaveText(language==='python'?'solution.py':'solution.cpp');
  for(const [character,pair] of [['(', '()'],['[','[]'],['{','{}'],["'","''"],[`"`,`""`]]){await clearEditor();await page.keyboard.type(character);await expect.poll(editorText).toContain(pair)}
  await clearEditor();await page.keyboard.type(language==='python'?'if value:':'if (value) {');await page.keyboard.press('Enter');await page.keyboard.type(language==='python'?'pass':'return 0;');await expect.poll(editorText).toContain(language==='python'?'    pass':'    return 0;');
  await clearEditor();await page.keyboard.press('Tab');await page.keyboard.type('value');await expect.poll(editorText).toContain('    value');await page.keyboard.press('Shift+Tab');await expect.poll(editorText).toContain('value');await expect.poll(editorText).not.toContain('    value');
 }
 await expect(page.getByRole('heading',{name:title})).toBeVisible();expect(await page.locator('body').textContent()).not.toContain('PROBLEM / EASY');expect(await page.locator('body').textContent()).not.toContain('PROBLEM / MEDIUM');expect(await page.locator('body').textContent()).not.toContain('PROBLEM / HARD');expect(await page.locator('.eyebrow')).toHaveCount(0);expect(await page.locator('body').textContent()).not.toContain('secret-hidden-case');expect(await page.evaluate(()=>(window as any).pwned)).toBeUndefined();
 await page.screenshot({path:'/tmp/practice-editor.png',fullPage:true});expect(errors).toEqual([]);
});
test('regular user solves in Python and C++ with semantic completion',async({page})=>{
 test.skip(process.env.TEST_RUNNER!=='1','Requires deployed gVisor worker and editor services');
 test.setTimeout(240000);
 const origin=process.env.TEST_ORIGIN||'http://127.0.0.1:18080';
 await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('admin');await page.getByLabel('Password',{exact:true}).fill('Integration-password-123');await page.getByRole('button',{name:'Sign in →',exact:true}).click();await expect(page.getByRole('button',{name:'Problems',exact:true})).toBeVisible();
 const me=await (await page.request.get('/api/v1/session')).json();const student='browser'+Date.now();const headers={'Origin':origin,'X-CSRF-Token':me.csrf};
 createUser(student,'Browser-student-password');
 const problem={title:'Solve echo '+Date.now(),statement:'Return the input.',difficulty:'easy',tags:['acceptance'],schema:3,interface:{kind:'function',name:'echo',params:[{name:'value',ty:'string'}],returns:'string'},limits:{time_ms:2000,memory_mib:256,output_bytes:1048576},tests:[{args:['hello'],expected:'hello',hidden:false},{args:['private unicode 🦀'],expected:'private unicode 🦀',hidden:true}]};
 const draft=createProblem(problem);await page.getByLabel('User menu').click();await page.getByRole('button',{name:'Sign out'}).click();await page.getByLabel('Username',{exact:true}).fill(student);await page.getByLabel('Password',{exact:true}).fill('Browser-student-password');await page.getByRole('button',{name:'Sign in →',exact:true}).click();await page.getByRole('button',{name:new RegExp(problem.title)}).click();await expect(page.locator('.monaco-editor').first()).toBeVisible();
 // Each reload replaces an in-flight editor session; the final page must recover.
 for(let i=0;i<6;i++){
  await expect(page.locator('.semantic')).toHaveAttribute('aria-label',/Connecting semantic completion|Semantic completion connected|reconnecting semantic service/,{timeout:45000});
  await page.reload({waitUntil:'domcontentloaded'});
 }
 await expect(page.locator('.semantic')).toHaveAttribute('aria-label','Semantic completion connected',{timeout:120000});
 const initialEditor=page.locator('.monaco-editor').first();
 await initialEditor.click();await page.keyboard.press('ControlOrMeta+A');
 await page.keyboard.insertText('#include <vector>\nint main() { std::vector<int> values; values.');
 await page.keyboard.press('ControlOrMeta+Space');
 await expect(page.locator('.suggest-widget.visible')).toContainText('push_back',{timeout:20000});
 await page.keyboard.press('Escape');
 for(const language of ['python','cpp']){
  await selectValue(page.getByRole('combobox',{name:'Language',exact:true}),language);await expect(page.locator('.semantic')).toHaveAttribute('aria-label','Semantic completion connected',{timeout:120000});
  const editor=page.locator('.monaco-editor').first();
  await editor.click();await page.keyboard.press('ControlOrMeta+A');
  await page.keyboard.insertText(language==='python'?'value = "hello"\nvalue.':'#include <vector>\nint main() { std::vector<int> values; values.');
  await page.keyboard.press('ControlOrMeta+Space');
  await expect(page.locator('.suggest-widget.visible')).toContainText(language==='python'?'upper':'push_back',{timeout:20000});
  await page.keyboard.press('Escape');
  await page.locator('.monaco-editor').first().click();await page.keyboard.press('ControlOrMeta+A');await page.keyboard.insertText(language==='python'?'def echo(value: str) -> str:\n    return value':'#include <string>\nstd::string echo(std::string value) { return value; }');
  await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.locator('.result-panel h3')).toContainText('accepted',{timeout:60000});await expect(page.locator('.result-heading')).toContainText('2 / 2');await expect(page.getByText('Custom tests',{exact:true})).toHaveCount(0);await expect(page.getByRole('img',{name:'Visible case 1: accepted'})).toBeVisible();await expect(page.getByRole('img',{name:'Hidden case 1: accepted'})).toBeVisible();const cards=page.locator('.comparison-card');await expect(cards).toHaveCount(3);await expect(cards.nth(0)).toContainText('Input');await expect(cards.nth(0)).toContainText('["hello"]');await expect(cards.nth(1)).toContainText('Expected output');await expect(cards.nth(1)).toContainText('"hello"');await expect(cards.nth(2)).toContainText('Current output');await expect(cards.nth(2)).toContainText('"hello"');expect(await page.locator('body').textContent()).not.toContain('private unicode 🦀');
 }
});
