<script lang="ts">
	import { Link } from 'svelte5-router';
</script>

<aside class="w-64">
	<nav>
		<ul class="menu w-full">
			<li>
				<Link to="/">
					{#snippet children(active)}
						<span class:menu-active={active}>Home Page</span>
					{/snippet}
				</Link>
			</li>
			<li>
				<Link to="/users">
					{#snippet children(active)}
						<span class:menu-active={active}>Users</span>
					{/snippet}
				</Link>
			</li>
			<li>
				<Link to="/servers">
					{#snippet children(active)}
						<span class:menu-active={active}>Servers</span>
					{/snippet}
				</Link>
			</li>
			<li>
				<Link to="/nodes">
					{#snippet children(active)}
						<span class:menu-active={active}>Nodes</span>
					{/snippet}
				</Link>
			</li>
			<li>
				<Link to="/manual">
					{#snippet children(active)}
						<span class:menu-active={active}>Manual</span>
					{/snippet}
				</Link>
			</li>

			<li>
				<details>
					<summary>Developer Options</summary>
					<li>
						<Link to="/create-server">
							{#snippet children(active)}
								<span class:menu-active={active}>Create servers</span>
							{/snippet}
						</Link>
					</li>
				</details>
			</li>
		</ul>
	</nav>
</aside>
