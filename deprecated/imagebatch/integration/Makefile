# Archived recipes; not included by the active Makefile.
imagebatch-rank-filter-fault-contract: ## Run target-only ImageBatch RankFilter fallback fault contracts
	@set +e; \
	$(MATURIN) develop --skip-install --manifest-path $(PY_SRC)/Cargo.toml --locked --features migration-fault-injection; \
	build_status=$$?; \
	if [ $$build_status -eq 0 ]; then \
		RANKFILTER_INCLUDE_FAULT_CONTRACT=1 $(PYTHON) scripts/test_imagebatch_rank_filter_parity.py; \
		test_status=$$?; \
	else \
		test_status=0; \
	fi; \
	$(MAKE) build-parity; \
	restore_status=$$?; \
	if [ $$restore_status -ne 0 ]; then exit $$restore_status; fi; \
	if [ $$build_status -ne 0 ]; then exit $$build_status; fi; \
	exit $$test_status

imagebatch-paste-fault-contract: ## Run target-only ImageBatch Paste fallback fault contracts
	@set +e; \
	$(MATURIN) develop --skip-install --manifest-path $(PY_SRC)/Cargo.toml --locked --features migration-fault-injection; \
	build_status=$$?; \
	if [ $$build_status -eq 0 ]; then \
		PASTE_BATCH_INCLUDE_FAULT_CONTRACT=1 $(PYTHON) scripts/test_imagebatch_parity.py; \
		test_status=$$?; \
	else \
		test_status=0; \
	fi; \
	$(MAKE) build-parity; \
	restore_status=$$?; \
	if [ $$restore_status -ne 0 ]; then exit $$restore_status; fi; \
	if [ $$build_status -ne 0 ]; then exit $$build_status; fi; \
	exit $$test_status

imagebatch-composite-fault-contract: ## Run target-only ImageBatch Composite fallback fault contracts
	@set +e; \
	$(MATURIN) develop --skip-install --manifest-path $(PY_SRC)/Cargo.toml --locked --features migration-fault-injection; \
	build_status=$$?; \
	if [ $$build_status -eq 0 ]; then \
		COMPOSITE_BATCH_INCLUDE_FAULT_CONTRACT=1 $(PYTHON) scripts/test_imagebatch_parity.py; \
		test_status=$$?; \
	else \
		test_status=0; \
	fi; \
	$(MAKE) build-parity; \
	restore_status=$$?; \
	if [ $$restore_status -ne 0 ]; then exit $$restore_status; fi; \
	if [ $$build_status -ne 0 ]; then exit $$build_status; fi; \
	exit $$test_status

imagebatch-expand-fault-contract: ## Run target-only ImageBatch Expand fallback fault contracts
	@set +e; \
	$(MATURIN) develop --skip-install --manifest-path $(PY_SRC)/Cargo.toml --locked --features migration-fault-injection; \
	build_status=$$?; \
	if [ $$build_status -eq 0 ]; then \
		EXPAND_BATCH_INCLUDE_FAULT_CONTRACT=1 $(PYTHON) scripts/test_imagebatch_expand_parity.py; \
		test_status=$$?; \
	else \
		test_status=0; \
	fi; \
	$(MAKE) build-parity; \
	restore_status=$$?; \
	if [ $$restore_status -ne 0 ]; then exit $$restore_status; fi; \
	if [ $$build_status -ne 0 ]; then exit $$build_status; fi; \
	exit $$test_status

imagebatch-grayscale-fault-contract: ## Run target-only ImageBatch Grayscale fallback fault contracts
	@set +e; \
	$(MATURIN) develop --skip-install --manifest-path $(PY_SRC)/Cargo.toml --locked --features migration-fault-injection; \
	build_status=$$?; \
	if [ $$build_status -eq 0 ]; then \
		GRAYSCALE_BATCH_INCLUDE_FAULT_CONTRACT=1 $(PYTHON) scripts/test_imagebatch_parity.py; \
		test_status=$$?; \
	else \
		test_status=0; \
	fi; \
	$(MAKE) build-parity; \
	restore_status=$$?; \
	if [ $$restore_status -ne 0 ]; then exit $$restore_status; fi; \
	if [ $$build_status -ne 0 ]; then exit $$build_status; fi; \
	exit $$test_status
