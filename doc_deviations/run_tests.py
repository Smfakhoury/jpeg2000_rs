#!/usr/bin/env python3
from pathlib import Path
from deepspec_core.rfcmap.replay import main

if __name__ == '__main__':
    raise SystemExit(main(package=Path(__file__).resolve().parent))
