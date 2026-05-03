<script lang="ts">
  import { serverConsole } from '../../lib/stores/serverConsoleStore.svelte';
</script>

<ul class="menu menu-horizontal rounded bg-base-100 w-full">
  <li><a href="/">Home</a></li>
  <li><a href="/filebrowser">Filebrowser</a></li>
  <li><a href="/statistics">Statistics</a></li>
  <li><a href="/workflows">Workflows</a></li>
  <li><a href="/integrations">Integrations</a></li>
  <li><a href="/backups">Backups</a></li>
  <li><a href="/settings">Settings</a></li>
  <li>
    <button popovertarget="manage-btns" style="anchor-name:--manage-btns">Manage buttons</button>
    <ul
      class="dropdown menu w-52 rounded-box bg-base-100 shadow-md -ml-1 mt-3"
      popover
      id="manage-btns"
      style="position-anchor:--manage-btns"
    >
      <li><button onclick={() => serverConsole.loadTopmostButtons()}>Configure Button links</button></li>
      <li><button onclick={() => serverConsole.loadTopmostButtons()}>Temporarily enable defaults</button></li>
      <li><button onclick={() => serverConsole.loadTopmostButtons()}>Restore defaults</button></li>
    </ul>
  </li>
</ul>