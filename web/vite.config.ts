import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
export default defineConfig({ plugins: [svelte()], worker:{format:"es"}, server: { proxy: { '/api': 'http://localhost:8080', '/editor': {target:'ws://localhost:8081',ws:true} } }, build: { target:'es2022' } });
