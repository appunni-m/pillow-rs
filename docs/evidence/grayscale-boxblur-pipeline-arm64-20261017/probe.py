#!/usr/bin/env python3
import argparse
import hashlib
import importlib.metadata
import json
import platform
import os
import random
import statistics
import subprocess
import sys
import time
from pathlib import Path

SIZE=(1024,768)
IMAGES=32
SAMPLES=5
WINDOWS=SAMPLES+2
ROOT=Path('/Users/lazytrot/work/pillow-rs')

def child(subject, out_path, stats_path):
    import PIL
    from PIL import Image, ImageFilter, ImageOps
    core=None
    if subject != 'Pillow':
        import pillow_rs
        core=pillow_rs._core
        for backend in ('cpu','simd','gpu'):
            pillow_rs.disable_backend(backend)
        if subject in ('CPU','SIMD'):
            pillow_rs.enable_backend(subject.lower())
    sources=[
        random.Random(i*131+1).randbytes(SIZE[0]*SIZE[1]*3)
        for i in range(IMAGES)
    ]
    source_hash=hashlib.sha256()
    for pixels in sources: source_hash.update(pixels)
    def graph(window,index):
        image=Image.frombytes('RGB',SIZE,sources[index])
        result=ImageOps.grayscale(image)
        result=result.filter(ImageFilter.BoxBlur(1))
        if subject in ('CPU','SIMD'):
            result._rust_image.lock_active_backend()
        return result
    executor=None
    if subject.startswith('GPU'):
        from PIL import GpuBatchExecutor
        executor=GpuBatchExecutor(queue=subject=='GPU queued',max_jobs=16,max_in_flight=4)
    def run_window(window):
        if executor is None:
            return [graph(window,i).tobytes() for i in range(IMAGES)]
        values={}
        for result in executor.run((i,graph(window,i)) for i in range(IMAGES)):
            values[result.input_key]=result.image.tobytes()
        return [values[i] for i in range(IMAGES)]
    receipt=None
    if core is not None and subject in ('CPU','SIMD'):
        core.take_pipeline_telemetry()
        core.set_pipeline_telemetry(True)
        proof=graph(0,0).tobytes()
        receipt=core.take_pipeline_telemetry()
        core.set_pipeline_telemetry(False)
        if not proof or not receipt:
            raise RuntimeError(f'no materialized backend receipt: {receipt}')
        expected=subject.lower()
        if receipt['actual_backend']!=expected or receipt['requested_backend']!=expected or receipt['fallback_reason'] is not None:
            raise RuntimeError(f'backend proof failed: {receipt}')
    times=[]
    output_digests=[]
    with open(out_path,'wb') as capture:
        for window in range(WINDOWS):
            started=time.perf_counter_ns()
            output=run_window(window)
            elapsed=(time.perf_counter_ns()-started)/1e9
            if window>=2: times.append(elapsed)
            digests=[hashlib.sha256(data).hexdigest() for data in output]
            output_digests.append(digests)
            if window==2:
                for data in output:
                    capture.write(len(data).to_bytes(8,'little'))
                    capture.write(data)
    stats=executor.stats if executor is not None else None
    if stats:
        jobs=WINDOWS*IMAGES
        if stats['submitted_jobs']!=jobs or stats['live_jobs']!=0 or stats['gpu_bytes']!=0 or stats['host_bytes']!=0:
            raise RuntimeError(f'GPU batch did not drain: {stats}')
        if stats['dispatches']!=jobs*2:
            raise RuntimeError(f'expected two stage dispatches per graph: {stats}')
        if stats['upload_bytes']!=jobs*SIZE[0]*SIZE[1]*3:
            raise RuntimeError(f'unexpected GPU input transfers: {stats}')
        if stats['readback_bytes']!=jobs*SIZE[0]*SIZE[1]:
            raise RuntimeError(f'unexpected GPU terminal materialization: {stats}')
        if subject=='GPU queued' and (stats['largest_submission']<2 or stats['submissions']>=jobs):
            raise RuntimeError(f'queued work did not batch: {stats}')
        executor.close()
    result={
      'subject':subject,'pillow_module_version':PIL.__version__,'pillow_distribution_version':importlib.metadata.version('Pillow'),'PIL_module':PIL.__file__,'input_sha256':source_hash.hexdigest(),
      'extension':str(core.__file__) if core else None,
      'extension_sha256':hashlib.sha256(Path(core.__file__).read_bytes()).hexdigest() if core else None,
      'seconds_samples':times,'median_seconds':statistics.median(times),'output_digests':output_digests,
      'throughput_images_per_s':IMAGES/statistics.median(times),
      'backend_receipt':receipt,'batch_stats':stats,
      'measurement':'construct RGB Image + Grayscale + BoxBlur(1), execute, synchronize, tobytes; 2 warmup and 5 measured changing-input windows; 32 distinct images/window'
    }
    Path(stats_path).write_text(json.dumps(result,sort_keys=True))

def main():
    if len(sys.argv)>1 and sys.argv[1]=='--child':
        child(sys.argv[2],sys.argv[3],sys.argv[4]); return
    subjects=('Pillow','CPU','SIMD','GPU eager','GPU queued')
    tmp=Path('/tmp/grayscale-boxblur-baseline'); tmp.mkdir(exist_ok=True)
    results=[]
    for subject in subjects:
        name=subject.replace(' ','-')
        env=os.environ.copy(); env.pop('PYTHONPATH',None)
        if subject!='Pillow': env['PYTHONPATH']=str(ROOT/'pillow-rs-py/python')
        subprocess.run([sys.executable,__file__,'--child',subject,str(tmp/(name+'.bytes')),str(tmp/(name+'.json'))],check=True,env=env)
        results.append(json.loads((tmp/(name+'.json')).read_text()))
    reference=(tmp/'Pillow.bytes').read_bytes()
    if any(row['output_digests'] != results[0]['output_digests'] for row in results[1:]):
        raise AssertionError('output digests mismatch across subjects or input windows')
    for result in results:
        payload=(tmp/(result['subject'].replace(' ','-')+'.bytes')).read_bytes()
        if payload!=reference: raise AssertionError(f'exact output mismatch for {result["subject"]}')
        result['exact_pillow_bytes']=True
    revision=subprocess.run(['git','rev-parse','HEAD'],cwd=ROOT,check=True,capture_output=True,text=True).stdout.strip()
    dirty=bool(subprocess.run(['git','status','--porcelain'],cwd=ROOT,check=True,capture_output=True,text=True).stdout.strip())
    distributions={row['pillow_distribution_version'] for row in results}
    if len(distributions)!=1: raise RuntimeError(f'Pillow distribution mismatch: {distributions}')
    report={'schema':'local-composed-gpu-baseline@1','source_revision':revision,'source_dirty':dirty,'host':platform.node(),'platform':platform.platform(),'architecture':platform.machine(),'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'workload_id':'pipeline-chain.grayscale-boxblur.material-rgb-noise-1024x768-radius-1','size':SIZE,'images_per_window':IMAGES,'samples':SAMPLES,'subjects':results,'input_sha256':results[0]['input_sha256'],'capture_sha256':hashlib.sha256(reference).hexdigest()}
    print(json.dumps(report,indent=2,sort_keys=True))
    (tmp/'report.json').write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
if __name__=='__main__': main()
