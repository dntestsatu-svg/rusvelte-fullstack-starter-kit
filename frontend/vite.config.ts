import tailwindcss from "@tailwindcss/vite";
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import path from "path";

export default defineConfig({
	envDir: '..',
	plugins: [tailwindcss(), sveltekit()],
	resolve: { alias: { $lib: path.resolve("./src/lib") } },
	server: {
		proxy: {
			'/api': {
				target: 'http://localhost:8080',
				changeOrigin: true
			}
		}
	}
});
