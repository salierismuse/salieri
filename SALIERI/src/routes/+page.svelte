<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { writable } from 'svelte/store';

  export const theme = writable<'light' | 'dark'>('dark');

  let unlisten: () => void;          // will hold the listener cleanup fn
  let commandInput = '';
  let commandOutput = '';

  /* ───── lifecycle ───── */
  onMount(async () => {
    try {
      // Get initial theme from backend
      const initialTheme = await invoke<'light' | 'dark'>('get_current_theme');
      theme.set(initialTheme);
      
      // Set initial theme class
      document.documentElement.classList.remove('light', 'dark');
      document.documentElement.classList.add(initialTheme);

      // Subscribe for live changes
      unlisten = await listen('theme_changed', ({ payload }) => {
        const newTheme = (payload as { theme: 'light' | 'dark' }).theme;
        theme.set(newTheme);
        document.documentElement.classList.remove('light', 'dark');
        document.documentElement.classList.add(newTheme);
      });
    } catch (e) {
      console.error('Failed to initialize theme:', e);
    }
  });

  onDestroy(() => unlisten?.());     // avoid piling up listeners in dev

  /* ───── command palette ───── */
  async function submitCommand() {
    if (commandInput.trim() === '') return;
    try {
      commandOutput = await invoke('handle_palette_command', {
        command: commandInput
      });
    } catch (e) {
      commandOutput = `error: ${e}`;
    } finally {
      commandInput = '';
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      submitCommand();
    }
  }

  /* ───── reactively sync <html> classes without wiping others ───── */
  $: {
    if (document.documentElement) {
      const root = document.documentElement.classList;
      root.remove('light', 'dark');
      root.add($theme);
    }
  }
</script>

<main>
  <h1>salieri</h1>

  <input
    type="text"
    bind:value={commandInput}
    placeholder="~welcome to salieri"
    on:keydown={handleKeydown}
  />

  {#if commandOutput}
    <p class="greeting-message">{commandOutput}</p>
  {/if}
</main>

<style>
:global(html.dark)  { --bg:#000; --fg:#fff; --accent:lime; }
:global(html.light) { --bg:#fff; --fg:#000; --accent:red; }

:global(html),
:global(body) {
  margin: 0;
  height: 100%;
  width: 100%;
  background: var(--bg);
  color: var(--fg);
}

main {
  height: 100%;
  width: 100%;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  text-align: center;
}

h1 {
  color: var(--accent);
  text-transform: lowercase;
  font-size: 4em;
  font-weight: 200;
  margin-bottom: 1.5em;
}

input[type='text'] {
  padding: 0.8em;
  border-radius: 4px;
  border: 1px solid var(--fg);
  background-color: var(--bg);
  font-size: 1em;
  position: fixed;
  bottom: 5px;
  left: 50%;
  transform: translateX(-50%);
  width: 95%;
  box-sizing: border-box;
  z-index: 100;
}

input[type='text']:focus {
  border-color: var(--accent);
  outline: none;
  color: var(--accent);
  box-shadow: 0 0 0 2px rgba(255, 62, 0, 0.2);
}

.greeting-message {
  margin-top: 1.5em;
  font-weight: bold;
  font-size: 1.1em;
  color: var(--accent);
}
</style>