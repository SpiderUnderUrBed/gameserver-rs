<script lang="ts" module>
	export async function action({ formData }: ActionArgs) {
		const username = formData.get('username') as string;
		const password = formData.get('password') as string;

		try {
			await auth.login(username, password);
		} catch {
			return { error: 'Invalid credentials' };
		}

		return redirect('/');
	}
</script>

<script lang="ts">
	import { Form, Link, redirect, useActionData, type ActionArgs } from 'cross-router-svelte';
	import { useTheme } from '../lib/theme/theme.svelte';
	import { auth } from '../lib/auth/auth.svelte';
	import { CircleXIcon, MoonIcon, SunIcon } from '@lucide/svelte';

	const theme = useTheme();

	const actionData = useActionData<typeof action>();

	const error = $derived(actionData()?.error);
</script>

<div class="flex flex-col justify-center items-center min-h-screen">
	<div class="card card-border shadow-md bg-base-100 w-full md:max-w-sm">
		<div class="card-body">
			<h1 class="text-2xl font-bold">Welcome to Gameserver-rs</h1>
			<h2 class="card-title">Login</h2>

			<Form class="flex flex-col gap-2" action="/auth/login">
				{#if error}
					<div class="alert alert-error">
						<CircleXIcon class="size-6 shrink-0" />
						<span>{error}</span>
					</div>
				{/if}

				<label class="label" for="username">Username</label>
				<input
					class="input w-full"
					type="text"
					required
					name="username"
					id="username"
					placeholder="Username"
				/>

				<label class="label" for="password">Password</label>
				<input
					class="input w-full"
					type="password"
					required
					name="password"
					id="password"
					placeholder="Password"
				/>

				<button class="btn btn-primary" type="submit">Login</button>
				<div class="divider my-2" aria-hidden="true">OR</div>
				<Link class="btn btn-secondary" href="/auth/signup">Signup</Link>
			</Form>
		</div>
	</div>
</div>

<button
	class="btn btn-ghost btn-square fixed top-4 right-4"
	onclick={() => theme.toggleTheme()}
	data-tip="Toggle Theme"
>
	{#if theme.current === 'dark'}
		<MoonIcon class="size-4" />
		<span class="sr-only">Toggle mode - dark</span>
	{:else}
		<SunIcon class="size-4" />
		<span class="sr-only">Toggle mode - light</span>
	{/if}
</button>
