import concurrent.futures, json, os, subprocess, time, hashlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
D=Path(__file__).resolve().parent
FROZEN_INFO=json.loads((D/"current-binary.json").read_text())
FROZEN=FROZEN_INFO["path"]
GNU="/nix/store/xbihkz9wlccg6ibr96j2qkgindirjs3l-emacs-unstable-31.1/bin/emacs-31.1"
import sys
sys.path.insert(0,str(ROOT/"tools"))
from audit_runtime import rows
jobs=[]
for group in (sys.argv[1:] or ["overlay_edges","json_edges"]):
 out=subprocess.check_output([GNU,"-Q","--batch","-l",str(ROOT/f"tests/audit_{group}.el")],env=dict(os.environ,REMACS_AUDIT_FORMS="1"),text=True,errors="replace",timeout=60)
 for k,form in rows(out,"FORM").items():
  for side,bin in [("gnu",GNU),("remacs",FROZEN)]:
   jobs.append((group,k,form,side,bin))
def run(job):
 group,k,form,side,bin=job
 env=dict(os.environ,TZ="UTC",LC_ALL="C",REMACS_AUDIT_ONLY=k,REMACS_AUDIT_FORMS="1")
 cmd=[bin,"-Q","--batch","-l",str(ROOT/f"tests/audit_{group}.el")]
 started=time.monotonic()
 try:
  p=subprocess.run(cmd,env=env,cwd=ROOT,capture_output=True,timeout=90)
  out,err,rc=p.stdout,p.stderr,p.returncode
 except subprocess.TimeoutExpired as e:out,err,rc=e.stdout or b"",e.stderr or b"",None
 stem=k.replace(":","-")+".current."+side
 (D/(stem+".out")).write_bytes(out);(D/(stem+".err")).write_bytes(err)
 result=dict(id=k,group=group,form=form,side=side,command=cmd,returncode=rc,seconds=round(time.monotonic()-started,2),value=rows(out.decode(errors="replace"),"AUDIT").get(k),stdout=stem+".out",stderr=stem+".err",complete=b"AUDIT-DONE" in out)
 print(k,side,rc,result["value"],flush=True)
 return result
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:results=list(pool.map(run,jobs))
(D/("current-"+"-".join(sys.argv[1:])+".json" if sys.argv[1:] else "current.json")).write_text(json.dumps(dict(head=FROZEN_INFO["head"],worktree_head=subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),binary_sha256=hashlib.sha256(Path(FROZEN).read_bytes()).hexdigest(),runs=results),ensure_ascii=False,indent=2)+"\n")
