<script>
  import { onMount, onDestroy } from 'svelte';
  
  let isOnline = $state(true);
  let lastPing = $state(Date.now());
  let pingInterval = null;
  
  onMount(() => {
    // Check online/offline status
    isOnline = navigator.onLine;
    
    const handleOnline = () => {
      isOnline = true;
      lastPing = Date.now();
    };
    
    const handleOffline = () => {
      isOnline = false;
    };
    
    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);
    
    // Ping check every 5 seconds
    pingInterval = setInterval(() => {
      if (navigator.onLine) {
        lastPing = Date.now();
        isOnline = true;
      } else {
        isOnline = false;
      }
    }, 5000);
    
    return () => {
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
      if (pingInterval) clearInterval(pingInterval);
    };
  });
  
  onDestroy(() => {
    if (pingInterval) clearInterval(pingInterval);
  });
  
  const timeSinceLastPing = $derived(Math.floor((Date.now() - lastPing) / 1000));
  const isLagging = $derived(timeSinceLastPing > 10);
</script>

<div class="connection-status" class:online={isOnline} class:offline={!isOnline} class:lagging={isLagging}>
  <div class="status-indicator">
    <div class="dot"></div>
    <span class="text">
      {#if !isOnline}
        Offline
      {:else if isLagging}
        Lagging ({timeSinceLastPing}s)
      {:else}
        Online
      {/if}
    </span>
  </div>
</div>

<style>
  .connection-status {
    position: fixed;
    top: 80px;
    right: 20px;
    z-index: 50;
    padding: 8px 12px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    font-size: 12px;
    transition: all 0.3s;
  }

  .status-indicator {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #2ecc71;
    animation: pulse 2s ease-in-out infinite;
  }

  .connection-status.offline .dot {
    background: #ef4444;
    animation: none;
  }

  .connection-status.lagging .dot {
    background: #f59e0b;
  }

  .text {
    color: #888;
    font-weight: 500;
  }

  .connection-status.online .text {
    color: #2ecc71;
  }

  .connection-status.offline .text {
    color: #ef4444;
  }

  .connection-status.lagging .text {
    color: #f59e0b;
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.5;
      transform: scale(0.8);
    }
  }
</style>
