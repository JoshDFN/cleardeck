<script>
  import { soundManager } from '$lib/sounds';
  
  let enabled = $state(soundManager.enabled);
  let volume = $state(soundManager.volume);
  let showSettings = $state(false);
  
  function toggleSounds() {
    enabled = !enabled;
    soundManager.setEnabled(enabled);
    if (enabled) {
      soundManager.playNotification();
    }
  }
  
  function setVolume(value) {
    volume = value;
    soundManager.setVolume(volume);
    soundManager.playNotification();
  }
</script>

<div class="sound-settings">
  <button class="sound-toggle" onclick={() => showSettings = !showSettings} title="Sound Settings">
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      {#if enabled}
        <path d="M3 18v-6a9 9 0 0 1 18 0v6"/>
        <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"/>
        <path d="M9 9v6M15 9v6"/>
      {:else}
        <path d="M3 18v-6a9 9 0 0 1 18 0v6"/>
        <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"/>
        <line x1="9" y1="9" x2="21" y2="21"/>
      {/if}
    </svg>
  </button>
  
  {#if showSettings}
    <div class="settings-panel">
      <div class="setting-item">
        <label>
          <input type="checkbox" bind:checked={enabled} onchange={toggleSounds} />
          <span>Enable Sounds</span>
        </label>
      </div>
      {#if enabled}
        <div class="setting-item">
          <label>Volume: {Math.round(volume * 100)}%</label>
          <input type="range" min="0" max="1" step="0.1" bind:value={volume} oninput={(e) => setVolume(e.target.value)} />
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .sound-settings {
    position: relative;
  }
  
  .sound-toggle {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #888;
    width: 36px;
    height: 36px;
    border-radius: 8px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;
  }
  
  .sound-toggle:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }
  
  .settings-panel {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    min-width: 200px;
    background: #1a1a2e;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 12px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
    z-index: 100;
  }
  
  .setting-item {
    margin-bottom: 12px;
  }
  
  .setting-item:last-child {
    margin-bottom: 0;
  }
  
  .setting-item label {
    display: flex;
    align-items: center;
    gap: 8px;
    color: white;
    font-size: 13px;
    cursor: pointer;
  }
  
  .setting-item input[type="checkbox"] {
    width: 16px;
    height: 16px;
    cursor: pointer;
  }
  
  .setting-item input[type="range"] {
    width: 100%;
    margin-top: 8px;
  }
</style>
