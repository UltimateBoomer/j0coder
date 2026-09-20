import {test,expect,type Page} from '../../web/node_modules/@playwright/test/index';

async function signIn(page:Page,username='admin',password='Integration-password-123'){
 await page.getByLabel('Username',{exact:true}).fill(username);
 await page.getByLabel('Password',{exact:true}).fill(password);
 await page.getByRole('button',{name:'Sign in'}).click();
}

test('routes preserve authentication intent, filters, and browser history',async({page})=>{
 await page.goto('/');await signIn(page);await expect(page.getByRole('heading',{name:'Problems'})).toBeVisible();
 const session=await (await page.request.get('/api/v1/session')).json();
 const headers={'Origin':process.env.TEST_ORIGIN||'http://127.0.0.1:18080','X-CSRF-Token':session.csrf};
 const definition={title:`Routing sample ${Date.now()}`,statement:'Return the input.',difficulty:'medium',tags:['routing'],signature:{method:'echo',params:[{name:'value',ty:'string'}],returns:'string'},limits:{time_ms:2000,memory_mib:256,output_bytes:1048576},tests:[{args:['hello'],expected:'hello',hidden:false}]};
 const draft=await (await page.request.post('/api/v1/admin/problems',{headers,data:definition})).json();
 await page.request.post(`/api/v1/admin/problems/${draft.id}/publish`,{headers});

 await page.getByRole('button',{name:'Sign out'}).click();
 await page.goto(`/problems/${draft.id}`);expect(page.url()).toContain(`/problems/${draft.id}`);
 await signIn(page);await expect(page.getByRole('heading',{name:definition.title})).toBeVisible();
 await page.getByRole('button',{name:'Problems',exact:true}).click();
 await page.getByLabel('Search problems').fill('Routing sample');await page.getByLabel('Difficulty').selectOption('medium');await page.getByLabel('Tag').fill('routing');await page.getByRole('button',{name:'Search'}).click();
 await expect(page).toHaveURL(/\?q=Routing(\+|%20)sample&difficulty=medium&tag=routing$/);
 await page.getByRole('button',{name:new RegExp(definition.title)}).click();await page.goBack();
 await expect(page.getByLabel('Search problems')).toHaveValue('Routing sample');await expect(page.getByLabel('Difficulty')).toHaveValue('medium');await expect(page.getByLabel('Tag')).toHaveValue('routing');
 await page.goForward();await expect(page.getByRole('heading',{name:definition.title})).toBeVisible();
 await page.reload();await expect(page.getByRole('heading',{name:definition.title})).toBeVisible();
});

test('admin routes and route error states are addressable',async({page})=>{
 await page.goto('/');await signIn(page);await page.getByRole('button',{name:'Authoring'}).click();await expect(page).toHaveURL(/\/admin\/problems$/);
 const session=await (await page.request.get('/api/v1/session')).json();const headers={'Origin':process.env.TEST_ORIGIN||'http://127.0.0.1:18080','X-CSRF-Token':session.csrf};
 await page.getByRole('button',{name:'+ New problem'}).click();await expect(page).toHaveURL(/\/admin\/problems\/new$/);
 const title=`Unsaved route ${Date.now()}`;const textarea=page.getByLabel('Problem definition (JSON)');const value=JSON.parse(await textarea.inputValue());value.title=title;await textarea.fill(JSON.stringify(value));await page.getByRole('button',{name:'Save draft'}).click();
 await expect(page).toHaveURL(/\/admin\/problems\/[0-9a-f-]+$/);await expect(page.getByRole('status')).toContainText('Draft saved');await page.reload();await expect(textarea).toHaveValue(new RegExp(title));
 await page.goto('/unknown/path');await expect(page.getByRole('heading',{name:'Page not found'})).toBeVisible();
 await page.goto('/problems/not-a-uuid');await expect(page.getByRole('heading',{name:'Page not found'})).toBeVisible();
 await page.goto('/problems/00000000-0000-4000-8000-000000000000');await expect(page.getByRole('heading',{name:'Page not found'})).toBeVisible();
 const student=`route${Date.now()}`;await page.request.post('/api/v1/admin/users',{headers,data:{username:student,password:'Routing-student-password'}});await page.getByRole('button',{name:'Sign out'}).click();await page.goto('/admin/problems');await signIn(page,student,'Routing-student-password');await expect(page.getByRole('heading',{name:'Access denied'})).toBeVisible();
});
