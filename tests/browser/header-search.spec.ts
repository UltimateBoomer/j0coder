import {test} from './account-fixtures';
import {expect,type Page} from '../../web/node_modules/@playwright/test/index';

const id=(index:number)=>`00000000-0000-4000-8000-${String(index).padStart(12,'0')}`;
const rows=Array.from({length:245},(_,index)=>({
 id:id(index),version:`00000000-0000-4000-8001-${String(index).padStart(12,'0')}`,
 title:`Problem ${String(index).padStart(3,'0')}`,summary:`Summary for problem ${index}`,
 difficulty:'easy',difficulty_score:2,tags:['arrays'],
 cursor:`Problem ${String(index).padStart(3,'0')}|${id(index)}`
}));

async function mockApp(page:Page){
 let catalog=rows;
 let drafts:any[]=[];
 let pageRequests=0;
 await page.route('**/api/v1/session',route=>route.fulfill({json:{id:id(999),username:'admin',admin:true,csrf:'test'}}));
 await page.route('**/api/v1/problems?*',route=>{
  const params=new URL(route.request().url()).searchParams;
  const cursor=params.get('cursor');
  const start=cursor?catalog.findIndex(row=>row.cursor===cursor)+1:0;
  pageRequests++;
  return route.fulfill({json:catalog.slice(start,start+Number(params.get('limit')||100))});
 });
 await page.route('**/api/v1/problems/*',route=>{
  const problemId=route.request().url().split('/').pop();
  const summary=catalog.find(row=>row.id===problemId)!;
  return route.fulfill({json:{id:summary.id,version:summary.version,problem:{title:summary.title,statement:'A sample problem.',difficulty:'easy',difficulty_score:2,tags:['arrays'],tests:[],limits:{time_ms:2000,memory_mib:256},interface:{kind:'function',name:'solve',params:[],returns:'int'}},starters:{cpp:'int solve() { return 0; }',python:'def solve():\n    return 0'}}});
 });
 await page.route('**/api/v1/submissions?*',route=>route.fulfill({json:[]}));
 await page.route('**/api/v1/admin/problems',route=>route.fulfill({json:drafts}));
 await page.route('**/api/v1/admin/catalog',route=>route.fulfill({json:{settings:null,state:{},runs:[]}}));
 return {requests:()=>pageRequests,add:(row:typeof rows[number])=>catalog=[...catalog,row],setDrafts:(value:any[])=>drafts=value};
}

test('header search works on problem and authoring pages with keyboard and pointer navigation',async({page})=>{
 const catalog=await mockApp(page);
 await page.goto(`/problems/${id(0)}`);
 await expect(page.getByRole('heading',{name:'Problem 000'})).toBeVisible();
 const search=page.getByRole('combobox',{name:'Find a problem'});
 await search.fill('Problem 244');
 await expect(page.getByRole('option',{name:/Problem 244/})).toBeVisible();
 expect(catalog.requests()).toBe(3);
 await search.press('Enter');
 await expect(page).toHaveURL(new RegExp(`/problems/${id(244)}$`));
 await expect(search).toHaveValue('');
 await expect(search).toHaveAttribute('aria-expanded','false');
 await page.getByRole('button',{name:'Authoring'}).click();
 await expect(page.getByRole('heading',{name:'Problem studio'})).toBeVisible();
 await search.fill('Problem 120');
 await expect(page.getByRole('option',{name:/Problem 120/})).toBeVisible();
 await search.press('Escape');
 await expect(search).toHaveAttribute('aria-expanded','false');
 await search.press('ArrowDown');
 await expect(page.getByRole('option',{name:/Problem 120/})).toBeVisible();
 await page.mouse.click(page.viewportSize()!.width-20,page.viewportSize()!.height-20);
 await expect(search).toHaveAttribute('aria-expanded','false');
 await search.focus();
 await page.getByRole('option',{name:/Problem 120/}).click();
 await expect(page).toHaveURL(new RegExp(`/problems/${id(120)}$`));
 expect(catalog.requests()).toBe(3);
});

test('header results are ranked, independent of page filters, and virtualized while scrolling',async({page})=>{
 await mockApp(page);
 await page.goto('/');
 await expect(page.locator('.count')).toHaveText('245 problems');
 await page.getByLabel('Search problems',{exact:true}).fill('Problem 001');
 await page.getByLabel('Difficulty').selectOption('hard');
 await expect(page.locator('.count')).toHaveText('0 problems');
 const search=page.getByRole('combobox',{name:'Find a problem'});
 await search.fill('Problem');
 const results=page.getByRole('listbox',{name:'Problem search results'});
 await expect(results).toBeVisible();
 expect(await results.evaluate(node=>node.scrollHeight>node.clientHeight)).toBe(true);
 expect(await results.getByRole('option').count()).toBeLessThan(30);
 await results.evaluate(node=>node.scrollTop=node.scrollHeight);
 await expect(page.getByRole('option',{name:/Problem 244/})).toBeVisible();
 expect(await results.getByRole('option').count()).toBeLessThan(30);
 await search.fill('Problem 120');
 await expect(results.getByRole('option').first()).toContainText('Problem 120');
 expect(await results.evaluate(node=>node.scrollTop)).toBe(0);
 await expect(page.locator('.count')).toHaveText('0 problems');
 await page.setViewportSize({width:390,height:844});
 const bounds=await page.locator('.quick-dropdown').boundingBox();
 expect(bounds).not.toBeNull();
 expect(bounds!.x).toBeGreaterThanOrEqual(0);
 expect(bounds!.x+bounds!.width).toBeLessThanOrEqual(390);
 expect(bounds!.y+bounds!.height).toBeLessThanOrEqual(844);
 await expect(search).toBeVisible();
 await search.press('ArrowDown');
 await expect(search).toHaveAttribute('aria-activedescendant',new RegExp('header-problem-'));
 await search.press('ArrowUp');
 await expect(search).toHaveAttribute('aria-activedescendant',`header-problem-${id(120)}`);
 await search.press('Enter');
 await expect(page).toHaveURL(new RegExp(`/problems/${id(120)}$`));
});

test('dismissed search reopens at the top and closes when focus leaves',async({page})=>{
 await mockApp(page);
 await page.goto('/');
 await expect(page.locator('.count')).toHaveText('245 problems');
 const search=page.getByRole('combobox',{name:'Find a problem'});
 const results=page.getByRole('listbox',{name:'Problem search results'});
 await search.fill('Problem');
 await results.evaluate(node=>node.scrollTop=node.scrollHeight);
 await expect.poll(()=>results.getByRole('option').first().getAttribute('aria-posinset')).not.toBe('1');
 await search.press('Escape');
 await search.press('ArrowDown');
 await expect(results.getByRole('option').first()).toHaveAttribute('aria-posinset','1');
 expect(await results.evaluate(node=>node.scrollTop)).toBe(0);
 await results.evaluate(node=>node.scrollTop=node.scrollHeight);
 await expect.poll(()=>results.getByRole('option').first().getAttribute('aria-posinset')).not.toBe('1');
 await page.mouse.click(page.viewportSize()!.width-20,page.viewportSize()!.height-20);
 await search.focus();
 await expect(results.getByRole('option').first()).toHaveAttribute('aria-posinset','1');
 expect(await results.evaluate(node=>node.scrollTop)).toBe(0);
 await search.press('Tab');
 await expect(search).toHaveAttribute('aria-expanded','false');
 await page.getByLabel('Search problems',{exact:true}).focus();
 await expect(results).toHaveCount(0);
});

test('header search shows a fetch error, retries, and reports no matches',async({page})=>{
 await mockApp(page);
 let allowRetry=false;
 await page.route('**/api/v1/problems?*',route=>{
  if(!allowRetry)return route.fulfill({status:500,json:{error:'Catalog unavailable'}});
  return route.fallback();
 });
 await page.goto('/admin/problems');
 const search=page.getByRole('combobox',{name:'Find a problem'});
 await search.fill('no-such-problem-query');
 await expect(page.locator('.quick-state')).toContainText('Could not load problems.');
 allowRetry=true;
 await page.getByRole('button',{name:'Retry'}).click();
 await expect(page.getByText('No matching problems.')).toBeVisible();
});

test('publishing refreshes the header search catalog',async({page})=>{
 const catalog=await mockApp(page);
 const newRow={...rows[0],id:id(800),version:id(801),title:'Newly published challenge',cursor:`Newly published challenge|${id(800)}`};
 catalog.setDrafts([{id:id(800),draft:{title:newRow.title},managed:false,version:null}]);
 await page.route('**/api/v1/admin/problems/*',route=>route.fulfill({json:{}}));
 await page.route('**/api/v1/admin/problems/*/publish',route=>{catalog.add(newRow);return route.fulfill({json:{version:id(801)}})});
 await page.goto(`/admin/problems/${id(800)}`);
 const search=page.getByRole('combobox',{name:'Find a problem'});
 await search.fill('Newly published');
 await expect(page.getByText('No matching problems.')).toBeVisible();
 await page.getByRole('button',{name:'Validate & publish'}).click();
 await expect(page.getByRole('status')).toContainText('Published immutable version');
 await search.focus();
 await expect(page.getByRole('option',{name:/Newly published challenge/})).toBeVisible();
 expect(catalog.requests()).toBe(6);
});
