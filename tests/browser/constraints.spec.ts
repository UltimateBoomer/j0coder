import {test} from './account-fixtures';
import {expect} from '../../web/node_modules/@playwright/test/index';

const ids={function:'00000000-0000-4000-8000-000000000101',stateful:'00000000-0000-4000-8000-000000000102',legacy:'00000000-0000-4000-8000-000000000103'};
const limits={time_ms:2000,memory_mib:256,output_bytes:1048576};
const base={schema:3,title:'Constraint sample',statement:'## Task\nSolve this example.',difficulty:'easy',tags:[],limits,tests:[]};
const functionProblem={...base,interface:{kind:'function',name:'solve',params:[
 {name:'values',ty:{array:'int'},constraints:'At most **100** values. <script>window.pwned=true</script>'},
 {name:'unused',ty:'int'},
 {name:'target',ty:'int',constraints:'Between `-10` and `10`.'}
],returns:'int'}};
const statefulProblem={...base,interface:{kind:'data_structure',name:'Counter',constructor:{params:[{name:'start',ty:'int',constraints:'Start at `0` or higher.'}]},methods:[
 {name:'add',params:[{name:'delta',ty:'int',constraints:'Positive values only.'}],returns:'void'},
 {name:'get',params:[{name:'unused',ty:'int'}],returns:'int'}
]}};
const legacyProblem={...base,interface:{kind:'function',name:'solve',params:[{name:'value',ty:'int'}],returns:'int'}};

test.beforeEach(async({page})=>{
 await page.route('**/api/v1/**',route=>{
  const path=new URL(route.request().url()).pathname;
  if(path.endsWith("/capabilities")||path.includes("/me/"))return route.fallback();
  if(path.endsWith('/session'))return route.fulfill({json:{id:'user-id',username:'admin',admin:true,csrf:'test'}});
  if(path.endsWith('/admin/problems'))return route.fulfill({json:[]});
  if(path.endsWith('/admin/catalog'))return route.fulfill({json:{state:{},runs:[],settings:null}});
  if(path.includes('/solutions/'))return route.fulfill({status:404,json:{error:'not found'}});
  const definitions:Record<string,unknown>={[ids.function]:functionProblem,[ids.stateful]:statefulProblem,[ids.legacy]:legacyProblem};
  const id=path.split('/').at(-1)??'';
  if(id in definitions)return route.fulfill({json:{id,version:'00000000-0000-4000-8000-000000000104',problem:definitions[id],starters:{cpp:'int solve() { return 0; }',python:'def solve():\n    return 0'}}});
  return route.fulfill({json:[]});
 });
});

test('learner constraints are ordered, sanitized, grouped, and optional',async({page})=>{
 await page.goto(`/problems/${ids.function}`);
 const constraints=page.getByRole('region',{name:'Constraints'});
 await expect(constraints).toBeVisible();
 await expect(constraints.locator('dt')).toHaveText(['values','target']);
 await expect(constraints.locator('strong')).toHaveText('100');
 await expect(constraints.locator('script')).toHaveCount(0);
 expect(await page.evaluate(()=>(window as any).pwned)).toBeUndefined();
 await expect(page.locator('.statement-pane')).toContainText('Solve this example.');

 await page.goto(`/problems/${ids.stateful}`);
 const stateful=page.getByRole('region',{name:'Constraints'});
 await expect(stateful.locator('h4')).toHaveText(['Constructor','add()']);
 await expect(stateful.locator('dt')).toHaveText(['start','delta']);

 await page.goto(`/problems/${ids.legacy}`);
 await expect(page.getByRole('region',{name:'Constraints'})).toHaveCount(0);
});

test('admin preview uses the same constraints section',async({page})=>{
 await page.goto('/admin/problems/new');
 await page.getByLabel('Problem definition (JSON)').fill(JSON.stringify(functionProblem));
 await page.getByRole('button',{name:'Preview statement'}).click();
 const constraints=page.getByRole('region',{name:'Constraints'});
 await expect(constraints.locator('dt')).toHaveText(['values','target']);
 await expect(constraints.locator('script')).toHaveCount(0);

 await page.getByLabel('Problem definition (JSON)').fill(JSON.stringify(legacyProblem));
 await page.getByRole('button',{name:'Preview statement'}).click();
 await expect(page.getByRole('region',{name:'Constraints'})).toHaveCount(0);
});
