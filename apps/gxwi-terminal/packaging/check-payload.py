import pathlib, sys, subprocess, tomllib
root = pathlib.Path(sys.argv[1])
for binary in ['peiterm']:
    elf = root / 'usr/bin' / binary
    assert elf.is_file()
    dynamic = subprocess.check_output(['readelf', '-dW', elf], text=True)
    assert 'BIND_NOW' in dynamic and 'RELR' in dynamic
    assert 'RPATH' not in dynamic and 'RUNPATH' not in dynamic and 'TEXTREL' not in dynamic
    notes = subprocess.check_output(['readelf', '-n', elf], text=True)
    assert 'IBT' in notes and 'SHSTK' in notes
    build_id = notes.split('Build ID: ')[1].split()[0]
    debug = root / 'debug' / 'peiterm' / 'usr/lib/debug/.build-id' / build_id[:2] / (build_id[2:] + '.debug')
    assert debug.is_file()
for app in (root / 'usr/share/apps').glob('*.toml'):
    tomllib.loads(app.read_text())
# The emulator is in the program, so its notices are in the package.
assert (root / 'licenses/xterm.js/LICENSE').is_file()
assert (root / 'usr/share/icons/base/dev.peios.peiterm.svg').is_file()
