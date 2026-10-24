import fcntl,json,os,pty,re,select,signal,struct,subprocess,termios,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent/"pty-editing-direct";D.mkdir(exist_ok=True)
GNU="/nix/store/xbihkz9wlccg6ibr96j2qkgindirjs3l-emacs-unstable-31.1/bin/emacs-31.1"
REM=json.load(open(D.parent/"current-binary.json"))["path"]
CASES=[
 ("unicode","",["A日😀B".encode(),b"\r"]),
 ("ctrl-edit","abc\n",[b"\x05\x02\x7f\x04"]),
 ("undo","",[b"abc",b"\x01\x05",b"X",b"\x1f"]),
 ("isearch","one two one\n",[b"\x13two\r",b"X"]),
 ("split","abc\n",[b"\x182",b"\x18o",b"\x05X",b"\x181"]),
 ("ctrl-home","abc\ndef\n",[b"\x1b>",b"\x1b[1;5H",b"X"]),
 ("kbd-macro","",[b"\x18(",b"x",b"\x18)",b"\x18e"]),
 ("prefix-repeat","",[b"\x153x"]),
]
ANSI=re.compile(rb"\x1b\[[0-?]*[ -/]*[@-~]")
results=[]
for name,initial,steps in CASES:
 for side,binary in [("gnu",GNU),("remacs",REM)]:
  file=D/(name+"."+side+".txt");file.write_text(initial)
  master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack("HHHH",24,80,0,0))
  def setup():
   os.setsid();fcntl.ioctl(0,termios.TIOCSCTTY,0)
  cmd=[binary,"-Q","-nw",str(file)]
  proc=subprocess.Popen(cmd,cwd=ROOT,stdin=slave,stdout=slave,stderr=slave,env=dict(os.environ,TERM="xterm-256color",LC_ALL="en_US.UTF-8"),preexec_fn=setup)
  os.close(slave);output=bytearray();started=time.monotonic();ready=False;forced=False
  def drain(seconds):
   end=time.monotonic()+seconds
   while time.monotonic()<end:
    if select.select([master],[],[],min(.05,max(0,end-time.monotonic())))[0]:
     try:chunk=os.read(master,65536)
     except OSError:return
     if not chunk:return
     output.extend(chunk)
    if proc.poll() is not None:return
  try:
   while time.monotonic()-started<30 and proc.poll() is None:
    drain(.1)
    if file.name.encode() in ANSI.sub(b"",output):ready=True;break
   if ready:
    for data in steps:
     os.write(master,data);drain(.3)
    os.write(master,b"\x18\x13");drain(1.0)
    os.write(master,b"\x18\x03");drain(6.0)
   (D/(name+"."+side+".terminal")).write_bytes(output)
   if proc.poll() is None:
    forced=True;proc.terminate()
   try:proc.wait(timeout=3)
   except subprocess.TimeoutExpired:proc.kill();proc.wait()
  finally:
   if proc.poll() is None:proc.kill();proc.wait()
   os.close(master)
  (D/(name+"."+side+".terminal")).write_bytes(output)
  record=dict(case=name,side=side,ready=ready,forced=forced,returncode=proc.returncode,seconds=round(time.monotonic()-started,2),saved_hex=file.read_bytes().hex(),saved_text=file.read_text(errors="replace"),input_hex=[s.hex() for s in steps],command=cmd,tail=ANSI.sub(b"",output)[-1500:].decode(errors="replace"))
  results.append(record);(D/"results.json").write_text(json.dumps(results,ensure_ascii=False,indent=2)+"\n")
  print(name,side,ready,forced,proc.returncode,repr(record["saved_text"]),flush=True)
