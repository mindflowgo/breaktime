<script lang="ts">
	let { children } = $props();
</script>

<div class="h-screen w-screen overflow-hidden bg-transparent select-none text-zinc-100">
	{@render children()}
</div>
