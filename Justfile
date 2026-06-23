test:
	cargo test

bench:
	cargo bench --bench layout

fuzz bin:
	cargo fuzz run --sanitizer none {{bin}}
