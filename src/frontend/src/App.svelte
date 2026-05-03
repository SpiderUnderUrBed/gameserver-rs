<script lang="ts">
  import DashboardRoot from './pages/dashboard/DashboardRoot.svelte';
  import Home from './pages/dashboard/Home.svelte';
  import HomeIndex from './pages/dashboard/home/HomeIndex.svelte';
  import ThemeProvider from './lib/theme/ThemeProvider.svelte';
</script>

<ThemeProvider>
  <DashboardRoot>
    {#snippet outlet()}
      <Home>
        {#snippet outlet()}
          <HomeIndex />
        {/snippet}
      </Home>
    {/snippet}
  </DashboardRoot>
</ThemeProvider>