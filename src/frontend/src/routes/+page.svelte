<script lang="ts">
    import { onMount } from 'svelte';
  
    let siteUrl = '';
    let username = '';
    let password = '';
    let userMessage = '';
    let serverMessage = '';
    let showDeveloper = false;
    let colorState = $state('WHITE');
    let currentLogin = $state('manual');
    let logged_in = $state(false);

    let loggedInClass = $derived(
      logged_in ? `logged-in-${colorState === 'WHITE' ? 'white' : 'black'}` : ''
    );

    const logins = [
      {
        id: "manual",
        label: "Manual",
      },
      {
        id: "oidc",
        label: "Oidc"
      }
    ];
  
    onMount(() => {
      const meta = document.querySelector('meta[name="site-url"]');
      siteUrl = (meta?.getAttribute('content') ?? '').replace(/\/$/, '');
      checkLoggedInStatus();
    });
    function toggleColor(){
      if (colorState == "WHITE"){
        sessionStorage.setItem('binary-theme', 'black');
        document.body.style.backgroundColor = "black";
        colorState = "BLACK";
      } else {
        sessionStorage.setItem('binary-theme', 'white');
        document.body.style.backgroundColor = "white";
        colorState = "WHITE";
      }
    }
  
    function goToMainPage() {
      window.location.href = `${siteUrl}/main.html`;
    }
  
    function toggleDeveloper() {
      showDeveloper = !showDeveloper;
    }
  
    async function logout(){
      try {
        const res = await fetch(`${siteUrl}/api/signout`, {
          method: 'DELETE'
        });
        if (res.ok){
          checkLoggedInStatus();
        } else {
          console.log("log out failed");
        }
      } catch (e) {
        console.error(e);
      }
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
        checkLoggedInStatus();

        const jwtToken = encodeURIComponent(data.response);
        const nextUrl = encodeURIComponent(`${siteUrl}/main.html`);
  
        window.location.href = `${siteUrl}/?next=${nextUrl}`;
        //window.location.href = `${siteUrl}/authenticate?next=${nextUrl}&jwk=${jwtToken}`;
      } catch (err) {
        console.error('Login error:', err);
        alert('Login failed: ' + err.message);
      }
    }
    async function checkLoggedInStatus(){
      try {
        const res = await fetch(`${siteUrl}/api/user/me`)
        if (res.ok) {
          logged_in = true;
        } else {
          logged_in = false;
        }
      } catch (e) {
        logged_in = false;
        console.error(e);
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
    /* .color-toggle-white {
      background-color: white;
      color: black;
    }
    .color-toggle-black {
      background-color: black;
      color: white;
    } */
    .header-white {
      background-color: orange;
      color: black; 
    }
    .header-black {
      background-color: orange;
      color: white;
    }
    .secondary-header-white {
      background-color: orange;
      color: black; 
    }
    .secondary-header-black {
      background-color: orange;
      color: white;
    }
    
    .general-buttons-white {
      background-color: white;
      color: black; 
    }
    .general-buttons-black {
      background-color: black;
      color: white;
    }

    .logged-in-white {
      text-decoration: solid;
      background-color: green;
    }

    .logged-in-black {
      text-decoration: solid;
      background-color: green;
    }
    .user-login-white {
      background-color: white;
      color: black; 
    }
    .user-login-black {
      background-color: black;
      color: white;
    }
    .user-password-white {
      background-color: white;
      color: black; 
    } 
    .user-password-black {
      background-color: black;
      color: white;
    } 
    .center {
      width: 100svw;
      display: flex;
      justify-content: center;
    }
    .orange-banner {
      background-color: orange;
      width: 100svw;
      display: flex;
      justify-content: center;
    }
  </style>

  <div class="center">
    <div class="orange-banner">
      <h1 class="header-{colorState === "WHITE" ? "white" : "black"}">Welcome to Gameserver-rs</h1>
    </div>
  </div>

  <p>{siteUrl ? `Base Path: ${siteUrl}` : ''}</p>
  
  <div class="center">
    <div class="orange-banner">
      <div>
        <div class="login">
  				{#each logins as login}
					<button
						type="button"
						
						onclick={() => { currentLogin = login.id; }}
					>
						{login.label}
					</button>
				{/each}
          <h4 class="secondary-header-{colorState === "WHITE" ? "white" : "black"}">Login:</h4>
          <div id="inner-login">
            {#if currentLogin === 'manual'}
              {@render manualLogin()}
            {:else if currentLogin === 'oidc'}
              {@render oidcLogin()}
            {/if}           
          </div>
        </div>


        <div>
          <button class="general-buttons-{colorState === "WHITE" ? "white" : "black"}" onclick={toggleColor}>Toggle {colorState}</button>
          <button class="general-buttons-{colorState === "WHITE" ? "white" : "black"}" onclick={toggleDeveloper}>Developer Work</button>
          {#if logged_in}
            <br>
            <p>You are logged in</p>
            <br>
          {:else}

          {/if}
          <button class="{logged_in ? loggedInClass : ''} general-buttons-{colorState === 'WHITE' ? 'white' : 'black'}" onclick={goToMainPage}>Go to main page</button>
        </div>

        {#if showDeveloper}
          <div class="developer-section">
            <p>Click the button to get a message from the server:</p>
            <button onclick={fetchMessage}>Get Message</button>
            <div>{serverMessage}</div>
        
            <h2>Send a Message to the Server:</h2>
            <input type="text" bind:value={userMessage} placeholder="Type your message here" />
            <button onclick={sendUserMessage}>Send Message</button>
          </div>
        {/if}
      </div>
    </div>
</div>
  
{#snippet manualLogin()}
  <input class="user-login-{colorState === "WHITE" ? "white" : "black"}" type="text" bind:value={username} placeholder="Username" />
  <input class="user-password-{colorState === "WHITE" ? "white" : "black"}" type="password" bind:value={password} placeholder="Password" />
  <div>
    <button class="general-buttons-{colorState === "WHITE" ? "white" : "black"}">Signup</button>
    <button class="general-buttons-{colorState === "WHITE" ? "white" : "black"}" onclick={login}>Login</button>
    <button class="general-buttons-{colorState === "WHITE" ? "white" : "black"}" onclick={logout}>Logout</button>
  </div>
{/snippet}
{#snippet oidcLogin()}
	<button
		onclick={() => {
			window.location.replace('/oidc');
		}}
	>
		Log in with configured OIDC provider
	</button>
{/snippet}

<!-- class="btn btn-sm" 
  class:btn-primary={currentLogin === login.id}
  class:btn-ghost={currentLogin !== login.id} -->