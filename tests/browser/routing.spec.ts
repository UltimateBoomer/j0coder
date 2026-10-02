import {test,createProblem,createUser,adminOrigin,publicOrigin} from './host-fixtures';
import {expect,type Page} from '../../web/node_modules/@playwright/test/index';

async function signIn(page:Page,username='admin',password='Integration-password-123'){
 await page.getByLabel('Username',{exact:true}).fill(username);
 await page.getByLabel('Password',{exact:true}).fill(password);
 await page.getByRole('button',{name:'Sign in →',exact:true}).click();
}

test('routes preserve authentication intent, filters, and browser history',async({page})=>{
 await page.goto('/');await signIn(page);await expect(page.getByRole('heading',{name:'Problems'})).toBeVisible();
 const session=await (await page.request.get('/api/v1/session')).json();
 const headers={'Origin':process.env.TEST_ORIGIN||'http://127.0.0.1:18080','X-CSRF-Token':session.csrf};
 const uniqueTag=`route-tag-${Date.now()}`;
 const definition={title:`Routing sample ${Date.now()}`,statement:'Return the input.',difficulty:'medium',tags:[uniqueTag],schema:3,interface:{kind:'function',name:'echo',params:[{name:'value',ty:'string'}],returns:'string'},limits:{time_ms:2000,memory_mib:256,output_bytes:1048576},tests:[{args:['hello'],expected:'hello',hidden:false}]};
 const draft=createProblem(definition);

 await page.getByLabel('User menu').click();await page.getByRole('button',{name:'Sign out'}).click();await expect(page.getByLabel('Username',{exact:true})).toBeVisible();
 await page.goto(`/problems/${draft.id}`);expect(page.url()).toContain(`/problems/${draft.id}`);
 await signIn(page);await expect(page.getByRole('heading',{name:definition.title})).toBeVisible();
 await page.getByRole('button',{name:'Problems',exact:true}).click();
 const result=page.getByRole('button',{name:new RegExp(definition.title)});
 await page.getByLabel('Search problems').fill('Routng sample');await expect(result).toBeVisible();
 await page.getByLabel('Search problems').fill(uniqueTag);await expect(result).toBeVisible();
 await page.getByLabel('Difficulty').selectOption('hard');await expect(result).toHaveCount(0);await expect(page.locator('.count')).toHaveText('0 problems');await expect(page.getByText('No matching problems.')).toBeVisible();
 await page.getByLabel('Difficulty').selectOption('medium');await expect(result).toBeVisible();
 await page.getByLabel('Search problems').fill('no-such-problem-query');await expect(page.locator('.count')).toHaveText('0 problems');
 await page.getByLabel('Search problems').fill('');await page.getByLabel('Difficulty').selectOption('');await expect(page).toHaveURL(/\/$/);await expect(result).toBeVisible();
 const titles=await page.locator('.problem-title').evaluateAll(nodes=>nodes.map(node=>node.childNodes[0]?.textContent||''));expect(titles).toEqual([...titles].sort((a,b)=>a.localeCompare(b)));
 await page.getByLabel('Search problems').fill('Routing sample');await page.getByLabel('Difficulty').selectOption('medium');
 await expect(page).toHaveURL(/\?q=Routing(\+|%20)sample&difficulty=medium$/);
 await page.getByRole('button',{name:new RegExp(definition.title)}).click();await page.goBack();
 await expect(page.getByLabel('Search problems')).toHaveValue('Routing sample');await expect(page.getByLabel('Difficulty')).toHaveValue('medium');await expect(page.getByLabel('Tag')).toHaveCount(0);
 await page.goForward();await expect(page.getByRole('heading',{name:definition.title})).toBeVisible();
 await page.reload();await expect(page.getByRole('heading',{name:definition.title})).toBeVisible();
 await page.goto(`/?q=Routng%20sample&difficulty=medium&tag=${uniqueTag}`);await expect(result).toBeVisible();await expect(page.getByLabel('Search problems')).toHaveValue('Routng sample');
});

test('admin routes and route error states are addressable',async({page})=>{
 await page.goto(adminOrigin+'/');await signIn(page);await page.getByRole('button',{name:'Authoring'}).click();await expect(page).toHaveURL(/\/admin\/problems$/);
 const session=await (await page.request.get('/api/v1/session')).json();const headers={'Origin':process.env.TEST_ORIGIN||'http://127.0.0.1:18080','X-CSRF-Token':session.csrf};
 await page.getByRole('button',{name:'+ New problem'}).click();await expect(page).toHaveURL(/\/admin\/problems\/new$/);
 const title=`Unsaved route ${Date.now()}`;const textarea=page.getByLabel('Problem definition (JSON)');const value=JSON.parse(await textarea.inputValue());value.title=title;await textarea.fill(JSON.stringify(value));await page.getByRole('button',{name:'Save draft'}).click();
 await expect(page).toHaveURL(/\/admin\/problems\/[0-9a-f-]+$/);await expect(page.getByRole('status')).toContainText('Draft saved');await page.reload();await expect(textarea).toHaveValue(new RegExp(title));
 await page.goto(adminOrigin+'/unknown/path');await expect(page.getByRole('heading',{name:'Page not found'})).toBeVisible();
 await page.goto(adminOrigin+'/problems/not-a-uuid');await expect(page.getByRole('heading',{name:'Page not found'})).toBeVisible();
 await page.goto(adminOrigin+'/problems/00000000-0000-4000-8000-000000000000');await expect(page.getByRole('heading',{name:'Page not found'})).toBeVisible();
 const student=`route${Date.now()}`;createUser(student,'Routing-student-password');await page.getByLabel('User menu').click();await page.getByRole('button',{name:'Sign out'}).click();await expect(page.getByLabel('Username',{exact:true})).toBeVisible();await page.goto(publicOrigin+'/admin/problems');await signIn(page,student,'Routing-student-password');await expect(page.getByRole('heading',{name:'Access denied'})).toBeVisible();
});
