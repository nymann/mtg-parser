set shell := ["sh", "-cu"]

# Default WotC URL. Pinned per published version — update by passing url=...
# Find new versions at https://magic.wizards.com/en/rules
rules_url := "https://media.wizards.com/2026/downloads/MagicCompRules%2020260417.txt"
qmd_index_path := "target/qmd-index/index.sqlite"
qmd_config_home := "target/qmd-config"
qmd_env := "INDEX_PATH=" + qmd_index_path + " XDG_CONFIG_HOME=" + qmd_config_home
qmd := qmd_env + " qmd"

# One-time bootstrap for new contributors: download the comprehensive rules
# and split them into resources/rules/. Idempotent — re-running re-splits
# without re-downloading. Override the URL with
#   just rules url=https://.../MagicCompRules%20YYYYMMDD.txt
rules url=rules_url:
	mkdir -p resources
	if [ ! -s resources/comprehensive_rules.txt ]; then \
		echo "fetching {{url}}"; \
		curl --fail --silent --show-error --location "{{url}}" -o resources/comprehensive_rules.txt; \
	else \
		echo "resources/comprehensive_rules.txt already present (use 'just rules-refresh' to re-download)"; \
	fi
	cargo xtask rules-split

# Force re-download of the rules text, then re-split.
rules-refresh url=rules_url:
	rm -f resources/comprehensive_rules.txt
	just rules url="{{url}}"

# Index resources/rules/ with qmd BM25 search so the grammar-fix
# orchestrator can retrieve relevant rules sections into its prompts.
# Requires qmd on PATH:  npm install -g @tobilu/qmd
# Idempotent — re-running picks up new/changed rules files.
rules-index:
	@command -v qmd >/dev/null 2>&1 || { echo "qmd not installed. Install with: npm install -g @tobilu/qmd" >&2; exit 1; }
	@[ -d resources/rules ] || { echo "resources/rules/ missing. Run 'just rules' first." >&2; exit 1; }
	mkdir -p $(dirname {{qmd_index_path}})
	mkdir -p {{qmd_config_home}}
	if ! {{qmd}} collection show mtg-rules >/dev/null 2>&1; then \
		echo "adding qmd collection mtg-rules"; \
		{{qmd}} collection add resources/rules --name mtg-rules; \
		{{qmd}} context add qmd://mtg-rules "Magic: The Gathering Comprehensive Rules, split per-section and per-glossary-entry. Canonical source for keyword definitions, damage/replacement/prevention shapes, and game vocabulary. The mtg-parser grammar should mirror this wording."; \
		{{qmd}} collection exclude mtg-rules; \
	fi
	{{qmd}} update

# Optional vector embeddings for qmd query/vsearch. The grammar-fix and
# refactor-hotspot flows use BM25 search, so this is not required for them.
rules-embed:
	@command -v qmd >/dev/null 2>&1 || { echo "qmd not installed. Install with: npm install -g @tobilu/qmd" >&2; exit 1; }
	mkdir -p $(dirname {{qmd_index_path}})
	mkdir -p {{qmd_config_home}}
	{{qmd}} embed

# Drop the qmd collection and rebuild from scratch. Use when the split
# structure changed in a way `qmd update` can't reconcile (e.g. renames).
rules-index-refresh:
	@command -v qmd >/dev/null 2>&1 || { echo "qmd not installed. Install with: npm install -g @tobilu/qmd" >&2; exit 1; }
	mkdir -p $(dirname {{qmd_index_path}})
	mkdir -p {{qmd_config_home}}
	{{qmd}} collection remove mtg-rules 2>/dev/null || true
	just rules-index

# Generate the local churn-vs-LOC audit report. Override refs with a
# comma-separated list of REF:Label entries if you want a different window.
audit-page out="audit-churn-complexity.html" refs="d6cb122:Baseline,59bef24:Semantic collapse,b23fa54:Damage refactor,c8e346e:Parse refactor,HEAD:Current":
	python3 scripts/generate_audit_page.py --out "{{out}}" --refs "{{refs}}"

corpus-summary:
	@jq '{total, passing, failing: (.total - .passing), grammar_left: ([.cards | to_entries[] | select(.value.status == "fail" and (.value.error | startswith("empty oracle text") | not))] | length), empty_oracle: ([.cards | to_entries[] | select(.value.status == "fail" and (.value.error | startswith("empty oracle text")))] | length)}' corpus_status.json

corpus-left:
	@jq -r '.cards | to_entries[] | select(.value.status == "fail" and (.value.error | startswith("empty oracle text") | not)) | .key' corpus_status.json

corpus-add-set code:
	cargo xtask corpus-add-set {{code}}

corpus-advance *args:
	cargo xtask corpus-advance {{args}}
