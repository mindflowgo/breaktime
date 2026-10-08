<script lang="ts">
	import '../app.css';

	let { children } = $props();
</script>

<svelte:head>
	<title>Breaktime</title>
</svelte:head>

{@render children()}
