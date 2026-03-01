import { bench, describe, it, expect } from 'vitest';
import { spawn } from 'node:child_process';

const TIMEOUT_MS = 10000;

async function measureStartupTime(): Promise<number> {
  const start = Date.now();

  return new Promise((resolve, reject) => {
    const proc = spawn('pnpm', ['tauri', 'dev', '--', '--no-default-window'], {
      cwd: process.cwd(),
      stdio: 'pipe',
      shell: true,
    });

    let output = '';
    let resolved = false;

    const timeout = setTimeout(() => {
      if (!resolved) {
        resolved = true;
        proc.kill();
        resolve(Date.now() - start);
      }
    }, TIMEOUT_MS);

    proc.stdout?.on('data', (data) => {
      output += data.toString();
      if (output.includes('ready') && !resolved) {
        resolved = true;
        clearTimeout(timeout);
        proc.kill();
        resolve(Date.now() - start);
      }
    });

    proc.stderr?.on('data', (data) => {
      output += data.toString();
    });

    proc.on('error', (err) => {
      if (!resolved) {
        resolved = true;
        clearTimeout(timeout);
        reject(err);
      }
    });

    proc.on('close', () => {
      if (!resolved) {
        resolved = true;
        clearTimeout(timeout);
        resolve(Date.now() - start);
      }
    });
  });
}

describe('Startup Performance Benchmark', () => {
  bench(
    'cold startup time should be under 3 seconds',
    async () => {
      const startupTime = await measureStartupTime();
      console.log(`Startup time: ${startupTime}ms`);
    },
    { time: 30000, iterations: 1 }
  );
});

describe('Startup Performance Validation', () => {
  it('should start within acceptable time', async () => {
    const startupTime = await measureStartupTime();
    console.log(`Measured startup time: ${startupTime}ms`);
    expect(startupTime).toBeLessThan(3000);
  }, 30000);
});
