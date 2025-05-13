<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { writable } from 'svelte/store';
  import { tasks, type Task } from '$lib/stores';

  export const theme = writable<'light' | 'dark'>('dark');

  let unlisten: () => void;          // will hold the listener cleanup fn
  let commandInput = '';
  let commandOutput = '';

  /* ───── lifecycle ───── */
  onMount(async () => {
    try {
      const initialTheme = await invoke<'light' | 'dark'>('get_current_theme');
      theme.set(initialTheme);

      // initial tasks loading
      const today = new Date().toLocaleDateString('en-CA');
      await load_tasks_for_day(today);

      document.documentElement.classList.remove('light', 'dark');
      document.documentElement.classList.add(initialTheme);

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

    //clean this later, mostly just testing
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
    const today = new Date().toLocaleDateString('en-CA');
    await load_tasks_for_day(today);
    
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      submitCommand();
    }
  }

  // function to get tasks
  async function load_tasks_for_day(day: string) {
    try {
      const list = await invoke<Task[]>('get_tasks', {day});
      tasks.set(list);
    }
    catch (e) {
      console.error('failed loading tasks: ', e);
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
  <div class="topbar"> 
    <p>
      {#if $tasks.length > 0}
        {#each $tasks as task}
          {#if task.status == "doing"}
            {task.title}
          {/if}
        {/each}
      {/if}
    </p>
  </div>
  <div class="tasks">
    <h2>tasks!</h2>

    {#if $tasks.length > 0}
    <ul>
      {#each $tasks as task}
        <li class={task.status}>
          {task.title}
        </li>
      {/each}
    </ul>
    {:else}
      <p>not a task yet, get to work!</p>
    {/if}
  </div>



  <div class ="commandline">
    <input
      type="text"
      bind:value={commandInput}
      placeholder="~welcome to salieri"
      on:keydown={handleKeydown}
    />
  </div>

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

.topbar {
  color: purple;
}

.doing {
  color: green;
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

.tasks {
  width: 20%;
  height: 60%;
  background-color: red;
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