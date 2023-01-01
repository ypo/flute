develop_py:
	maturin develop --all-features

test_py: develop_py
	python -m unittest discover
