import concurrent.futures, json, os, subprocess, time, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT/"tools"));from audit_runtime import rows
groups=sys.argv[1:] or ["overlay_edges","json_edges","insert_edges","completion_edges","edit_commands","change_hook_edges"]
def run(group):
 env=dict(os.environ,REMACS_NO_ELC="1",REMACS_AUDIT_SKIP="overlay:make-dead-buffer",LC_ALL="C",TZ="UTC")
 cmd=[str(ROOT/"target/debug/remacs"),"-Q","--batch","-l",str(ROOT/f"tests/audit_{group}.el")]
 start=time.monotonic()
 try:
  p=subprocess.run(cmd,env=env,cwd=ROOT,capture_output=True,timeout=90)
  out,err,rc=p.stdout,p.stderr,p.returncode
 except subprocess.TimeoutExpired as e:out,err,rc=e.stdout or b"",e.stderr or b"",None
 stem=group+".source";(D/(stem+".out")).write_bytes(out);(D/(stem+".err")).write_bytes(err)
 result=dict(group=group,returncode=rc,seconds=round(time.monotonic()-start,2),complete=b"AUDIT-DONE" in out,values=rows(out.decode(errors="replace"),"AUDIT"),command=cmd,environment={"REMACS_NO_ELC":"1","REMACS_AUDIT_SKIP":"overlay:make-dead-buffer"})
 print(group,rc,result["complete"],len(result["values"]),flush=True);return result
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:results=list(pool.map(run,groups))
(D/("source-mode-extra.json" if sys.argv[1:] else "source-mode.json")).write_text(json.dumps(results,ensure_ascii=False,indent=2)+"\n")
