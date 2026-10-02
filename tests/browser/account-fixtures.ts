import {languageDescriptors} from './language-fixtures';
import {test as base} from '../../web/node_modules/@playwright/test/index';
export const preferences={theme:'system',default_language:'cpp',semantic_completion:true,font_size:14,tab_width:4,word_wrap:false,minimap:false,blind_mode:false};
export const test=base.extend<{accountDefaults:void}>({accountDefaults:[async({page},use)=>{
 let saved={...preferences};
 await page.route('**/api/v1/**',route=>{
  const path=new URL(route.request().url()).pathname;
  if(path.endsWith('/capabilities'))return route.fulfill({json:{languages:languageDescriptors,guest_browsing:false,registration:'invite',web_admin:true}});
  if(path.endsWith('/me/preferences')){if(route.request().method()==='PATCH')saved={...saved,...route.request().postDataJSON()};return route.fulfill({json:saved})}
  if(path.endsWith('/me/limits'))return route.fulfill({json:{submissions_minute:6,submissions_day:100,pending:2,editor_sessions:1}});
  return route.fallback();
 });
 await use();
},{auto:true}]});
