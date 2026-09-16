#!/usr/bin/env python3
"""Execute public quickstarts against packages installed from their registries."""
from __future__ import annotations

import json
import os
import re
from pathlib import Path
import subprocess
import sys
import tempfile

from check_docs_examples import fenced_examples
from docs_release import check_documents, published

ROOT = Path(__file__).resolve().parent.parent


def run(argv: list[str], cwd: Path, env: dict | None = None) -> None:
    subprocess.run(argv, cwd=cwd, env=env, check=True)


def main() -> None:
    config = json.loads((ROOT / 'documentation.json').read_text())
    check_documents(ROOT, config)
    version = published(config)['version']
    name = config['project']
    with tempfile.TemporaryDirectory(prefix='docs-consumer-') as directory:
        work = Path(directory)
        rust = config['rust_example']
        code = fenced_examples(ROOT / rust['source'], 'rust')[rust.get('index', 0)]
        if rust.get('font_input'):
            font = (ROOT / rust['font_input']).resolve()
            code += f'\nfn main() {{ assert!(!render_a(include_bytes!({json.dumps(str(font))})).unwrap().is_empty()); }}\n'
        (work / 'src').mkdir()
        (work / 'src/main.rs').write_text(code)
        (work / 'Cargo.toml').write_text(
            '[package]\nname = "public-docs-consumer"\nversion = "0.0.0"\nedition = "2024"\n'
            f'[dependencies]\n{name} = "={version}"\n')
        toolchain = re.search(r'(?m)^channel\s*=\s*"([^"]+)"', (ROOT / 'rust-toolchain.toml').read_text())[1]
        env = dict(os.environ, RUSTC_WRAPPER='', RUSTUP_TOOLCHAIN=toolchain,
                   CARGO_TARGET_DIR=str(ROOT / 'target/docs-registry-cargo'))
        run(['cargo', 'generate-lockfile'], work, env)
        run(['cargo', 'run', '--locked'], work, env)
        if 'pypi' in config['registries']:
            run([sys.executable, '-m', 'venv', str(work / 'venv')], work)
            python = work / 'venv' / ('Scripts/python.exe' if os.name == 'nt' else 'bin/python')
            run([str(python), '-m', 'pip', 'install', '--only-binary=:all:', f'{name}=={version}'], work)
            for source in config['python_examples']:
                for code in fenced_examples(ROOT / source, 'python'):
                    run([str(python), '-c', code], work)
        if 'npm' in config['registries']:
            run(['npm', 'install', '--ignore-scripts', '--no-audit', '--no-fund', '--save-exact', f'{name}@{version}'], work)
            installed = json.loads((work / 'node_modules' / name / 'package.json').read_text())
            if installed['version'] != version:
                raise ValueError('npm installed a different package version')
            example = config['javascript_example']
            code = fenced_examples(ROOT / example['source'], example['language'])[example.get('index', 0)]
            (work / 'quickstart.mjs').write_text(code)
            argv = ['node', 'quickstart.mjs']
            if rust.get('font_input'):
                argv.append(str((ROOT / rust['font_input']).resolve()))
            run(argv, work)
    print(f'Published-package quickstarts passed: {name} {version} ({", ".join(config["registries"])})')


if __name__ == '__main__':
    main()
