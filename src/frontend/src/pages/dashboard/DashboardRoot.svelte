<script lang="ts">
	import { RouterView, useNavigate } from 'cross-router-svelte';
	import ThemeToggle from '../../components/ThemeToggle.svelte';
	import { LogOutIcon } from '@lucide/svelte';
	import SideNav from '../../components/dashboard/SideNav.svelte';
	import { auth } from '../../lib/auth/auth.svelte';
	import type { Snippet } from 'svelte';

	let { outlet }: { outlet?: Snippet } = $props();

	const navigate = useNavigate();

	async function logout(event: Event) {
		event.preventDefault();
		try {
			await auth.logout();
		} catch {}

		navigate('/auth/login');
	}
</script>

<div class="app-grid">
	<header class="navbar bg-primary flex flex-row gap-2">
		<span class="flex-1 text-xl font-semibold">Server Panel</span>

		<ThemeToggle class="hover:btn-primary" />
		<button
			class="btn btn-square btn-ghost hover:btn-primary tooltip tooltip-bottom"
			data-tip="Logout"
			aria-label="Logout"
			onclick={logout}
		>
			<LogOutIcon class="w-4" />
		</button>
	</header>
	<SideNav />

	{@render outlet?.()}
</div>

<style>
	.app-grid {
		display: grid;
		grid-template-columns: 250px 1fr;
		grid-template-rows: auto 1fr;
		min-height: 100vh;
	}
	.app-grid > header {
		grid-column: 1 / -1;
	}
</style>
