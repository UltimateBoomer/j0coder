import {test} from './account-fixtures';
import {expect} from '../../web/node_modules/@playwright/test/index';

async function catalog(page:import('../../web/node_modules/@playwright/test/index').Page){
 await page.route('**/api/v1/session',route=>route.fulfill({json:{id:'student',username:'student',admin:false,csrf:'test'}}));
 await page.route('**/api/v1/problems?*',route=>route.fulfill({json:[]}));
 await page.goto('/');
 await expect(page.getByRole('heading',{name:'Problems',exact:true})).toBeVisible();
}
test('catalog animation pause persists locally',async({page})=>{
 await catalog(page);
 await page.getByLabel('User menu').click();
 const toggle=page.getByRole('button',{name:'Pause background animation'});
 const layer=page.locator('.catalog-gradient').first();
 await expect(toggle).toHaveAttribute('aria-pressed','false');
 expect(await layer.evaluate(node=>getComputedStyle(node).animationDuration)).toBe('32s');
 await toggle.click();
 await expect(toggle).toHaveAttribute('aria-pressed','true');
 expect(await layer.evaluate(node=>getComputedStyle(node).animationPlayState)).toBe('paused');
 await page.reload();
 await page.getByLabel('User menu').click();
 await expect(toggle).toHaveAttribute('aria-pressed','true');
 expect(await page.evaluate(()=>localStorage.getItem('j0coder:catalog-motion'))).toBe('paused');
 await toggle.click();
 expect(await page.evaluate(()=>localStorage.getItem('j0coder:catalog-motion'))).toBe('running');
 expect(await layer.evaluate(node=>getComputedStyle(node).animationPlayState)).toBe('running');
});
test('reduced motion overrides the running preference',async({page})=>{
 await page.emulateMedia({reducedMotion:'reduce'});
 await catalog(page);
 await page.getByLabel('User menu').click();
 const layer=page.locator('.catalog-gradient').first();
 expect(await layer.evaluate(node=>getComputedStyle(node).animationName)).toBe('none');
 await page.getByRole('button',{name:'Pause background animation'}).click();
 await page.getByRole('button',{name:'Pause background animation'}).click();
 expect(await layer.evaluate(node=>getComputedStyle(node).animationName)).toBe('none');
 await page.emulateMedia({reducedMotion:'no-preference'});
 expect(await layer.evaluate(node=>getComputedStyle(node).animationName)).toBe('catalog-drift');
});
