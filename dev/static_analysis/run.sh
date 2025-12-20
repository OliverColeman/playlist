#!/bin/bash
set -euxo pipefail
uv sync
uvx ruff check .
pyright .
deptry .
