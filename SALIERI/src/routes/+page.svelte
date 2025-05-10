<script lang="ts">
  // Import the invoke function from Tauri's API
  import { invoke } from '@tauri-apps/api/core';
  import { writable } from 'svelte/store';
  export const theme = writable<'light' | 'dark'>('dark');
  

  import { listen } from '@tauri-apps/api/event';
	listen('theme_changed', ({ payload }) => theme.set(payload.theme));




  // Reactive variables to hold the input name and the greeting message
  let commandInput: string = '';
  let commandOutput: string = '';
  let greeting: string = '';

async function submitCommand() {
  if (commandInput.trimEnd() === '') return;
  try {
    commandOutput = await invoke('handle_palette_command', { command: commandInput });
  } catch (e) {
    commandOutput = `Error: ${e}`;
  } finally {
    commandInput = '';
  }
}
// pretty simple, searched for keyboard down events, if that is enter
// it activates, stops enters default behavior, and runs submitCommand
async function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Enter') {
      event.preventDefault();

      await submitCommand();

  }
 

}

$: document.documentElement.className = $theme;   // "dark" or "light"

</script>
  <main>
    <h1>SALIERI</h1>
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
  margin:0; height:100%; width:100%;
  background:var(--bg);
  color:var(--fg);
}

main {
  height: 100%;
  width: 100%;
  margin: 0;          /* wipe the old 40 px */
  padding: 0;         /* optional – if you want edge-to-edge */
  display: flex;      /* or grid – up to you */
  flex-direction: column;
  text-align: center;
}


  h1 {
    color:var(--accent); /* Svelte orange */
    text-transform: lowercase;
    font-size: 4em;
    font-weight: 200; /* Lighter font weight */
    margin-bottom: 1.5em;
  }

  .card {
  display: flex;
  flex-direction: column;
  gap: 0.8em;
  margin-bottom: 5em;
  padding: 1.5em;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

input[type="text"] {
  padding: 0.8em;
  border-radius: 4px;
  border: 1px solid var(--fg);
  background-color:var(--bg);
  font-size: 1em;

  /* positioning the input to the bottom of the viewport */
  position: fixed;
  bottom: 5px;
  left: 48%;
  transform: translateX(-50%);
  width: 95%; /* or adjust based on desired layout */
  box-sizing: border-box; /* often needed with width: 100% or 90% + padding */
  z-index: 100; /* optional, to ensure it's on top */
}

  input[type="text"]:focus {
    border-color: var(--accent);
    outline: none;
    color: var(--accent);
    box-shadow: 0 0 0 2px rgba(255, 62, 0, 0.2);
  }

  button {
    padding: 0.8em 1.2em;
    border-radius: 4px;
    border: none;
    background-color: var(--bg);
    color: var(--fg);
    cursor: pointer;
    font-size: 1em;
    transition: background-color 0.2s ease;
  }

  button:hover {
    background-color: var(--bg);
  }

  .greeting-message {
    margin-top: 1.5em;
    font-weight: bold;
    font-size: 1.1em;
    color: var(--accent);
  }

</style>