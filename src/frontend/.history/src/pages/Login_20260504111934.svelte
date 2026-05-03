<script lang="ts">
  import { auth } from '../lib/auth/auth.svelte';
  import { CircleXIcon } from '@lucide/svelte';
  import ThemeToggle from '../components/ThemeToggle.svelte';

  const logins = [
    { id: "manual", label: "Manual" },
    { id: "oidc", label: "Oidc" }
  ];

  let currentLogin = $state('manual');
  let error = $state<string | null>(null);
  let submitting = $state(false);

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    error = null;
    submitting = true;
    const form = event.currentTarget as HTMLFormElement;
    const data = new FormData(form);
    try {
      await auth.login(
        data.get('username') as string,
        data.get('password') as string
      );
      window.location.href = '/';
    } catch {
      error = 'Invalid credentials';
    } finally {
      submitting = false;
    }
  }
</script>

<div class="flex flex-col justify-center items-center min-h-screen">
  <div class="card card-border shadow-md bg-base-100 w-full md:max-w-sm">
    <div class="card-body">
      <h1 class="text-2xl font-bold">Welcome to Gameserver-rs</h1>
      <h2 class="card-title">Login</h2>
      <div class="flex gap-2 relative z-10">
        {#each logins as login}
          <button
            type="button"
            class="btn btn-sm"
            class:btn-primary={currentLogin === login.id}
            class:btn-ghost={currentLogin !== login.id}
            onclick={() => { currentLogin = login.id; }}
          >
            {login.label}
          </button>
        {/each}
      </div>
      {#if currentLogin === 'manual'}
        {@render manualLogin()}
      {:else if currentLogin === 'oidc'}
        {@render oidcLogin()}
      {/if}
    </div>
  </div>
</div>

{#snippet manualLogin()}
  <form class="flex flex-col gap-2" onsubmit={handleSubmit}>
    {#if error}
      <div class="alert alert-error">
        <CircleXIcon class="size-6 shrink-0" />
        <span>{error}</span>
      </div>
    {/if}
    <label class="label" for="username">Username</label>
    <input class="input w-full" type="text" required name="username" id="username" placeholder="Username" />
    <label class="label" for="password">Password</label>
    <input class="input w-full" type="password" required name="password" id="password" placeholder="Password" />
    <button class="btn btn-primary" type="submit" disabled={submitting}>
      {submitting ? 'Logging in…' : 'Login'}
    </button>
    <div class="divider my-2" aria-hidden="true">OR</div>
    <a class="btn btn-secondary" href="/auth/signup">Signup</a>
  </form>
{/snippet}

{#snippet oidcLogin()}
  <button onclick={() => { window.location.replace('/oidc'); }}>
    Log in with configured OIDC provider
  </button>
{/snippet}

<ThemeToggle class="fixed top-4 right-4" />