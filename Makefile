SHELL := /bin/bash

install_maturin:
	python -m venv .env
	source .env/bin/activate
	pip install maturin

develop_py:
	maturin develop --all-features

test_py: develop_py
	python -m unittest discover
