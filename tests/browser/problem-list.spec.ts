import {test} from './account-fixtures';
import {expect} from '../../web/node_modules/@playwright/test/index';

test('the full catalog stays in a viewport-sized virtual list',async({page})=>{
 const rows=Array.from({length:245},(_,i)=>({
  id:`00000000-0000-4000-8000-${String(i).padStart(12,'0')}`,
  version:`00000000-0000-4000-8001-${String(i).padStart(12,'0')}`,
  title:`Problem ${String(i).padStart(3,'0')}`,
  summary:`Summary for problem ${i}`,
  difficulty:'easy',difficulty_score:2,tags:['arrays'],
  cursor:`Problem ${String(i).padStart(3,'0')}|00000000-0000-4000-8000-${String(i).padStart(12,'0')}`
 }));
 let pages=0;
 await page.route('**/api/v1/session',route=>route.fulfill({json:{username:'student',admin:false,csrf:'test'}}));
 await page.route('**/api/v1/problems?*',route=>{
  const params=new URL(route.request().url()).searchParams;
  const cursor=params.get('cursor');
  const start=cursor?rows.findIndex(row=>row.cursor===cursor)+1:0;
  pages++;
  return route.fulfill({json:rows.slice(start,start+Number(params.get('limit')||100))});
 });
 await page.goto('/');
 await expect(page.locator('.count')).toHaveText('245 problems');
 expect(pages).toBe(3);
 const list=page.getByRole('region',{name:'Problem list'});
 expect(await list.evaluate(node=>node.scrollHeight>node.clientHeight)).toBe(true);
 expect(await page.locator('.problem-row').count()).toBeLessThan(30);
 expect(await page.locator('.problems-page').evaluate(node=>node.getBoundingClientRect().bottom)).toBeLessThanOrEqual(page.viewportSize()!.height);
 await list.evaluate(node=>node.scrollTop=node.scrollHeight);
 await expect(page.getByRole('button',{name:/Problem 244/})).toBeVisible();
 expect(await page.locator('.problem-row').count()).toBeLessThan(30);
 await page.getByLabel('Search problems').fill('Problem 244');
 await expect(page.getByRole('button',{name:/Problem 244/})).toBeVisible();
 expect(await list.evaluate(node=>node.scrollTop)).toBe(0);
});
