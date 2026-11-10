#!/usr/bin/env python3
"""Linux resource measurements of an isolated host and optional browser workload."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import select
import signal
import socket
import statistics
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
WORKLOAD = 'sample-mail-read-compose-v1'


def summarize(samples):
    return {key: {'mean': statistics.mean(sample[key] for sample in samples),
                  'median': statistics.median(sample[key] for sample in samples)}
            for key in samples[0]}


def compare(current, previous):
    if current['compatibility'] != previous['compatibility']:
        return None
    result = {}
    for key, value in current['summary'].items():
        old = previous['summary'].get(key)
        if old is not None:
            result[key] = {'previous_median': old['median'],
                           'median_percent': (value['median'] / old['median'] - 1) * 100 if old['median'] else None}
    return result


def isolated_environment(environment, directory, port):
    result = {key: value for key, value in environment.items() if not key.startswith('COLECTO_')}
    result.update(COLECTO_DATA_DIR=str(directory), COLECTO_PORT=str(port))
    return result


def process_sample(pid):
    base = Path('/proc') / str(pid)
    status = dict(line.split(':', 1) for line in (base / 'status').read_text().splitlines() if ':' in line)
    fields = (base / 'stat').read_text().rsplit(')', 1)[1].split()
    return {'rss_bytes': int(status['VmRSS'].split()[0]) * 1024,
            'peak_rss_bytes': int(status['VmHWM'].split()[0]) * 1024,
            'cpu_seconds': (int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK'),
            'parent': int(fields[1])}


class Sampler:
    def __init__(self, daemon_pid):
        self.daemon_pid = daemon_pid
        self.browser_pid = None
        self.browser_cpu = {}
        self.daemon_peak = 0
        self.browser_peak = 0
        self.last = {}
        self.lock = threading.Lock()
        self.stop = threading.Event()
        self.thread = threading.Thread(target=self.run, daemon=True)

    def snapshot(self):
        with self.lock:
            return self._snapshot()

    def _snapshot(self):
        processes = {}
        for entry in Path('/proc').iterdir():
            if entry.name.isdecimal():
                try:
                    processes[int(entry.name)] = process_sample(int(entry.name))
                except (OSError, KeyError, ValueError, ProcessLookupError):
                    pass
        daemon = processes.get(self.daemon_pid)
        if daemon is None:
            raise RuntimeError('Daemon exited during measurement')
        self.daemon_peak = max(self.daemon_peak, daemon['peak_rss_bytes'])
        tree = {self.browser_pid} if self.browser_pid else set()
        while True:
            descendants = {pid for pid, value in processes.items() if value['parent'] in tree}
            expanded = tree | descendants
            if expanded == tree:
                break
            tree = expanded
        browser_rss = 0
        for pid in tree:
            value = processes.get(pid)
            if value:
                browser_rss += value['rss_bytes']
                self.browser_cpu[pid] = max(self.browser_cpu.get(pid, 0), value['cpu_seconds'])
        self.browser_peak = max(self.browser_peak, browser_rss)
        self.last = {'daemon_rss_bytes': daemon['rss_bytes'], 'daemon_cpu_seconds': daemon['cpu_seconds'],
                     'browser_rss_bytes': browser_rss, 'browser_cpu_seconds': sum(self.browser_cpu.values())}
        return dict(self.last)

    def run(self):
        while not self.stop.wait(0.05):
            try:
                self.snapshot()
            except RuntimeError:
                return

    def close(self):
        self.stop.set()
        self.thread.join()


def wait_line(worker, expected, timeout=90):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        ready, _, _ = select.select([worker.stdout], [], [], 0.1)
        if ready:
            line = worker.stdout.readline().strip()
            if line == expected:
                return
            if line:
                print(f'  browser: {line}', flush=True)
            elif worker.poll() is not None:
                break
    raise RuntimeError(f'Browser did not reach {expected}; exit={worker.poll()}')


def stop_process(process):
    if process is None or process.poll() is not None:
        return
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()


def run_once(binary, with_ui, cycles):
    with tempfile.TemporaryDirectory(prefix='colecto-perf-') as directory:
        with socket.socket() as reserve:
            reserve.bind(('127.0.0.1', 0))
            port = reserve.getsockname()[1]
        base = f'http://127.0.0.1:{port}'
        with tempfile.TemporaryFile(mode='w+') as logs:
            daemon = subprocess.Popen([str(binary)], cwd=ROOT, env=isolated_environment(os.environ, directory, port),
                                      stdout=logs, stderr=logs, start_new_session=True)
            worker = None
            sampler = Sampler(daemon.pid)
            started = time.monotonic()
            try:
                for _ in range(200):
                    if daemon.poll() is not None:
                        logs.seek(0)
                        raise RuntimeError(f'Daemon startup failed: {logs.read()}')
                    try:
                        owner_token = (Path(directory) / 'owner-token').read_text().strip()
                        with urllib.request.urlopen(urllib.request.Request(base + '/api/v1/session', method='POST', headers={'Origin': base, 'Authorization': 'Bearer ' + owner_token}), timeout=1) as response:
                            if response.status == 200:
                                break
                    except (OSError, urllib.error.URLError):
                        time.sleep(0.05)
                else:
                    raise RuntimeError('Daemon did not become ready')
                sampler.thread.start()
                if with_ui:
                    worker = subprocess.Popen(['node', str(ROOT / 'scripts/resource_workload.mjs'), base, str(cycles)],
                                              cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                              text=True, start_new_session=True, env=isolated_environment(os.environ, directory, port))
                    sampler.browser_pid = worker.pid
                    wait_line(worker, 'READY')
                idle_started = sampler.snapshot()
                time.sleep(1)
                initial = sampler.snapshot()
                if worker:
                    worker.stdin.write('RUN\n')
                    worker.stdin.flush()
                    wait_line(worker, 'EXERCISED')
                else:
                    headers = {'Authorization': 'Bearer ' + owner_token, 'x-colecto-extension': 'inbox', 'x-colecto-account': 'demo'}
                    for _ in range(cycles):
                        request = urllib.request.Request(base + '/api/v1/messages', headers=headers)
                        with urllib.request.urlopen(request) as response:
                            if not json.load(response):
                                raise RuntimeError('Expected sample mail')
                time.sleep(1)
                final = sampler.snapshot()
                sample = {'elapsed_seconds': time.monotonic() - started,
                          'daemon_cpu_seconds': final['daemon_cpu_seconds'],
                          'daemon_idle_cpu_seconds': initial['daemon_cpu_seconds'] - idle_started['daemon_cpu_seconds'],
                          'daemon_idle_rss_bytes': initial['daemon_rss_bytes'],
                          'daemon_retained_rss_bytes': final['daemon_rss_bytes'],
                          'daemon_peak_rss_bytes': sampler.daemon_peak,
                          'daemon_rss_growth_bytes': final['daemon_rss_bytes'] - initial['daemon_rss_bytes']}
                if worker:
                    sample.update(browser_cpu_seconds=final['browser_cpu_seconds'],
                                  browser_idle_cpu_seconds=initial['browser_cpu_seconds'] - idle_started['browser_cpu_seconds'],
                                  browser_idle_rss_bytes=initial['browser_rss_bytes'],
                                  browser_retained_rss_bytes=final['browser_rss_bytes'],
                                  browser_peak_sampled_rss_bytes=sampler.browser_peak,
                                  browser_rss_growth_bytes=final['browser_rss_bytes'] - initial['browser_rss_bytes'])
                return sample
            finally:
                sampler.stop.set()
                if sampler.thread.is_alive():
                    sampler.close()
                if worker and worker.poll() is None:
                    try:
                        worker.stdin.write('QUIT\n')
                        worker.stdin.flush()
                        worker.wait(timeout=10)
                    except (OSError, subprocess.TimeoutExpired):
                        stop_process(worker)
                stop_process(daemon)


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--runs', type=int, default=3)
    parser.add_argument('--cycles', type=int, default=12)
    parser.add_argument('--history', type=Path, default=ROOT / 'performance-results')
    parser.add_argument('--skip-ui', action='store_true')
    args = parser.parse_args()
    if platform.system() != 'Linux' or not Path('/proc/self/status').exists():
        parser.error('This recorder requires Linux /proc; native Android and other desktop OS measurements are not implemented')
    if args.runs < 1 or args.cycles < 1:
        parser.error('--runs and --cycles must be positive')
    if args.binary is None:
        metadata = json.loads(command('cargo', 'metadata', '--format-version', '1', '--no-deps'))
        args.binary = Path(metadata['target_directory']) / 'debug/colecto-host'
    binary = args.binary.resolve(strict=True)
    browser_version = command('node', '-p', "require('./ui/node_modules/playwright/package.json').version") if not args.skip_ui else None
    cpu = next((line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines()
                if line.startswith('model name')), platform.machine())
    machine = hashlib.sha256((platform.node() + cpu + platform.release()).encode()).hexdigest()[:16]
    dirty = bool(command('git', 'status', '--porcelain'))
    result = {'schema_version': 1, 'timestamp': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
              'commit': command('git', 'rev-parse', 'HEAD'), 'dirty': dirty,
              'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'ui_sha256': hashlib.sha256(b''.join(path.name.encode() + path.read_bytes() for path in sorted((ROOT / 'ui/dist').rglob('*')) if path.is_file())).hexdigest() if not args.skip_ui else None,
              'machine': {'host': platform.node(), 'cpu': cpu, 'kernel': platform.release(), 'architecture': platform.machine()},
              'compatibility': {'machine': machine, 'workload': WORKLOAD, 'cycles': args.cycles,
                                'ui': not args.skip_ui, 'profile': binary.parent.name, 'binary': binary.name,
                                'browser_driver': browser_version, 'cpu_count': os.cpu_count(),
                                'worker_sha256': hashlib.sha256((ROOT / 'scripts/resource_workload.mjs').read_bytes()).hexdigest() if not args.skip_ui else None, 'node': command('node', '--version') if not args.skip_ui else None},
              'samples': []}
    for run in range(args.runs):
        print(f'Measuring run {run + 1}/{args.runs} ({args.cycles} cycles)', flush=True)
        result['samples'].append(run_once(binary, not args.skip_ui, args.cycles))
    result['summary'] = summarize(result['samples'])
    args.history.mkdir(parents=True, exist_ok=True)
    for path in sorted(args.history.glob('*.json'), reverse=True):
        try:
            previous = json.loads(path.read_text())
        except (OSError, ValueError) as error:
            print(f'Skipping unreadable history {path.name}: {error}')
            continue
        comparison = compare(result, previous)
        if comparison is not None:
            result['comparison'] = {'baseline': path.name, 'metrics': comparison}
            break
    name = time.strftime('%Y%m%dT%H%M%SZ', time.gmtime()) + '-' + result['commit'][:8] + '-' + str(time.time_ns())[-6:] + '.json'
    destination = args.history / name
    destination.write_text(json.dumps(result, indent=2) + '\n')
    for metric, values in result['summary'].items():
        scale = 1024 * 1024 if metric.endswith('_bytes') else 1
        unit = 'MiB' if metric.endswith('_bytes') else 's'
        print(f"{metric}: median {values['median']/scale:.3f} {unit}, mean {values['mean']/scale:.3f} {unit}")
    if 'comparison' in result:
        print('Compared with ' + result['comparison']['baseline'] + ' (report only; see JSON for changes)')
    else:
        print('No compatible previous run; recorded baseline (report only).')
    print('Saved ' + str(destination))


if __name__ == '__main__':
    main()
