import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { auth } from './lib/auth/auth.svelte'

void auth.fetchUser().catch(() => {
  auth.user = null;
})

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
