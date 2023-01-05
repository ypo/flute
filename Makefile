SHELL := /bin/bash

init_py:
	python3 -m venv venv
	source venv/bin/activate
	pip install maturin
	
test_py:
	maturin develop --all-features
	python3 -m unittest discover
