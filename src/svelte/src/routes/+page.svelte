<script>
  import { onMount } from 'svelte';

  let basePath = '';

  onMount(() => {
    const meta = document.querySelector('meta[name="site-url"]');
    basePath = (meta?.content ?? '').replace(/\/$/, '');
  });
  console.log(basePath);

  let username = '';
  let password = '';

  async function login() {
    const formBody = new URLSearchParams();
    formBody.append('user', username);
    formBody.append('password', password);

    try {
      const res = await fetch(`${basePath}/api/signin`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/x-www-form-urlencoded',
        },
        body: formBody.toString(),
      });

      if (!res.ok) {
        throw new Error(`Login failed with status ${res.status}`);
      }

      const data = await res.json();
      const jwtToken = data.response;

      const nextUrl = encodeURIComponent(`${basePath}/main.html`);
      const jwkToken = encodeURIComponent(jwtToken);

      window.location.href = `${basePath}/authenticate?next=${nextUrl}&jwk=${jwkToken}`;
    } catch (err) {
      console.error('Login error:', err);
      alert('Login failed: ' + err.message);
    }
  }
</script>

<meta name="site-url" content="[[SITE_URL]]">

<style>
  .login {
    margin-bottom: 1em;
  }
</style>

<h1>Welcome to My Page</h1>

<div class="login">
  <h4>Login:</h4>
  <input
    type="text"
    name="username"
    placeholder="Username"
    bind:value={username}
  />
  <input
    type="password"
    name="password"
    placeholder="Password"
    bind:value={password}
  />
  <div>
    <button on:click={login}>Login</button>
  </div>
</div>
