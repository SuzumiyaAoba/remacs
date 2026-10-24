import concurrent.futures, json, os, subprocess, time, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT/"tools"));from audit_runtime import rows
fixed=json.load(open(D/"fixed-binary.json"));runs=[]
for p in D.glob("isolated*.json"):runs+=json.load(open(p))["runs"]
cases={r["id"]:r for r in runs if r["side"]=="remacs"}
findings=json.load(open(D/"findings.json"));(D/"final-repro").mkdir(exist_ok=True)
def run(f):
 cid=f["cases"][0];ref=cases[cid]
 code=';;; -*- lexical-binding: t; -*-\n(load '+json.dumps(str(ROOT/"tests/audit_helpers.el"))+ ' nil t)\n(audit-check '+json.dumps(cid)+' \''+ref["form"]+')\n(princ "AUDIT-DONE\\n")\n'
 path=D/"final-repro"/(f["id"]+".el");path.write_text(code)
 cmd=[fixed["path"],"-Q","--batch","-l",str(path)]
 started=time.monotonic()
 try:
  p=subprocess.run(cmd,cwd=ROOT,capture_output=True,timeout=30);out,err,rc=p.stdout,p.stderr,p.returncode
 except subprocess.TimeoutExpired as e:out,err,rc=e.stdout or b"",e.stderr or b"",None
 stem=D/"final-repro"/f["id"];stem.with_suffix(".out").write_bytes(out);stem.with_suffix(".err").write_bytes(err)
 value=rows(out.decode(errors="replace"),"AUDIT").get(cid)
 result=dict(id=f["id"],case=cid,returncode=rc,value=value,matches_prior=rc==ref["returncode"] and value==ref["value"],seconds=round(time.monotonic()-started,2))
 print(result,flush=True);return result
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:results=list(pool.map(run,findings))
(D/"final-verification.json").write_text(json.dumps(dict(binary=fixed,runs=results),ensure_ascii=False,indent=2)+"\n")
