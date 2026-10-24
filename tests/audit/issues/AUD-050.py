import pathlib, subprocess, sys, tempfile
binary = str(pathlib.Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix='remacs-repro-') as tmp:
    work = pathlib.Path(tmp)
    (work / 'fixture.txt').write_text('fixture-text\n')
    (work / 'lib').mkdir()
    (work / 'lib' / 'audit-cli-library.el').write_text(
        "(defvar audit-cli-library-value 42)\n(provide 'audit-cli-library)\n")
    argv = ['-Q', '--batch', '-L', '<audit-dir>/lib', '--eval', "(condition-case e (progn (require 'audit-cli-library) (princ audit-cli-library-value)) (error (prin1 (car e))))"]
    argv = [arg.replace('<audit-dir>', str(work)) for arg in argv]
    result = subprocess.run([binary, *argv], cwd=work,
                            capture_output=True, timeout=30)
    print('cli:load-path-short', result.returncode, repr(result.stdout.decode()))

    argv = ['-Q', '--batch', '--directory', '<audit-dir>/lib', '--eval', "(condition-case e (progn (require 'audit-cli-library) (princ audit-cli-library-value)) (error (prin1 (car e))))"]
    argv = [arg.replace('<audit-dir>', str(work)) for arg in argv]
    result = subprocess.run([binary, *argv], cwd=work,
                            capture_output=True, timeout=30)
    print('cli:load-path-long', result.returncode, repr(result.stdout.decode()))
