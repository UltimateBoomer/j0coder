import {defineConfig} from '@playwright/test';
export default defineConfig({testDir:'../tests/browser',fullyParallel:false,use:{baseURL:process.env.TEST_ORIGIN||'http://127.0.0.1:18080',headless:true},timeout:60000,reporter:'list'});
