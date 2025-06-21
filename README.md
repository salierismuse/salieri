Salieri
A minimalist productivity application combining task management with Pomodoro technique in a terminal-inspired interface.
Overview
Salieri is a desktop application built with Tauri and Svelte that helps you focus on what matters. It features a clean, distraction-free interface inspired by terminal aesthetics, combining simple task management with integrated Pomodoro timers.
Features

Task Management: Create and track up to 5 daily tasks
Pomodoro Timer: 25-minute work sessions with automatic breaks
Code Editor: Integrated editor with syntax highlighting
Note Taking: Rich text editing with Markdown support
Command Palette: Keyboard-driven interface for all actions
Theme Support: Dark and light mode switching
File Management: Organized file system with automatic directory creation

![image](https://github.com/user-attachments/assets/0e8565a8-db9c-402c-99f8-ce75224914d8)


Installation
Prerequisites

Rust (latest stable)
Node.js 16+
npm or yarn

Build from Source
bashgit clone https://github.com/yourusername/salieri.git
cd salieri
npm install
npm run tauri dev
Quick Start
Press Ctrl +  ` to open the command palette and try these commands:
bash/todo write documentation    # Create a task
/doing write documentation   # Start working on a task
/start                      # Begin Pomodoro timer
/code myfile.js            # Open code editor
/theme toggle              # Switch themes
Architecture
Frontend

Framework: Svelte with TypeScript
Editor: CodeMirror for code editing, TipTap for rich text
Styling: CSS custom properties for theming

Backend

Runtime: Tauri (Rust)
Storage: JSON files with Serde serialization
Task Management: Async timer loops with persistent state
File System: Automatic directory creation in ~/salieri_files/

Key Modules
src-tauri/src/
├── main.rs        # Application entry point
├── tasks.rs       # Task management and persistence
├── pomodoro.rs    # Timer functionality
├── commands.rs    # Command palette routing
├── theme.rs       # Theme management
└── fileaccess.rs  # File operations
Data Storage

Tasks: tasks_store.json in local app data directory
Settings: settings.json for themes and preferences
User Statistics: user.json for completed tasks and Pomodoros

