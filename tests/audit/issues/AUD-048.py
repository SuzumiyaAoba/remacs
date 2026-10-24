import pathlib, subprocess, sys, tempfile
binary = str(pathlib.Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix='remacs-repro-') as tmp:
    work = pathlib.Path(tmp)
    (work / 'fixture.txt').write_text('fixture-text\n')
    (work / 'lib').mkdir()
    (work / 'lib' / 'audit-cli-library.el').write_text(
        "(defvar audit-cli-library-value 42)\n(provide 'audit-cli-library)\n")
    argv = ['-Q', '--batch', '--eval', '(defun audit-cli-consume () (setq command-line-args-left nil))', '-f', 'audit-cli-consume', '--eval', '(princ 123)']
    argv = [arg.replace('<audit-dir>', str(work)) for arg in argv]
    result = subprocess.run([binary, *argv], cwd=work,
                            capture_output=True, timeout=30)
    print('cli:funcall-consumed-arguments', result.returncode, repr(result.stdout.decode()))
