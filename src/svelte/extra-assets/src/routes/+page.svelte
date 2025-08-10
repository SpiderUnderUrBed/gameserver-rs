<script lang="ts">
    import { onMount } from 'svelte';
  
    let siteUrl = '';
    let username = '';
    let password = '';
    let userMessage = '';
    let serverMessage = '';
    let showDeveloper = false;
  
    onMount(() => {
      const meta = document.querySelector('meta[name="site-url"]');
      siteUrl = (meta?.getAttribute('content') ?? '').replace(/\/$/, '');
    });
  
    function goToMainPage() {
      window.location.href = `${siteUrl}/main.html`;
    }
  
    function toggleDeveloper() {
      showDeveloper = !showDeveloper;
    }
  
    async function login() {
      const formBody = new URLSearchParams();
      formBody.append('user', username);
      formBody.append('password', password);
  
      try {
        const res = await fetch(`${siteUrl}/api/signin`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/x-www-form-urlencoded'
          },
          body: formBody.toString()
        });
  
        if (!res.ok) {
          throw new Error(`Login failed with status ${res.status}`);
        }
  
        const data = await res.json();
        const jwtToken = encodeURIComponent(data.response);
        const nextUrl = encodeURIComponent(`${siteUrl}/main.html`);
  
        window.location.href = `${siteUrl}/authenticate?next=${nextUrl}&jwk=${jwtToken}`;
      } catch (err) {
        console.error('Login error:', err);
        alert('Login failed: ' + err.message);
      }
    }
  
    async function fetchMessage() {
      try {
        const response = await fetch(`${siteUrl}/api/message`);
        if (response.ok) {
          const data = await response.json();
          serverMessage = data.message;
        } else {
          serverMessage = 'Failed to get message from the server.';
        }
      } catch (error) {
        console.error('Error fetching message:', error);
        serverMessage = 'Error connecting to the server.';
      }
    }
  
    async function sendUserMessage() {
      if (!userMessage) {
        serverMessage = 'Please enter a message to send.';
        return;
      }
  
      try {
        const response = await fetch(`${siteUrl}/api/send`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json'
          },
          body: JSON.stringify({ message: userMessage })
        });
  
        if (response.ok) {
          const data = await response.json();
          serverMessage = `Server Response: ${data.response}`;
        } else {
          serverMessage = 'Failed to send message to the server.';
        }
      } catch (error) {
        console.error('Error sending message:', error);
        serverMessage = 'Error sending message to the server.';
      }
    }
  </script>
  <meta name="site-url" content="[[SITE_URL]]" />
  <style>
    /* Move your original styles here */
  </style>
  
  <h1>Welcome to My Page</h1>
  
  <p>{siteUrl ? `Base Path: ${siteUrl}` : ''}</p>
  
  <div class="login">
    <h4>Login:</h4>
    <input class="user-login" type="text" bind:value={username} placeholder="Username" />
    <input class="user-password" type="password" bind:value={password} placeholder="Password" />
    <div>
      <button class="signup">Signup</button>
      <button class="login-button" on:click={login}>Login</button>
    </div>
  </div>
  
  <button class="big-button" on:click={goToMainPage}>Go to main page</button>
  <button class="small-button" on:click={toggleDeveloper}>Developer Work</button>
  
  {#if showDeveloper}
    <div class="developer-section">
      <p>Click the button to get a message from the server:</p>
      <button on:click={fetchMessage}>Get Message</button>
      <div>{serverMessage}</div>
  
      <h2>Send a Message to the Server:</h2>
      <input type="text" bind:value={userMessage} placeholder="Type your message here" />
      <button on:click={sendUserMessage}>Send Message</button>
    </div>
  {/if}
  