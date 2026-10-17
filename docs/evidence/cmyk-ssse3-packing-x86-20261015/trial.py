"""Small, explicit native CMYK experiment driver. No cleanup or broad campaign."""
from pathlib import Path
import hashlib
import importlib.metadata
import json
import os
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
VENV = Path('/tmp/pillow-rs-resize-venv-20261015')
RUST = Path('/home/appunni/.rustup/toolchains/1.96.1-x86_64-unknown-linux-gnu/bin')
MANIFEST = ROOT / 'pillow-rs/tests/fixtures/manifest.cmyk-packing-20261015.yaml'
WORKLOADS = ['pipeline-op.convert-mode-cmyk-to-rgb-1024x768', 'pipeline-chain.convert.material-cmyk-to-rgb-1024x768']
ENV = os.environ.copy()
ENV.update(PATH=f'{RUST}:{VENV}/bin:' + ENV['PATH'], RUSTC=str(RUST/'rustc'), RUSTUP_TOOLCHAIN='1.96.1-x86_64-unknown-linux-gnu', VIRTUAL_ENV=str(VENV), PYO3_PYTHON=str(VENV/'bin/python'), CARGO_TARGET_DIR='/tmp/pillow-rs-resize-target-20261015', MIGRATION_BENCHMARK_BACKENDS='simd', MIGRATION_TARGET_BACKEND='simd', MIGRATION_STRICT_TARGET_BACKEND='1', PYTHONDONTWRITEBYTECODE='1')
for key in ['MIGRATION_TARGET_PROFILE', 'MIGRATION_BENCHMARK_COHORT', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'PYTHONPATH']:
    ENV.pop(key, None)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_new(path, content):
    with path.open('x') as stream:
        stream.write(json.dumps(content, indent=2, sort_keys=True)+'\n')


def check_variant(variant):
    for rel, digest in json.loads((OUT/f'{variant}-source.json').read_text()).items():
        if sha(ROOT/rel) != digest:
            raise SystemExit('Unexpected source content: '+rel)


def run(label, command):
    print(json.dumps({'stage':label, 'command':command}), flush=True)
    with (OUT/f'{label}.log').open('x') as log:
        result=subprocess.run(command,cwd=ROOT,env=ENV,stdout=log,stderr=subprocess.STDOUT)
    print(json.dumps({'stage':label,'exit_code':result.returncode}),flush=True)
    if result.returncode:
        print((OUT/f'{label}.log').read_text()[-6000:],flush=True)
        raise SystemExit(result.returncode)


def snapshot(label, variant):
    check_variant(variant)
    inputs={}
    # Hash all Rust, binding Python, and producer sources, including untracked files.
    for base in ['pillow-rs/src','pillow-rs-py/src','pillow-rs-py/python','scripts']:
        for path in sorted((ROOT/base).rglob('*')):
            if path.is_file() and path.suffix in {'.rs','.py','.toml'}:
                inputs[str(path.relative_to(ROOT))]=sha(path)
    for rel in ['Cargo.toml','Cargo.lock','pillow-rs/Cargo.toml','pillow-rs-py/Cargo.toml','Makefile']:
        inputs[rel]=sha(ROOT/rel)
    for path in [MANIFEST,ROOT/'pillow-rs/tests/fixtures/inputs/parity/cmyk-packing-20261015.json',ROOT/'pillow-rs/tests/fixtures/inputs/benchmark/cmyk-packing-20261015.json',OUT/'capture_benchmark.py']:
        inputs[str(path.relative_to(ROOT))]=sha(path)
    oracle=importlib.metadata.distribution('Pillow')
    oracle_files={str(path):sha(Path(oracle.locate_file(path))) for path in oracle.files if str(path).startswith('PIL/') and Path(oracle.locate_file(path)).is_file() and Path(path).suffix in {'.py','.so'}}
    extensions={str(p.relative_to(ROOT)):sha(p) for p in (ROOT/'pillow-rs-py/python/pillow_rs').glob('*.so')}
    if not extensions: raise SystemExit('Missing built extension')
    obj={'variant':variant,'source_files':inputs,'source_digest':hashlib.sha256(json.dumps(inputs,sort_keys=True).encode()).hexdigest(),'extensions':extensions,'oracle_version':oracle.version,'oracle_digest':hashlib.sha256(json.dumps(oracle_files,sort_keys=True).encode()).hexdigest(),'python':sys.version,'rustc':subprocess.check_output([str(RUST/'rustc'),'-Vv'],text=True),'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'git_status':subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True),'environment':{k:ENV[k] for k in ['CARGO_TARGET_DIR','MIGRATION_BENCHMARK_BACKENDS','MIGRATION_TARGET_BACKEND','RUSTUP_TOOLCHAIN']},'host':subprocess.check_output(['uname','-a'],text=True).strip()}
    write_new(OUT/f'{label}.identity.json',obj)
    print(json.dumps({'snapshot':label,'source_digest':obj['source_digest'],'extensions':extensions}),flush=True)
    return obj


def main():
    action,label,variant=sys.argv[1:4]
    if action=='build':
        check_variant(variant)
        run(label,['make','build-parity',f'PYTHON={VENV}/bin/python'])
        snapshot(label,variant)
    elif action=='switch':
        check_variant('control' if variant=='candidate' else 'candidate')
        command=['git','apply']+(['--reverse'] if variant=='control' else [])+[str(OUT/'runtime.patch')]
        subprocess.run(command[:2]+['--check']+command[2:],cwd=ROOT,check=True)
        subprocess.run(command,cwd=ROOT,check=True)
        check_variant(variant)
        print('source switched to '+variant,flush=True)
    elif action=='test':
        check_variant(variant)
        run(label,[str(RUST/'cargo'),'test','-p','pillow-rs','--lib','--release','--locked','cmyk_packing_tests','--','--nocapture'])
    elif action=='guard':
        snapshot(label,variant)
        run(label,[str(VENV/'bin/python'),str(ROOT/'scripts/run_migration_parity.py'),'--manifest',str(MANIFEST),'--case-id','PIL.Image.Image.convert.nuanced.mode-audit-CMYK-to-RGB-17x3','--output',str(OUT/f'{label}.json'),'--timeout','300'])
    elif action=='benchmark':
        identity=snapshot(label,variant)
        ENV['CMYK_RAW_SAMPLES']=str(OUT/f'{label}.raw.jsonl')
        command=[str(VENV/'bin/python'),str(OUT/'capture_benchmark.py'),'--manifest',str(MANIFEST),'--output',str(OUT/f'{label}.json'),'--parity-output',str(OUT/f'{label}.parity.json'),'--timeout','300']
        if len(sys.argv)>4:
            previous=sys.argv[4]
            old=json.loads((OUT/f'{previous}.identity.json').read_text())
            for key in ['source_digest','extensions','oracle_digest','oracle_version','python','rustc','head','environment','host']:
                if identity[key]!=old[key]: raise SystemExit('Cannot reuse parity; mismatched '+key)
            command+=['--reuse-parity',str(OUT/f'{previous}.parity.json')]
        for workload in WORKLOADS: command+=['--workload-id',workload]
        run(label,command)
    else: raise SystemExit('Unknown action')

if __name__=='__main__': main()
