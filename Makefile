
test_py:
	maturin develop --all-features
	python3 -m unittest discover

publish_py:
	act -j linux --env ACTIONS_RUNTIME_TOKEN=foo --artifact-server-path ./artifact

readme:
	cargo readme > README.md

# Check the Reed-Solomon codec against zfec and OpenFEC (downloaded and built under target/rs-interop)
rs_interop:
	./interop/rs/run.sh
