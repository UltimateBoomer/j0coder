export type Filters={q:string;difficulty:string;tag:string};
export type Route=
 | {kind:'problems';filters:Filters}
 | {kind:'problem';problemId:string}
 | {kind:'admin'}
 | {kind:'admin-new'}
 | {kind:'admin-edit';problemId:string}
 | {kind:'access-denied'}
 | {kind:'not-found'};

const uuid=/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
export function parseRoute(location:Pick<Location,'pathname'|'search'>):Route{
 const path=location.pathname.replace(/\/+$/,'')||'/';
 if(path==='/'){const params=new URLSearchParams(location.search);return {kind:'problems',filters:{q:params.get('q')||'',difficulty:params.get('difficulty')||'',tag:params.get('tag')||''}}}
 if(path==='/admin/problems')return {kind:'admin'};
 if(path==='/admin/problems/new')return {kind:'admin-new'};
 let match=path.match(/^\/problems\/([^/]+)$/);
 if(match)return uuid.test(match[1])?{kind:'problem',problemId:match[1]}:{kind:'not-found'};
 match=path.match(/^\/admin\/problems\/([^/]+)$/);
 if(match)return uuid.test(match[1])?{kind:'admin-edit',problemId:match[1]}:{kind:'not-found'};
 return {kind:'not-found'};
}
export function problemListUrl(filters:Filters):string{const params=new URLSearchParams();if(filters.q)params.set('q',filters.q);if(filters.difficulty)params.set('difficulty',filters.difficulty);if(filters.tag)params.set('tag',filters.tag);const query=params.toString();return query?`/?${query}`:'/'}
export function push(url:string){history.pushState(null,'',url);window.dispatchEvent(new PopStateEvent('popstate'))}
export function replace(url:string){history.replaceState(null,'',url)}
