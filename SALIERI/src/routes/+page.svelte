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
  import { javascript } from '@codemirror/lang-javascript'
  import { oneDark } from '@codemirror/theme-one-dark'
  import { indentWithTab } from '@codemirror/commands'
  import { linter, lintGutter } from '@codemirror/lint'
  import { acceptCompletion, autocompletion } from '@codemirror/autocomplete'
  import { indentMore } from '@codemirror/commands'
  import { indentLess } from '@codemirror/commands'
  import { markdown } from "@codemirror/lang-markdown";
 // import Tiptap from '$lib/TipTap.svelte'
  import { Editor } from '@tiptap/core'
  import StarterKit from '@tiptap/starter-kit'



  export const theme = writable<'light' | 'dark'>('dark');
  export const timerState = writable<'Idle' | 'Running' | 'Paused' | 'ShortBreak' | 'LongBreak'>('Idle');
  export let remainingTime = writable(25 * 60);

  let unlistenTheme: () => void;
  let unlistenTimer: () => void;
  let commandInput = '';
  let commandOutput = '';
  let done = false; 
  let currentLogicalDay = writable('');
  let currentTaskDayDisplay = writable('');
  let editorDiv: HTMLDivElement;
  const showEditor = writable(false);
  let myView: EditorView | null = null;
  let showCommands = false;
  let working_code_doc;
  let currFile = '';
  let elementTip;
  let element: HTMLElement;
  let editor: Editor;
  let tiptapBool = false;
  let currentDayOffset = 0;

  const commands = [
    { cmd: '/todo [task]', desc: 'add new task' },
    { cmd: '/doing [task]', desc: 'start working on task' },
    { cmd: '/done [task]', desc: 'mark task complete' },
    { cmd: '/break [task]', desc: 'pause current task' },
    { cmd: '/delete [task]', desc: 'delete task' },
    { cmd: '/start', desc: 'begin pomodoro' },
    { cmd: '/pause', desc: 'pause timer' },
    { cmd: '/resume', desc: 'resume timer' },
    { cmd: '/stop', desc: 'stop timer' },
    { cmd: '/code', desc: 'toggle code editor' },
    { cmd: '/wq', desc: 'save and exit your work'},
    { cmd: '/write', desc: 'take notes, or write the next american novel'},
    { cmd: '/theme [dark|light|toggle]', desc: 'change theme' }
  ];

  onMount(async () => {
    try {
      const initialTheme = await invoke<'light' | 'dark'>('get_current_theme');
      theme.set(initialTheme);

      const logicalDayKey = await invoke<string>('get_current_logical_day_key');
      currentLogicalDay.set(logicalDayKey);
      currentTaskDayDisplay = currentLogicalDay;
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

    } catch (e) {
      console.error('failed to initialize:', e);
    }
  });

  onDestroy(() => {
    unlistenTheme?.();
    unlistenTimer?.();
    if (editor) {
    editor.destroy()
    }
  });

  // task handling
async function handlePrev() {
  currentDayOffset += 1;
  console.log('handlePrev called, currentDayOffset is now', currentDayOffset);

  // must be exactly "days_offset"
const payload = { days_offset: currentDayOffset };
console.log('calling get_current_logical_day_key with payload:', JSON.stringify(payload));
  const currentTaskDayDisplayKey = await invoke<string>(
    'get_current_logical_day_key',
    { daysOffset: currentDayOffset }
  );
  console.log('get_current_logical_day_key returned:', currentTaskDayDisplayKey);

  const newTasks = await invoke<Task[]>('get_tasks', {
    day: currentTaskDayDisplayKey,
    done,
    days_offset: currentDayOffset
  });
  console.log('get_tasks returned:', newTasks);

  tasks.set(newTasks);
  console.log('Tasks after loading:', get(tasks));
}

async function handleNext() {
  currentDayOffset -= 1;

const payload = { days_offset: currentDayOffset };
  const currentTaskDayDisplayKey = await invoke<string>(
    'get_current_logical_day_key',
    { daysOffset: currentDayOffset }
  );

  const newTasks = await invoke<Task[]>('get_tasks', {
    day: currentTaskDayDisplayKey,
    done,
    days_offset: currentDayOffset
  });

  tasks.set(newTasks);
}

  async function toggleWriter(content: string) {
      editor = new Editor({
      element: element,
      extensions: [
         StarterKit,
      ],
      content: content,
      })
  }

  async function submitCommand() {
    if (commandInput.trim() === '') return;
    const cmd = commandInput.trim();
    commandInput = '';
    showCommands = false;

    if (cmd === '/?') {
      showCommands = !showCommands;
      return;
    }



    try {
      commandOutput = await invoke('handle_palette_command', { command: cmd, days_offset: currentDayOffset });
    } catch (e) {
      commandOutput = `error: ${e}`;
    }


      if (cmd.startsWith('/write')) {
      const parts = cmd.split(' ');
      currFile = parts.slice(1).join(' ').trim();
      tiptapBool = true;  
      await tick();
      toggleWriter(commandOutput as string);

      return;
    }

    const dayToLoad = $currentLogicalDay || new Date().toLocaleDateString('en-CA');

    if (cmd.startsWith('/wq')) {
      // for when we add "save as"
      //  currFile = cmd.slice(3);

      // general function
      if (myView) {
        if (!currFile) {
          commandOutput = 'error: no file is open';
          return;
        }
        const currentValue = myView.state.doc.toString();
        try {
          await invoke('save_file', { userPath: currFile, information: currentValue });
        } catch (e) {
          commandOutput = `error saving: ${e}`;
          return;
        }
        toggleEditor(null);
      }

      else if (tiptapBool) {
        const content = editor.getHTML();
        await invoke('save_file', { userPath: currFile, information: content});
        await tick();
        tiptapBool = false;
      }
      else {
        Error("file not open");
      }
      return;
    }

    if (cmd.startsWith('/code')) {

      // set current file and then call being editor functions. 
      const parts = cmd.split(' ');
      currFile = parts.slice(1).join(' ').trim();
      toggleEditor(commandOutput);
      commandOutput = "opened successfully";
      return;
    }

    // timer commands
    if (cmd === '/start') await invoke('start_timer');
    else if (cmd === '/pause') await invoke('pause_timer');
    else if (cmd === '/stop') await invoke('stop_timer');

    // task filter state update
    if (cmd.startsWith('/todo')) 
    {
      done = false;
      currentDayOffset = 0;
    }
    if (cmd.startsWith('/nextDay')) {
      handleNext();
    }
    if (cmd.startsWith('/prevDay'))
    {
      handlePrev();
    }
    else if (cmd.startsWith('/completed')) done = true;

    await load_tasks_for_day(dayToLoad, done);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      submitCommand();
    } else if (event.key === 'Escape') {
      showCommands = false;
    }
  }

  async function load_tasks_for_day(day: string, taskDoneStatus = false) {
    try {
      const list = await invoke<Task[]>('get_tasks', { day, done: taskDoneStatus, days_offset: currentDayOffset });
      tasks.set(list);
    } catch (e) {
      console.error('failed loading tasks:', e);
    }
  }

  async function toggleEditor(pathToLoad: string | null = null) {
    const next = !get(showEditor);
    showEditor.set(next);

    if (next) {
      await tick();
      await mountEditor(pathToLoad ?? '~');  
    } else {
      myView?.destroy();
      myView = null;
    }
  }

  async function mountEditor(filePath: string) {
    if (!editorDiv) return;

    myView?.destroy();

    let fileContent = '';
    try {
      fileContent = filePath; 
    } catch(e) {
      fileContent = "error";
    }

    const tabKeymap = keymap.of([
    {
      key: 'Tab',
      preventDefault: true,
      run: (view) => {
        // check for autocompletion first
        if (acceptCompletion(view)) return true;
        // then handle indentation
        return indentMore(view);
      }
    },
    {
      key: 'Shift-Tab',
      preventDefault: true, 
      run: (view) => {
        // handle shift-tab for unindent
        return indentLess(view);
      }
    }
  ]);
    let codeType;
    const state = EditorState.create({
      doc: fileContent,
      extensions: [basicSetup,
       javascript(),
       oneDark, 
       autocompletion(), tabKeymap,]
    });

    myView = new EditorView({
      state,
      parent: editorDiv,
    });
    myView.focus();
  }

  function formatTime(totalSeconds: number): string {
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;
  }

  function getTimerStateIcon(state: string): string {
    switch (state) {
      case 'Running': return '▶';
      case 'Paused': return '⏸';
      case 'ShortBreak': return '☕';
      case 'LongBreak': return '🌙';
      default: return '⏹';
    }
  }

  function getTimerProgress(): number {
    const total = $timerState === 'ShortBreak' ? 5 * 60 : 
                  $timerState === 'LongBreak' ? 15 * 60 : 
                  5 * 60; // work duration
    return (1 - $remainingTime / total) * 100;
  }

  $: activeTask = $tasks.find(t => t.status === 'doing');
  $: todoTasks = $tasks.filter(t => t.status === 'todo');
  $: doneTasks = $tasks.filter(t => t.status === 'done');

  // Update active task time when timer ticks
  $: if ($timerState === 'Running' && activeTask) {
    tasks.update(tasks => {
      return tasks.map(task => {
        if (task.id === activeTask.id) {
          return { ...task, time_spent: task.time_spent + 1 };
        }
        return task;
      });
    });
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
  <header>
    <h1>salieri</h1>
    <div class="status-bar">
      <span class="day-indicator">{$currentLogicalDay}</span>
      <span class="task-count">{todoTasks.length} active • {doneTasks.length} done</span>
    </div>
  </header>

  <div class="workspace">
    <!-- Focus Zone -->
    <section class="focus-zone">
      <div class="active-task-card">
        <h2>focus</h2>
        {#if activeTask}
          <div class="task-active">
            <div class="task-title">{activeTask.title}</div>
            <div class="task-timer">{Math.floor(activeTask.time_spent / 60)}m {activeTask.time_spent % 60}</div>
          </div>
        {:else}
          <div class="task-idle">
            <div class="prompt">ready to focus?</div>
            <div class="hint">use /doing [task] to begin</div>
          </div>
        {/if}
      </div>

      <div class="pomodoro-card">
        <div class="timer-display">
          <div class="timer-icon">{getTimerStateIcon($timerState)}</div>
          <div class="timer-time">{formatTime($remainingTime)}</div>
          <div class="timer-state">{$timerState.toLowerCase()}</div>
        </div>
        <div class="timer-progress">
          <div class="progress-bar" style="width: {getTimerProgress()}%"></div>
        </div>
      </div>
    </section>

    <!-- Task Panel -->
    <section class="task-panel">
      <div class="task-section">
        <h3>todo</h3>
        <div class="task-list">
          {#if todoTasks.length === 0}
            <div class="empty-state">all clear</div>
          {:else}
            {#each todoTasks as task (task.id)}
              <div class="task-item" class:active={task.status === 'doing'}>
                <div class="task-dot"></div>
                <span class="task-text">{task.title}</span>
              </div>
            {/each}
          {/if}
        </div>
      </div>

      {#if doneTasks.length > 0}
        <div class="task-section completed">
          <h3>completed</h3>
          <div class="task-list">
            {#each doneTasks.slice(0, 5) as task (task.id)}
              <div class="task-item done">
                <div class="task-check">✓</div>
                <span class="task-text">{task.title}</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </section>

  
    {#if tiptapBool}
      <div bind:this={element}>
      </div>
    {/if}

    <!-- Editor Panel -->
    {#if $showEditor}
      <section class="editor-panel">
        <div class="editor-header">
          <span>code</span>
         <!---- <button class="close-btn" on:click={toggleEditor}>×</button> -->
        </div>
        <div class="editor-container" bind:this={editorDiv}></div>
      </section>
    {/if}
  </div>

  <!-- Command Line -->
  <footer class="command-line">
    {#if showCommands}
      <div class="commands-help">
        {#each commands as cmd}
          <div class="command-item">
            <code>{cmd.cmd}</code>
            <span>{cmd.desc}</span>
          </div>
        {/each}
      </div>
    {/if}
    
    <div class="input-section">
      <div class="prompt-symbol">$</div>
      <input
        type="text"
        bind:value={commandInput}
        placeholder="type a command or /? for help"
        on:keydown={handleKeydown}
        class="command-input"
      />
    </div>
    
    {#if commandOutput}
      <div class="output">{commandOutput}</div>
    {/if}
  </footer>
</main>

<style>
  :global(html.dark) {
    --bg-primary: #0a0a0a;
    --bg-secondary: #111111;
    --bg-tertiary: #1a1a1a;
    --fg-primary: #ffffff;
    --fg-secondary: #b3b3b3;
    --fg-muted: #666666;
    --accent: #00ff41;
    --accent-dim: #00cc33;
    --accent-dark: #009926;
    --border: #333333;
    --error: #ff4444;
    --warning: #ffaa00;
  }

  :global(html.light) {
    --bg-primary: #ffffff;
    --bg-secondary: #f8f8f8;
    --bg-tertiary: #eeeeee;
    --fg-primary: #000000;
    --fg-secondary: #4d4d4d;
    --fg-muted: #999999;
    --accent: #00aa33;
    --accent-dim: #00cc33;
    --accent-dark: #008822;
    --border: #dddddd;
    --error: #cc0000;
    --warning: #cc8800;
  }

  :global(*) {
    box-sizing: border-box;
  }

  :global(html), :global(body) {
    margin: 0;
    padding: 0;
    height: 100vh;
    background: var(--bg-primary);
    color: var(--fg-primary);
    font-family: 'SF Mono', 'Monaco', 'Consolas', 'Liberation Mono', monospace;
    font-size: 14px;
    line-height: 1.5;
    overflow: hidden;
  }

  main {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-primary);
  }

  header {
    padding: 1rem 2rem;
    border-bottom: 1px solid var(--border);
    background: var(--bg-secondary);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  h1 {
    font-size: 2rem;
    font-weight: 200;
    color: var(--accent);
    margin: 0;
    letter-spacing: 2px;
    text-transform: lowercase;
  }

  .status-bar {
    display: flex;
    gap: 2rem;
    font-size: 0.85rem;
    color: var(--fg-secondary);
  }

  .day-indicator {
    color: var(--accent-dim);
    font-weight: 500;
  }

  .workspace {
    flex: 1;
    display: grid;
    grid-template-columns: 1fr 300px;
    gap: 1px;
    background: var(--border);
    overflow: hidden;
  }

  .workspace.with-editor {
    grid-template-columns: 1fr 300px 1fr;
  }

  .focus-zone {
    background: var(--bg-primary);
    padding: 2rem;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 2rem;
  }

  .active-task-card, .pomodoro-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 2rem;
    text-align: center;
  }

  .active-task-card h2 {
    margin: 0 0 1rem;
    font-size: 1.2rem;
    font-weight: 400;
    color: var(--accent);
    text-transform: lowercase;
  }

  .task-active .task-title {
    font-size: 1.5rem;
    color: var(--fg-primary);
    margin-bottom: 0.5rem;
    font-weight: 300;
  }

  .task-active .task-timer {
    color: var(--accent-dim);
    font-size: 1rem;
  }

  .task-idle .prompt {
    font-size: 1.2rem;
    color: var(--fg-secondary);
    margin-bottom: 0.5rem;
  }

  .task-idle .hint {
    font-size: 0.9rem;
    color: var(--fg-muted);
  }

  .timer-display {
    margin-bottom: 1rem;
  }

  .timer-icon {
    font-size: 2rem;
    margin-bottom: 0.5rem;
  }

  .timer-time {
    font-size: 3rem;
    font-weight: 200;
    color: var(--accent);
    margin-bottom: 0.25rem;
    font-family: 'SF Mono', monospace;
  }

  .timer-state {
    font-size: 0.9rem;
    color: var(--fg-secondary);
    text-transform: lowercase;
  }

  .timer-progress {
    height: 4px;
    background: var(--bg-tertiary);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-bar {
    height: 100%;
    background: var(--accent);
    transition: width 1s ease;
  }

  .task-panel {
    background: var(--bg-secondary);
    padding: 2rem;
    overflow-y: auto;
    border-left: 1px solid var(--border);
  }

  .task-section {
    margin-bottom: 2rem;
  }

  .task-section h3 {
    margin: 0 0 1rem;
    font-size: 1rem;
    font-weight: 400;
    color: var(--accent);
    text-transform: lowercase;
  }

  .task-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .task-item {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem;
    border-radius: 4px;
    transition: background 0.15s ease;
  }

  .task-item:hover {
    background: var(--bg-tertiary);
  }

  .task-item.active {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    border: 1px solid var(--accent-dark);
  }

  .task-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--fg-muted);
    flex-shrink: 0;
  }

  .task-item.active .task-dot {
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent);
  }

  .task-check {
    color: var(--accent);
    font-weight: bold;
    flex-shrink: 0;
  }

  .task-text {
    flex: 1;
    font-size: 0.9rem;
  }

  .task-item.done .task-text {
    color: var(--fg-muted);
    text-decoration: line-through;
  }

  .empty-state {
    color: var(--fg-muted);
    font-style: italic;
    text-align: center;
    padding: 2rem 0;
  }

  .editor-panel {
    background: var(--bg-primary);
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
  }

  .editor-header {
    padding: 1rem;
    border-bottom: 1px solid var(--border);
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: var(--bg-secondary);
  }

  .editor-header span {
    color: var(--accent);
    font-weight: 500;
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--fg-muted);
    font-size: 1.5rem;
    cursor: pointer;
    padding: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    color: var(--fg-primary);
  }

  .editor-container {
    flex: 1;
    overflow: hidden;
  }

  :global(.cm-editor) {
    height: 100%;
    background: var(--bg-primary) !important;
    color: var(--fg-primary) !important;
  }

  :global(.cm-focused) {
    outline: none !important;
  }

  .command-line {
    background: var(--bg-secondary);
    border-top: 1px solid var(--border);
    padding: 1rem 2rem;
  }

  .commands-help {
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1rem;
    margin-bottom: 1rem;
    max-height: 200px;
    overflow-y: auto;
  }

  .command-item {
    display: flex;
    gap: 1rem;
    padding: 0.25rem 0;
    font-size: 0.85rem;
  }

  .command-item code {
    color: var(--accent);
    background: var(--bg-secondary);
    padding: 0.125rem 0.25rem;
    border-radius: 2px;
    min-width: 120px;
  }

  .command-item span {
    color: var(--fg-secondary);
  }

  .input-section {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .prompt-symbol {
    color: var(--accent);
    font-weight: bold;
  }

  .command-input {
    flex: 1;
    background: transparent;
    border: none;
    color: var(--fg-primary);
    font-family: inherit;
    font-size: inherit;
    outline: none;
  }

  .command-input::placeholder {
    color: var(--fg-muted);
  }

  .output {
    margin-top: 0.5rem;
    color: var(--accent-dim);
    font-size: 0.9rem;
  }

  @media (max-width: 1200px) {
    .workspace {
      grid-template-columns: 1fr;
    }
    
    .task-panel {
      border-left: none;
      border-top: 1px solid var(--border);
    }
  }
</style>