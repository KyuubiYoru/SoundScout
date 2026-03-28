<script lang="ts">
  import { searchStore } from "$lib/stores/searchStore";

  let { placeholder = "Search library…" } = $props();

  let value = $state("");

  function onInput() {
    searchStore.search(value);
  }
</script>

<div class="search-bar">
  <input
    id="main-search"
    type="search"
    {placeholder}
    bind:value
    oninput={onInput}
    autocomplete="off"
    spellcheck="false"
    aria-label="Search"
  />
</div>

<style>
  .search-bar input {
    width: 100%;
    padding: var(--spacing-sm) var(--spacing-md);
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-primary);
    font-size: var(--font-size-base);
  }
  .search-bar input:focus {
    outline: none;
    border-color: var(--border-focus);
  }
  .search-bar input::placeholder {
    color: var(--text-muted);
  }
</style>
