import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
export default defineConfig({ plugins: [svelte()], worker:{format:"es"}, server: { port: Number(process.env.PORT || 8080), strictPort: true, proxy: { '/api': 'http://127.0.0.1:18080', '/editor': {target:'ws://127.0.0.1:8081',ws:true}, '/healthz': 'http://127.0.0.1:18080', '/readyz': 'http://127.0.0.1:18080', '/metrics': 'http://127.0.0.1:18080' } }, build: { target:'es2022' } });
