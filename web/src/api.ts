import type {components} from './api.generated';
export type ProblemDetail=components['schemas']['ProblemDetail'];
export type ProblemSummary=components['schemas']['ProblemSummary'];
export type User=components['schemas']['User'];
export type Submission=components['schemas']['Submission'];
export let csrf = '';
export function setCsrf(token:string){csrf=token}
export class ApiError extends Error{
 constructor(public status:number,message:string){super(message);this.name='ApiError'}
}
export async function api(path:string, method='GET', body?:unknown, key?:string) {
 const response=await fetch(`/api/v1${path}`,{method,headers:{'Content-Type':'application/json','X-CSRF-Token':csrf,...(key?{'Idempotency-Key':key}:{})},body:body===undefined?undefined:JSON.stringify(body)});
 if(!response.ok){let data=await response.json().catch(()=>({error:`Request failed (${response.status})`}));throw new ApiError(response.status,data.error||`Request failed (${response.status})`)}
 return response.status===204?null:response.json();
}
