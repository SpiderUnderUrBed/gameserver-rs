<script lang="ts">
	import { Router, Route } from 'svelte5-router';
	import ThemeProvider from './lib/theme/ThemeProvider.svelte';
	import { Toaster } from 'svelte-sonner';
	import Login from './pages/Login.svelte';
	import DashboardRoot from './pages/dashboard/DashboardRoot.svelte';
	import Home from './pages/dashboard/Home.svelte';
	import Users from './pages/dashboard/Users.svelte';
	import Manual from './pages/dashboard/Manual.svelte';
	import Nodes from './pages/dashboard/Nodes.svelte';
	import Servers from './pages/dashboard/Servers.svelte';
	import CreateServer from './pages/dashboard/CreateServer.svelte';
	import Statistics from './pages/dashboard/home/Statistics.svelte';
	import Integrations from './pages/dashboard/home/Integrations.svelte';
	import FileBrowser from './pages/dashboard/home/FileBrowser.svelte';
	import Workflows from './pages/dashboard/home/Workflows.svelte';
	import HomeIndex from './pages/dashboard/home/HomeIndex.svelte';
	import Backups from './pages/dashboard/home/Backups.svelte';
	import Settings from './pages/dashboard/home/Settings.svelte';
	import ProtectedPage from './components/ProtectedPage.svelte';

	let url = $state('');
</script>

<Toaster />
<ThemeProvider>
	<Router {url}>
		<Route path="/auth/login" component={Login} />
		<Route path="/auth/signup" component={Login} />
		<Route path="/users">
			{#snippet children()}
				<ProtectedPage>
					<DashboardRoot>
						{#snippet outlet()}
							<Users />
						{/snippet}
					</DashboardRoot>
				</ProtectedPage>
			{/snippet}
		</Route>
		<Route path="/nodes">
			{#snippet children()}
				<ProtectedPage>
					<DashboardRoot>
						{#snippet outlet()}
							<Nodes />
						{/snippet}
					</DashboardRoot>
				</ProtectedPage>
			{/snippet}
		</Route>
		<Route path="/servers">
			{#snippet children()}
				<ProtectedPage>
					<DashboardRoot>
						{#snippet outlet()}
							<Servers />
						{/snippet}
					</DashboardRoot>
				</ProtectedPage>
			{/snippet}
		</Route>
		<Route path="/manual">
			{#snippet children()}
				<ProtectedPage>
					<DashboardRoot>
						{#snippet outlet()}
							<Manual />
						{/snippet}
					</DashboardRoot>
				</ProtectedPage>
			{/snippet}
		</Route>
		<Route path="/create-server">
			{#snippet children()}
				<ProtectedPage>
					<DashboardRoot>
						{#snippet outlet()}
							<CreateServer />
						{/snippet}
					</DashboardRoot>
				</ProtectedPage>
			{/snippet}
		</Route>
		<Route path="/filebrowser">
			{#snippet children()}
				<ProtectedPage>
					<DashboardRoot>
						{#snippet outlet()}
							<Home>
								{#snippet outlet()}
									<FileBrowser />
								{/snippet}
							</Home>
						{/snippet}
					</DashboardRoot>
				</ProtectedPage>
			{/snippet}
		</Route>
		<Route path="/statistics">
			{#snippet children()}
				<ProtectedPage>
					<DashboardRoot>
						{#snippet outlet()}
							<Home>
								{#snippet outlet()}
									<Statistics />
								{/snippet}
							</Home>
						{/snippet}
					</DashboardRoot>
				</ProtectedPage>
			{/snippet}
		</Route>
		<Route path="/workflows">
			{#snippet children()}
				<ProtectedPage>
					<DashboardRoot>
						{#snippet outlet()}
							<Home>
								{#snippet outlet()}
									<Workflows />
								{/snippet}
							</Home>
						{/snippet}
					</DashboardRoot>
				</ProtectedPage>
			{/snippet}
		</Route>
		<Route path="/backups">
			{#snippet children()}
				<ProtectedPage>
					<DashboardRoot>
						{#snippet outlet()}
							<Home>
								{#snippet outlet()}
									<Backups />
								{/snippet}
							</Home>
						{/snippet}
					</DashboardRoot>
				</ProtectedPage>
			{/snippet}
		</Route>
		<Route path="/settings">
			{#snippet children()}
				<ProtectedPage>
					<DashboardRoot>
						{#snippet outlet()}
							<Home>
								{#snippet outlet()}
									<Settings />
								{/snippet}
							</Home>
						{/snippet}
					</DashboardRoot>
				</ProtectedPage>
			{/snippet}
		</Route>
		<Route path="/integrations">
			{#snippet children()}
				<ProtectedPage>
					<DashboardRoot>
						{#snippet outlet()}
							<Home>
								{#snippet outlet()}
									<Integrations />
								{/snippet}
							</Home>
						{/snippet}
					</DashboardRoot>
				</ProtectedPage>
			{/snippet}
		</Route>
		<Route path="/">
			{#snippet children()}
				<ProtectedPage>
					<DashboardRoot>
						{#snippet outlet()}
							<Home>
								{#snippet outlet()}
									<HomeIndex />
								{/snippet}
							</Home>
						{/snippet}
					</DashboardRoot>
				</ProtectedPage>
			{/snippet}
		</Route>
	</Router>
</ThemeProvider>
