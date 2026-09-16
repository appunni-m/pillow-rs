# Public documentation interface. GNU Make 3.81+; no installs during help/build.
DOCS_VENV ?= .venv-docs
DOCS_PYTHON ?= $(DOCS_VENV)/bin/python
DOCS_HOST ?= 127.0.0.1
DOCS_PORT ?= 8000

DOCS_BENCHMARK_KIND ?= pillow
DOCS_BENCHMARK_SOURCE ?= build/migration-parity/benchmark-result.json
DOCS_BENCHMARK_OUTPUT ?= docs/evidence/benchmark.json
DOCS_REPOSITORY ?= appunni-m/pillow-rs

.PHONY: docs docs-setup docs-build docs-prepare docs-serve docs-test docs-lint docs-lock docs-benchmark docs-examples
docs: docs-build

docs-setup: ## Create an isolated environment and install the hashed documentation lock
	$(PYTHON) -m venv "$(DOCS_VENV)"
	$(DOCS_PYTHON) -m pip install --require-hashes --requirement requirements-docs.txt

docs-lock: ## Regenerate the documentation lock after an intentional dependency update
	uv pip compile requirements-docs.in --generate-hashes --output-file requirements-docs.txt

docs-prepare: ## Validate public sources and prepare the documentation site
	$(DOCS_PYTHON) scripts/docs_site.py prepare

docs-build: docs-prepare ## Build and validate the static site without publishing
	$(DOCS_PYTHON) -m mkdocs build --strict
	$(DOCS_PYTHON) scripts/docs_site.py check-html

docs-serve: docs-build ## Preview the built site locally; stop with Ctrl-C
	$(DOCS_PYTHON) -m http.server "$(DOCS_PORT)" --bind "$(DOCS_HOST)" --directory target/site

docs-lint: ## Check public document links, commands, attribution, and package identity
	$(PYTHON) scripts/docs_site.py check

docs-test: ## Exercise broken-link, output-path, and benchmark validation guards
	$(PYTHON) -m unittest discover -s scripts -p 'test_docs_site.py' -v

docs-examples: ## Compile and run the Rust quickstart directly from Markdown
	$(PYTHON) scripts/check_docs_examples.py

docs-benchmark: ## Export a public view of a recorded benchmark; never runs or changes measurements
	$(PYTHON) scripts/docs_evidence.py "$(DOCS_BENCHMARK_KIND)" "$(DOCS_BENCHMARK_SOURCE)" \
		"$(DOCS_BENCHMARK_OUTPUT)" --repository "$(DOCS_REPOSITORY)"
