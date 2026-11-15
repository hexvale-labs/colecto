// A pixel cat on a `canvas` overlay. The shell hands this Worker an OffscreenCanvas covering the
// window (or asks for finished frames where OffscreenCanvas is missing) and tells it where the
// pointer is; the cat runs toward it, sits nearby, and naps when nothing happens.
const SIZE = 32;
let settings = { speed: 10, opacity: 100, stop_distance: 44 };
let canvas = null;        // OffscreenCanvas we draw on
let frames = false;       // true: send each frame as an ImageBitmap instead
let view = { width: 0, height: 0, ratio: 1 };
let sprite = null;
let pointer = { x: 0, y: 0, inside: false };
const cat = { x: 0, y: 0, idle: 0, tick: 0 };

async function loadSprite(id) {
  const reply = await call('GET', `/extensions/${id}/assets/oneko.png`);
  sprite = await createImageBitmap(new Blob([reply], { type: 'image/png' }));
}

let calls = 0;
const pending = new Map();
function call(method, path) {
  const id = ++calls;
  colecto.send({ t: 'call', id, method, path, bytes: true });
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
}

function resize() {
  if (!canvas) return;
  // In frames mode only the cat travels to the window, so the canvas is one sprite big.
  canvas.width = Math.round((frames ? SIZE : view.width) * view.ratio);
  canvas.height = Math.round((frames ? SIZE : view.height) * view.ratio);
}

/** One step, about every 100 ms, as the original oneko.js. */
function step() {
  cat.tick += 1;
  const maxX = Math.max(view.width - SIZE, 0);
  const maxY = Math.max(view.height - SIZE, 0);
  const dx = pointer.x - (cat.x + SIZE / 2);
  const dy = pointer.y - (cat.y + SIZE / 2);
  const distance = Math.max(Math.abs(dx), Math.abs(dy));
  const phase = cat.tick % 2;
  let frame;
  if (pointer.inside && Math.abs(dx) < 22 && Math.abs(dy) < 22) {
    // The pointer landed on the cat: hop beside it.
    cat.x = pointer.x + 56 <= view.width ? pointer.x + 24 : pointer.x - 56;
    cat.y = pointer.y + 56 <= view.height ? pointer.y + 24 : pointer.y - 56;
    cat.idle = 0;
    frame = [7, 3];
  } else if (pointer.inside && distance > settings.stop_distance) {
    if (cat.idle > 1) {
      // Wake up for a few frames before running.
      cat.idle = Math.min(cat.idle, 7) - 1;
      frame = [7, 3];
    } else {
      cat.x += dx / distance * settings.speed;
      cat.y += dy / distance * settings.speed;
      const horizontal = Math.abs(dx) > distance * 0.5;
      const vertical = Math.abs(dy) > distance * 0.5;
      const pick = horizontal && vertical ? (dx > 0 ? (dy > 0 ? [[5, 1], [5, 2]] : [[0, 2], [0, 3]]) : (dy > 0 ? [[5, 3], [6, 1]] : [[1, 0], [1, 1]]))
        : horizontal ? (dx > 0 ? [[3, 0], [3, 1]] : [[4, 2], [4, 3]])
        : dy > 0 ? [[6, 3], [7, 2]] : [[1, 2], [1, 3]];
      frame = pick[phase];
    }
  } else {
    cat.idle += 1;
    frame = cat.idle > 100 ? [2, Math.floor(cat.tick / 4) % 2]
      : cat.idle > 92 ? [3, 2]
      : cat.idle > 40 && cat.idle < 52 ? [5 + cat.tick % 3, 0]
      : [3, 3];
  }
  cat.x = Math.min(Math.max(cat.x, 0), maxX);
  cat.y = Math.min(Math.max(cat.y, 0), maxY);
  draw(frame);
}

function draw([column, row]) {
  if (!canvas || !sprite) return;
  const context = canvas.getContext('2d');
  const x = Math.round(cat.x), y = Math.round(cat.y);
  context.setTransform(view.ratio, 0, 0, view.ratio, 0, 0);
  context.clearRect(0, 0, view.width, view.height);
  context.imageSmoothingEnabled = false;
  context.globalAlpha = settings.opacity / 100;
  context.drawImage(sprite, column * SIZE, row * SIZE, SIZE, SIZE, frames ? 0 : x, frames ? 0 : y, SIZE, SIZE);
  if (frames) { const bitmap = canvas.transferToImageBitmap(); colecto.send({ t: 'frame', bitmap: { $buffer: 0 }, x, y }, [bitmap]); }
}

colecto.onmessage = (message) => {
  switch (message.t) {
    case 'start':
      settings = { ...settings, ...message.settings };
      void loadSprite(message.id).catch(() => {});
      setInterval(step, 100);
      break;
    case 'settings': settings = { ...settings, ...message.settings }; break;
    case 'canvas':
      view = { width: message.width, height: message.height, ratio: message.ratio };
      frames = message.mode === 'frames';
      canvas = frames ? (typeof OffscreenCanvas === 'function' ? new OffscreenCanvas(1, 1) : null) : message.canvas;
      resize();
      break;
    case 'resize': view = { width: message.width, height: message.height, ratio: message.ratio }; resize(); break;
    case 'pointer': pointer = { x: message.x, y: message.y, inside: message.inside }; break;
    case 'reply': {
      const waiting = pending.get(message.id);
      pending.delete(message.id);
      if (message.ok) waiting?.resolve(message.data); else waiting?.reject(new Error(message.error));
      break;
    }
    case 'ping': colecto.send({ t: 'pong', n: message.n }); break;
  }
};
