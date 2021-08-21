#!/usr/bin/env python3

import subprocess

def run(*cmd):
    print(' '.join(cmd))
    subprocess.run(cmd, check=True)


def main():
    run('cargo', 'fmt', '--', '--check')
    run('cargo', 'clippy', '--', '-Dwarnings')
    run('cargo', 'test')


if __name__ == "__main__":
    main()
