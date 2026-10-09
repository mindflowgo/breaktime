<script lang="ts">
	let { children } = $props();
</script>

<svelte:head>
	<title>Breaktime Overlay</title>
</svelte:head>

<div class="overlay-root" data-tauri-drag-region>
	{@render children()}
</div>

<style>
	:global(html), :global(body) {
		background: transparent !important;
		margin: 0;
		padding: 0;
		overflow: hidden;
		user-select: none;
		-webkit-user-select: none;
		-webkit-app-region: drag;
	}

	.overlay-root {
		width: 100vw;
		height: 100vh;
		background: transparent !important;
		overflow: hidden;
		-webkit-app-region: drag;
	}
</style>
