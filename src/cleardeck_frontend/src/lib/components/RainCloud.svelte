<script>
  import { onMount } from 'svelte';

  let canvas;
  let drops = [];
  let cloudPuffs = [];
  let animationId;
  let frame = 0;

  onMount(() => {
    const ctx = canvas.getContext('2d');
    canvas.width = canvas.offsetWidth;
    canvas.height = canvas.offsetHeight;

    const cw = canvas.width;
    const ch = canvas.height;
    const cloudY = ch * 0.18;
    const cloudCenterX = cw * 0.5;

    // Build cloud shape from overlapping circles
    cloudPuffs = [
      { x: cloudCenterX - 35, y: cloudY + 5, r: 22 },
      { x: cloudCenterX - 15, y: cloudY - 8, r: 28 },
      { x: cloudCenterX + 8, y: cloudY - 12, r: 32 },
      { x: cloudCenterX + 30, y: cloudY - 4, r: 26 },
      { x: cloudCenterX + 48, y: cloudY + 6, r: 20 },
      // fill gaps
      { x: cloudCenterX - 5, y: cloudY + 8, r: 24 },
      { x: cloudCenterX + 20, y: cloudY + 8, r: 22 },
    ];

    function spawnDrop() {
      const spreadX = cloudCenterX + (Math.random() - 0.5) * 80;
      drops.push({
        x: spreadX,
        y: cloudY + 18 + Math.random() * 8,
        speed: 2.5 + Math.random() * 2.5,
        length: 8 + Math.random() * 10,
        opacity: 0.4 + Math.random() * 0.4,
        width: 1 + Math.random() * 1.2
      });
    }

    function drawCloud() {
      // Dark shadow
      ctx.save();
      ctx.translate(3, 4);
      for (const p of cloudPuffs) {
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
        ctx.fillStyle = 'rgba(30, 30, 50, 0.3)';
        ctx.fill();
      }
      ctx.restore();

      // Cloud body — dark stormy grey
      for (const p of cloudPuffs) {
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
        const grad = ctx.createRadialGradient(p.x - 4, p.y - 6, 0, p.x, p.y, p.r);
        grad.addColorStop(0, 'rgba(100, 110, 130, 0.95)');
        grad.addColorStop(1, 'rgba(55, 60, 80, 0.92)');
        ctx.fillStyle = grad;
        ctx.fill();
      }

      // Lightning flash every ~60 frames
      if (frame % 120 < 3) {
        ctx.save();
        ctx.globalAlpha = 0.6;
        for (const p of cloudPuffs) {
          ctx.beginPath();
          ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
          ctx.fillStyle = 'rgba(200, 200, 255, 0.4)';
          ctx.fill();
        }
        ctx.restore();

        // Lightning bolt
        const boltX = cloudCenterX + (Math.random() - 0.5) * 30;
        ctx.beginPath();
        ctx.moveTo(boltX, cloudY + 20);
        let lx = boltX;
        let ly = cloudY + 20;
        for (let seg = 0; seg < 4; seg++) {
          lx += (Math.random() - 0.5) * 14;
          ly += 10 + Math.random() * 8;
          ctx.lineTo(lx, ly);
        }
        ctx.strokeStyle = 'rgba(255, 255, 200, 0.8)';
        ctx.lineWidth = 2;
        ctx.stroke();
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.4)';
        ctx.lineWidth = 5;
        ctx.stroke();
      }
    }

    function animate() {
      ctx.clearRect(0, 0, cw, ch);
      frame++;

      // Spawn rain drops
      if (frame % 2 === 0) {
        spawnDrop();
        spawnDrop();
      }

      // Draw rain
      for (let i = drops.length - 1; i >= 0; i--) {
        const d = drops[i];
        d.y += d.speed;

        // Slight wind
        d.x += 0.3;

        if (d.y > ch + 10) {
          drops.splice(i, 1);
          continue;
        }

        ctx.beginPath();
        ctx.moveTo(d.x, d.y);
        ctx.lineTo(d.x - 0.3 * d.length * 0.3, d.y - d.length);
        ctx.strokeStyle = `rgba(120, 160, 220, ${d.opacity})`;
        ctx.lineWidth = d.width;
        ctx.lineCap = 'round';
        ctx.stroke();

        // Splash at bottom
        if (d.y > ch - 15 && d.y < ch - 10) {
          ctx.beginPath();
          ctx.arc(d.x, ch - 10, 3, Math.PI, 0);
          ctx.strokeStyle = `rgba(120, 160, 220, ${d.opacity * 0.5})`;
          ctx.lineWidth = 0.8;
          ctx.stroke();
        }
      }

      // Draw cloud on top of rain
      drawCloud();

      animationId = requestAnimationFrame(animate);
    }

    animate();

    return () => {
      if (animationId) cancelAnimationFrame(animationId);
    };
  });
</script>

<canvas bind:this={canvas} class="raincloud-canvas"></canvas>

<style>
  .raincloud-canvas {
    position: absolute;
    top: -70px;
    left: -40px;
    width: calc(100% + 80px);
    height: calc(100% + 100px);
    pointer-events: none;
    z-index: 100;
  }
</style>
