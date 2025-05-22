<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { writable } from 'svelte/store';
  import { tasks, type Task } from '$lib/stores';
  import { basicSetup } from 'codemirror'; 
  import {EditorView, keymap} from "@codemirror/view"; 
  import {defaultKeymap, selectPageDown} from "@codemirror/commands"
  import { EditorState } from '@codemirror/state';
  import { get } from 'svelte/store';
  import { tick } from 'svelte';

  export const theme = writable<'light' | 'dark'>('dark');
  export const timerState = writable<'Idle' | 'Running' | 'Paused' | 'ShortBreak' | 'LongBreak'>('Idle');
  export let remainingTime = writable(25 * 60);

  let unlistenTheme: () => void;
  let unlistenTimer: () => void;
  let commandInput = '';
  let commandOutput = '';
  let done = false; 
  let currentLogicalDay = writable('');
  let editorDiv: HTMLDivElement;
  const showEditor = writable(false);
  let myView: EditorView | null = null;

  onMount(async () => {
    try {
      const initialTheme = await invoke<'light' | 'dark'>('get_current_theme');
      theme.set(initialTheme);

      const logicalDayKey = await invoke<string>('get_current_logical_day_key');
      currentLogicalDay.set(logicalDayKey);
      await load_tasks_for_day(logicalDayKey, done); 

      document.documentElement.classList.remove('light', 'dark');
      document.documentElement.classList.add(initialTheme);

      unlistenTheme = await listen('theme_changed', ({ payload }) => {
        const newTheme = (payload as { theme: 'light' | 'dark' }).theme;
        theme.set(newTheme);
        document.documentElement.classList.remove('light', 'dark');
        document.documentElement.classList.add(newTheme);
      });

      unlistenTimer = await listen('timer_updated', ({ payload }) => {
        const timerPayload = payload as { state: 'idle' | 'running' | 'paused' | 'shortbreak' | 'longbreak'; remaining_time: number };
        timerState.set(timerPayload.state.charAt(0).toUpperCase() + timerPayload.state.slice(1) as 'Idle' | 'Running' | 'Paused' | 'ShortBreak' | 'LongBreak');
        remainingTime.set(timerPayload.remaining_time);

        

    });

    toggleEditor();

    } catch (e) {
      console.error('Failed to initialize:', e);
    }
  });

  onDestroy(() => {
    unlistenTheme?.();
    unlistenTimer?.();
  });

  async function submitCommand() {
    if (commandInput.trim() === '') return;
    const cmd = commandInput.trim();
    commandInput = '';

    if (cmd === '/code')
    {
      toggleEditor();
    }

    try {
      commandOutput = await invoke('handle_palette_command', { command: cmd });
    } catch (e) {
      commandOutput = `error: ${e}`;
    }

    const dayToLoad = $currentLogicalDay || new Date().toLocaleDateString('en-CA');

    // timer commands
    if (cmd === '/start') await invoke('start_timer');
    else if (cmd === '/pause') await invoke('pause_timer');
    else if (cmd === '/stop') await invoke('stop_timer');



    // task filter state update
    if (cmd.startsWith('/todo')) done = false;
    else if (cmd.startsWith('/completed')) done = true;

    await load_tasks_for_day(dayToLoad, done);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      submitCommand();
    }
  }

  async function load_tasks_for_day(day: string, taskDoneStatus = false) {
    try {
      const list = await invoke<Task[]>('get_tasks', { day, done: taskDoneStatus });
      tasks.set(list);
    } catch (e) {
      console.error('failed loading tasks:', e);
    }
  }


  async function toggleEditor() {
  const next = !get(showEditor);
  showEditor.set(next);

  if (next) {
    await tick();                // wait until <div> is in the DOM
    mountEditor();               // now editorDiv is live
  } else {
    myView?.destroy();           // clean up
    myView = null;
  }
}

function mountEditor() {
  if (!editorDiv) return;        // safety

  myView?.destroy();             // nuke any previous instance

  const state = EditorState.create({
    doc: 'amadeus was once here',
    extensions: [basicSetup]
  });

  myView = new EditorView({
    state,
    parent: editorDiv
  });
}

  function formatTime(totalSeconds: number): string {
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds < 10 ? '0' : ''}${seconds}`;
  }

  $: { 
    if (typeof document !== 'undefined' && document.documentElement) {
      const root = document.documentElement.classList;
      if (!$theme) { 
      } else if (root.contains('light') && $theme === 'dark') {
        root.remove('light');
        root.add('dark');
      } else if (root.contains('dark') && $theme === 'light') {
        root.remove('dark');
        root.add('light');
      } else if (!root.contains('light') && !root.contains('dark')) {
        root.add($theme); 
      }
    }
  }
</script>

<main>
  <h1>salieri</h1>
  <div class="page-content">
    <div class="left-panel tasks-list-container">
      <h2>tasks!</h2>
      {#if $tasks.length > 0}
        <ul>
          {#each $tasks as task (task.id)} 
            <li class:doing={task.status === 'doing'}>
              {task.title}
            </li>
          {/each}
        </ul>
      {:else}
        <p>not a task yet, get to work!</p>
      {/if}
    </div>
    <div class="center-panel">
      <div class="active-task-display">
        <h3>active task</h3>
        <p>
          {#if $tasks.find(t => t.status === 'doing')}
            {#each $tasks as task (task.id)}
              {#if task.status === 'doing'}
                {task.title}
              {/if}
            {/each}
          {:else}
            no active task.
          {/if}
        </p>
      </div>

      <div class="pomodoro-timer">
        <h3>pomodoro</h3>
        <p>{formatTime($remainingTime)}</p>
        <p>state: {$timerState}</p>
      </div>
    </div>
    <div class="right-pane">
      {#if $showEditor}
      <div class="codemirror_holder" bind:this={editorDiv}>
        </div>
      {/if} 
    </div>
</div>
  <div class="commandline-footer">
    <input
      type="text"
      bind:value={commandInput}
      placeholder="~welcome to salieri"
      on:keydown={handleKeydown}
    />
    {#if commandOutput}
      <p class="greeting-message">{commandOutput}</p>
    {/if}
  </div>
</main>

<style>
  :global(html.dark) {
    --bg: #000;
    --fg: #fff;
    --accent: lime;
    --panel-border: #333; 
    --red-panel-bg: #800000; 
    --red-panel-fg: #fff;
    --yellow-panel-bg: #808000; 
    --yellow-panel-fg: #fff;
  }
  :global(html.light) {
    --bg: #fff;
    --fg: #000;
    --accent: red;
    --panel-border: #ccc; 
    --red-panel-bg: #ffcccb;
    --red-panel-fg: #000;
    --yellow-panel-bg: #ffffcc; 
    --yellow-panel-fg: #000;
  }

  :global(html),
  :global(body) {
    margin: 0;
    padding: 0;
    height: 100%;
    width: 100%;
    background: var(--bg);
    color: var(--fg);
    font-family: sans-serif;
    overflow-x: hidden; 
  }

  main {
    display: flex;
    flex-direction: column; 
    height: 100vh;
    width: 100%;
    box-sizing: border-box;
  }

  h1 { /* header */
    color: var(--accent);
    text-transform: lowercase;
    font-size: 3em; 
    font-weight: 200;
    text-align: center;
    margin: 0.3em 0; 
    flex-shrink: 0; 
  }

  .page-content {
    display: flex;
    flex-direction: row; 
    flex-grow: 1; 
    overflow-y: hidden; 
    width: 100%;
    box-sizing: border-box;
  }

  .left-panel.tasks-list-container {
    flex-basis: 20%;
    background-color: var(--red-panel-bg);
    color: var(--red-panel-fg);
    padding: 1em;
    overflow-y: auto; 
    box-sizing: border-box;
    border-right: 1px solid var(--panel-border);
  }
  .left-panel.tasks-list-container h2 {
    margin-top: 0;
    font-size: 1.5em;
    color: var(--accent);
  }
  .left-panel.tasks-list-container ul {
    list-style: none;
    padding: 0;
  }
  .left-panel.tasks-list-container li {
    padding: 0.3em 0.1em;
    border-bottom: 1px solid color-mix(in srgb, var(--red-panel-fg) 20%, transparent);
  }
  .left-panel.tasks-list-container li:last-child {
    border-bottom: none;
  }


  .center-panel {
    flex-basis: 30%; 
    padding: 1em;
    display: flex;
    flex-direction: column;
    align-items: center; 
    text-align: center;
    overflow-y: auto;
    box-sizing: border-box;
    border-right: 1px solid var(--panel-border);
  }
  .center-panel h3 {
    font-size: 1.2em;
    color: var(--accent);
    margin-top: 0.5em;
    margin-bottom: 0.3em;
  }
  .active-task-display, .pomodoro-timer {
    margin-bottom: 1em;
  }
  .active-task-display p, .pomodoro-timer p {
    margin: 0.2em 0;
    font-size: 0.9em;
  }

  .right-pane {
    flex-basis: 50%;
    width: 50%;
    background-color: var(--yellow-panel-bg);
    color: var(--yellow-panel-fg);
    padding: 1em; 
    box-sizing: border-box;
    overflow-y: auto;
  }

  .commandline-footer {
    flex-shrink: 0; 
    padding: 0.5em 0;
    width: 100%;
    box-sizing: border-box;
  }

  input[type='text'] {
    padding: 0.8em;
    border-radius: 4px;
    border: 1px solid var(--fg);
    background-color: var(--bg);
    color: var(--fg);
    font-size: 1em;
    display: block;
    margin: 0 auto 0.5em auto; 
    width: 95%;
    box-sizing: border-box;
  }
  input[type='text']:focus {
    border-color: var(--accent);
    outline: none;
    color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 20%, transparent);
  }

  .greeting-message {
    text-align: center;
    font-weight: bold;
    font-size: 1em; 
    color: var(--accent);
    padding: 0 0.5em 0.5em 0.5em; 
    margin:0;
  }

  .doing { 
    font-weight: bold;
    color: var(--accent); 

  }
</style>