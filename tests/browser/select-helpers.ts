import type {Locator} from '../../web/node_modules/@playwright/test/index';
export async function selectValue(control:Locator,value:string|number){
 await control.click();
 const id=await control.getAttribute('aria-controls');
 await control.page().locator(`[id="${id}"] [role="option"][data-value=${JSON.stringify(String(value))}]`).click();
}
