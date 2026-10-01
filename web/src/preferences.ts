// Keep the old values available for a deployment rollback.
export function readPreference(name:'theme'|'blind-mode'):string|null{
 const key=`j0coder:${name}`;
 const current=localStorage.getItem(key);
 if(current!==null)return current;
 const legacy=localStorage.getItem(`locoder:${name}`);
 if(legacy!==null)localStorage.setItem(key,legacy);
 return legacy;
}
