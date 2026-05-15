set shell := ["sh", "-cu"]

corpus-summary:
	@jq '{total, passing, failing: (.total - .passing), grammar_left: ([.cards | to_entries[] | select(.value.status == "fail" and (.value.error | startswith("empty oracle text") | not))] | length), empty_oracle: ([.cards | to_entries[] | select(.value.status == "fail" and (.value.error | startswith("empty oracle text")))] | length)}' corpus_status.json

corpus-left:
	@jq -r '.cards | to_entries[] | select(.value.status == "fail" and (.value.error | startswith("empty oracle text") | not)) | .key' corpus_status.json

corpus-add-set code:
	cargo xtask corpus-add-set {{code}}

corpus-advance *args:
	cargo xtask corpus-advance {{args}}
