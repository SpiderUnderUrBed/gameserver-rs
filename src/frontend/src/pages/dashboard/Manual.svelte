<script lang="ts">
	import { marked } from "marked";
	import { onMount } from "svelte";
	let content = $state('');
	//const fs = require('fs');
	// import fs from 'fs'
	onMount(async () => {
		content = await fetch('/manual.md').then(r => r.text());
	});
</script>

<div class="flex-1 prose p-4">
	<h2 class="text-xl font-semibold mb-2">Manual</h2>

	<!-- <p>Welcome to the game server management system.</p> -->
	<div class="prose">
		{@html marked(content)}
	</div>
</div>
