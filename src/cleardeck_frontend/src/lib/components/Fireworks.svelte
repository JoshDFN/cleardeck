<script>
  import { onMount } from 'svelte';

  let canvas;
  let particles = [];
  let animationId;
  let launchCount = 0;
  const MAX_LAUNCHES = 5;

  onMount(() => {
    const ctx = canvas.getContext('2d');
    canvas.width = canvas.offsetWidth;
    canvas.height = canvas.offsetHeight;

    function createBurst(x, y) {
      const colors = ['#FFD700', '#FF6B35', '#00D4AA', '#FF3E6C', '#A855F7', '#3B82F6', '#FBBF24', '#34D399'];
      const count = 40 + Math.floor(Math.random() * 20);
      for (let i = 0; i < count; i++) {
        const angle = (Math.PI * 2 * i) / count + (Math.random() - 0.5) * 0.3;
        const speed = 2 + Math.random() * 4;
        particles.push({
          x, y,
          vx: Math.cos(angle) * speed,
          vy: Math.sin(angle) * speed,
          life: 1,
          decay: 0.012 + Math.random() * 0.008,
          color: colors[Math.floor(Math.random() * colors.length)],
          size: 2 + Math.random() * 3,
          trail: []
        });
      }
    }

    function launchRocket() {
      if (launchCount >= MAX_LAUNCHES) return;
      launchCount++;
      const x = canvas.width * (0.2 + Math.random() * 0.6);
      const targetY = canvas.height * (0.15 + Math.random() * 0.35);
      const startY = canvas.height;

      let rocketY = startY;
      const rocketSpeed = 4 + Math.random() * 3;

      function moveRocket() {
        rocketY -= rocketSpeed;
        // Draw rocket trail
        ctx.beginPath();
        ctx.arc(x, rocketY, 2, 0, Math.PI * 2);
        ctx.fillStyle = '#FFD700';
        ctx.fill();
        // Small trail particles
        particles.push({
          x: x + (Math.random() - 0.5) * 4,
          y: rocketY,
          vx: (Math.random() - 0.5) * 0.5,
          vy: Math.random() * 2,
          life: 0.6,
          decay: 0.03,
          color: '#FFA500',
          size: 1.5,
          trail: []
        });

        if (rocketY <= targetY) {
          createBurst(x, rocketY);
          // Schedule next launch
          if (launchCount < MAX_LAUNCHES) {
            setTimeout(launchRocket, 300 + Math.random() * 600);
          }
        } else {
          requestAnimationFrame(moveRocket);
        }
      }
      moveRocket();
    }

    function animate() {
      ctx.clearRect(0, 0, canvas.width, canvas.height);

      for (let i = particles.length - 1; i >= 0; i--) {
        const p = particles[i];
        p.x += p.vx;
        p.y += p.vy;
        p.vy += 0.05; // gravity
        p.vx *= 0.99; // friction
        p.life -= p.decay;

        if (p.life <= 0) {
          particles.splice(i, 1);
          continue;
        }

        // Glow
        ctx.globalAlpha = p.life * 0.3;
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size * 3, 0, Math.PI * 2);
        ctx.fillStyle = p.color;
        ctx.fill();

        // Core
        ctx.globalAlpha = p.life;
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
        ctx.fillStyle = p.color;
        ctx.fill();
      }

      ctx.globalAlpha = 1;

      if (particles.length > 0 || launchCount < MAX_LAUNCHES) {
        animationId = requestAnimationFrame(animate);
      }
    }

    // Start the show
    setTimeout(() => launchRocket(), 200);
    setTimeout(() => launchRocket(), 500);
    animate();

    return () => {
      if (animationId) cancelAnimationFrame(animationId);
    };
  });
</script>

<canvas bind:this={canvas} class="fireworks-canvas"></canvas>

<style>
  .fireworks-canvas {
    position: absolute;
    top: -60px;
    left: -60px;
    width: calc(100% + 120px);
    height: calc(100% + 120px);
    pointer-events: none;
    z-index: 100;
  }
</style>
