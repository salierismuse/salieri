import { writable } from 'svelte/store';
import { browser } from '$app/environment';


export type Task = {
  id: string;
  title: string;
  created_at: string;
  status: string;
};

export const tasks = writable<Task[]>([]);

// Create a custom store that handles persistence
function createThemeStore() {
    const { subscribe, set, update } = writable<'light' | 'dark'>('dark');

    return {
        subscribe,
        set: (value: 'light' | 'dark') => {
            if (browser) {
                document.documentElement.classList.remove('light', 'dark');
                document.documentElement.classList.add(value);
            }
            set(value);
        },
        update
    };
}

export const theme = createThemeStore();