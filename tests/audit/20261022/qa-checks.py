import concurrent.futures,json,subprocess,os
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
GNU="/nix/store/xbihkz9wlccg6ibr96j2qkgindirjs3l-emacs-unstable-31.1/bin/emacs-31.1"
suites=["json","overlay","overlays","overlay_type","modhooks","completion","textprops","indent","indentcmds"]
def run(job):
 suite,side=job;binary=GNU if side=="gnu" else str(ROOT/"target/debug/remacs")
 cmd=[binary,"-Q","--batch","-l",str(ROOT/"tests/qa_framework.el"),"-l",str(ROOT/f"tests/qa_{suite}.el"),"--eval","(princ (qa--done))"]
 try:
  p=subprocess.run(cmd,cwd=ROOT,capture_output=True,timeout=60);out,err,rc=p.stdout,p.stderr,p.returncode
 except subprocess.TimeoutExpired as e:out,err,rc=e.stdout or b"",e.stderr or b"",None
 stem="qa-"+suite+"."+side;(D/(stem+".out")).write_bytes(out);(D/(stem+".err")).write_bytes(err)
 r=dict(suite=suite,side=side,returncode=rc,command=cmd,stdout=out.decode(errors="replace"),stderr_file=stem+".err")
 print(suite,side,rc,r["stdout"][-120:],flush=True);return r
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:results=list(pool.map(run,[(s,t) for s in suites for t in ["gnu","remacs"]]))
(D/"qa-results.json").write_text(json.dumps(results,ensure_ascii=False,indent=2)+"\n")
