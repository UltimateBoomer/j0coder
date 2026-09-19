#!/usr/bin/env python3
"""Publish original samples through the same authenticated API used by authors."""
import getpass, http.cookiejar, json, os, pathlib, urllib.request
origin=os.environ.get('PUBLIC_ORIGIN','http://localhost:8080')
client=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
csrf=''
def api(path,body=None):
    req=urllib.request.Request(origin+'/api/v1'+path,data=json.dumps(body).encode() if body is not None else None,headers={'Content-Type':'application/json','Origin':origin,'X-CSRF-Token':csrf})
    return json.load(client.open(req))
api('/session',{'username':input('Administrator username: '),'password':getpass.getpass()})
csrf=api('/session')['csrf']
for path in sorted(pathlib.Path('samples').glob('*.json')):
    draft=api('/admin/problems',json.loads(path.read_text()))
    print(path.name,api('/admin/problems/'+draft['id']+'/publish',{}))
