import fnmatch, pathlib, sys, subprocess, tomllib
root = pathlib.Path(sys.argv[1])

# Everything staged under usr/ is in a package: a [files] entry that maps
# none of it leaves it out of every .peipkg, and nothing else would say so.
mapped = [key[1:] for f in pathlib.Path('packages.pekit').glob('*.package.pekit.toml')
          for key in tomllib.loads(f.read_text()).get('files', {}) if key.startswith(':')]
def shipped(rel):
    return any(fnmatch.fnmatch(rel, pattern) or (pattern.endswith('/**') and rel.startswith(pattern[:-2])) or rel == pattern for pattern in mapped)
dropped = [str(p.relative_to(root)) for p in (root / 'usr').rglob('*') if p.is_file() and not shipped(str(p.relative_to(root)))]
assert not dropped, f'staged but in no package: {dropped[:10]}'
assert (root / 'usr/share/man/man1/peitor.1.gz').is_file()
for binary in ['peitor']:
    elf = root / 'usr/bin' / binary
    assert elf.is_file()
    dynamic = subprocess.check_output(['readelf', '-dW', elf], text=True)
    assert 'BIND_NOW' in dynamic and 'RELR' in dynamic
    assert 'RPATH' not in dynamic and 'RUNPATH' not in dynamic and 'TEXTREL' not in dynamic
    notes = subprocess.check_output(['readelf', '-n', elf], text=True)
    assert 'IBT' in notes and 'SHSTK' in notes
    build_id = notes.split('Build ID: ')[1].split()[0]
    debug = root / 'debug' / 'peitor' / 'usr/lib/debug/.build-id' / build_id[:2] / (build_id[2:] + '.debug')
    assert debug.is_file()
for app in (root / 'usr/share/apps').glob('*.toml'):
    declared = tomllib.loads(app.read_text())
    # What it opens is what makes it the editor a file explorer finds.
    assert 'text/*' in declared['opens']
assert (root / 'usr/share/icons/base/dev.peios.peitor.svg').is_file()
