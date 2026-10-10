import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		// Not 5173: that's commonly grabbed by other Vite projects on this machine
		port: 5199,
		strictPort: true
	}
});
