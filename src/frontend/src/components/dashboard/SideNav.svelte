<script lang="ts">
	import { Link } from 'cross-router-svelte';
</script>

<aside class="w-64">
	<nav>
		<ul class="menu w-full">
			<li>
				<Link href="/" exactActiveClass="menu-active">Home Page</Link>
			</li>
			<li>
				<Link href="/users" activeClass="menu-active">Users</Link>
			</li>
			<li>
				<Link href="/servers" activeClass="menu-active">Servers</Link>
			</li>
			<li>
				<Link href="/nodes" activeClass="menu-active">Nodes</Link>
			</li>
			<li>
				<Link href="/manual" activeClass="menu-active">Manual</Link>
			</li>

			<li>
				<details>
					<summary>Developer Options</summary>

					<li>
						<Link href="/create-server" activeClass="menu-active">Create servers</Link>
					</li>
				</details>
			</li>
		</ul>
	</nav>
</aside>
