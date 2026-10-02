import {test as base} from '../../web/node_modules/@playwright/test/index';
import {createHash} from 'node:crypto';
// Fixtures deliberately use the host control plane; account creation has no HTTP route.
import {execFileSync} from 'node:child_process';
import {mkdtempSync,writeFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
export const publicOrigin=process.env.TEST_ORIGIN||'http://127.0.0.1:18080';
export const adminOrigin=process.env.TEST_ADMIN_ORIGIN||'http://127.0.0.1:18082';
function manage(args:string[],input?:string){return execFileSync('../target/debug/api',['manage',...args],{encoding:'utf8',input}).trim()}
export function createUser(name:string,password:string){manage(['create-user',name],password+'\n')}
export function createProblem(definition:unknown){
 const directory=mkdtempSync(join(tmpdir(),'j0coder-browser-'));
 try{const file=join(directory,'problem.json');writeFileSync(file,JSON.stringify(definition));const draft=JSON.parse(manage(['import',file]));manage(['publish',draft.id]);return draft}finally{rmSync(directory,{recursive:true,force:true})}
}

// Generic UI fixtures test behavior separately from the dedicated quota suite.
// Reset this one limiter only in an explicitly disposable, serialized test run.
export const test=base.extend<{hostLimits:void}>({hostLimits:[async({},use)=>{
 if(process.env.SECURITY_TEST_DISPOSABLE!=='yes'){await use();return;}
 const engine=process.env.TEST_CONTAINER_ENGINE||'podman';
 if(!['podman','docker'].includes(engine))throw new Error('Unsupported test container engine');
 const ip=createHash('sha256').update('127.0.0.1').digest('hex');
 execFileSync(engine,['exec','j0coder-hardening-valkey','valkey-cli','DEL',`limit:ip:${ip}:login`],{stdio:'ignore'});
 await use();
},{auto:true}]});
