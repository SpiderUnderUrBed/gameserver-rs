<script lang="ts">
	import { Link } from 'svelte5-router';
	import { serverConsole } from '../../lib/stores/serverConsoleStore.svelte';
</script>

<ul class="menu menu-horizontal rounded bg-base-100 w-full">
	<li>
		<Link to="/">
			{#snippet children(active)}
				<span class:menu-active={active}>Home</span>
			{/snippet}
		</Link>
	</li>
	<li>
		<Link to="/filebrowser">
			{#snippet children(active)}
				<span class:menu-active={active}>Filebrowser</span>
			{/snippet}
		</Link>
	</li>
	<li>
		<Link to="/statistics">
			{#snippet children(active)}
				<span class:menu-active={active}>Statistics</span>
			{/snippet}
		</Link>
	</li>
	<li>
		<Link to="/workflows">
			{#snippet children(active)}
				<span class:menu-active={active}>Workflows</span>
			{/snippet}
		</Link>
	</li>
	<li>
		<Link to="/integrations">
			{#snippet children(active)}
				<span class:menu-active={active}>Integrations</span>
			{/snippet}
		</Link>
	</li>
	<li>
		<Link to="/backups">
			{#snippet children(active)}
				<span class:menu-active={active}>Backups</span>
			{/snippet}
		</Link>
	</li>
	<li>
		<Link to="/settings">
			{#snippet children(active)}
				<span class:menu-active={active}>Settings</span>
			{/snippet}
		</Link>
	</li>

	<li>
		<button popovertarget="manage-btns" style="anchor-name:--manage-btns">Manage buttons</button>

		<ul
			class="dropdown menu w-52 rounded-box bg-base-100 shadow-md -ml-1 mt-3"
			popover
			id="manage-btns"
			style="position-anchor:--manage-btns"
		>
			<li>
				<button onclick={() => serverConsole.loadTopmostButtons()}>Configure Button links</button>
			</li>
			<li>
				<button onclick={() => serverConsole.loadTopmostButtons()}>
					Temporarily enable defaults
				</button>
			</li>

			<li>
				<button onclick={() => serverConsole.loadTopmostButtons()}>Restore defaults</button>
			</li>
		</ul>
	</li>
</ul>
