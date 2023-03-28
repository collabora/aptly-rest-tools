#!/usr/bin/env python3

from debian.deb822 import Changes, Deb822

from pathlib import Path

import functools
import hashlib
import io
import json
import sys


def fnv1a_64(data: Path):
    return functools.reduce(
        lambda acc, c: (acc ^ c) * 1099511628211 % 2 ** 64,
        data,
        14695981039346656037,
    )


def generate_package(changes_file, entry):
    name = entry['name']
    size = entry['size']
    md5 = entry['md5sum']
    sha1 = next(e['sha1'] for e in changes['checksums-sha1'] if e['name'] == name)
    sha256 = next(e['sha256'] for e in changes['checksums-sha256'] if e['name'] == name)

    # XXX
    sha512 = hashlib.sha512(sha256.encode()).hexdigest()

    data = io.BytesIO()
    data.write(name.encode())
    data.write(int(size).to_bytes(8))
    data.write(md5.encode())
    data.write(sha1.encode())
    data.write(sha256.encode())

    files_hash = hex(fnv1a_64(data.getvalue())).removeprefix('0x')

    with open(changes_file.parent / (name.removesuffix('.deb') + '.control')) as fp:
        control = Deb822(fp)

    short_key = f'P{control["architecture"]} {control["package"]} {control["version"]}'
    key = f'{short_key} {files_hash}'

    return {field: control.get_as_string(field) for field in control} | {
        'FilesHash': files_hash,
        'MD5sum': md5,
        'SHA1': sha1,
        'SHA256': sha256,
        'SHA512': sha512,
        'ShortKey': short_key,
        'Key': key,
    }


packages = []

for arg in sys.argv[1:]:
    changes_file = Path(arg)
    with open(changes_file) as fp:
        changes = Changes(fp)

    for entry in changes['files']:
        if entry['name'].endswith('.deb'):
            packages.append(generate_package(changes_file, entry))

json.dump(packages, sys.stdout, indent=2)
print()
