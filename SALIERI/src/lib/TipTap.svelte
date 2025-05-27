<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { Editor } from '@tiptap/core'
  import StarterKit from '@tiptap/starter-kit'
  
  export let editor: Editor
  let element: HTMLElement
  
  onMount(() => {
    editor = new Editor({
      element: element,
      extensions: [StarterKit],
      content: '<p>Hello World!</p>',
    })
  })
</script>

<div bind:this={element}></div>