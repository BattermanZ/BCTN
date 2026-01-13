CREATE INDEX IF NOT EXISTS idx_name_string_key_strict ON name_string(key_strict);
CREATE INDEX IF NOT EXISTS idx_name_string_key_folded ON name_string(key_folded);

CREATE INDEX IF NOT EXISTS idx_name_concept_member_name_string ON name_concept_member(name_string_id);

CREATE INDEX IF NOT EXISTS idx_name_observation_country_year ON name_observation(country, year);
CREATE INDEX IF NOT EXISTS idx_name_observation_name_string ON name_observation(name_string_id);
-- Speeds backend spellings preview queries (join on name_string_id with run_id/country filters).
CREATE INDEX IF NOT EXISTS idx_name_observation_name_string_run_country
  ON name_observation(name_string_id, run_id, country);
-- Speeds materialization queries filtering primarily by run_id/country/year.
CREATE INDEX IF NOT EXISTS idx_name_observation_run_country_year_name_string
  ON name_observation(run_id, country, year, name_string_id);
CREATE INDEX IF NOT EXISTS idx_name_observation_run_name_string
  ON name_observation(run_id, name_string_id);
CREATE INDEX IF NOT EXISTS idx_name_observation_year ON name_observation(year);
CREATE INDEX IF NOT EXISTS idx_name_observation_name_raw ON name_observation(name_raw);

CREATE INDEX IF NOT EXISTS idx_variant_group_member_concept_key ON variant_group_member(concept_key);

CREATE INDEX IF NOT EXISTS idx_concept_country_window_total_country ON concept_country_window_total(country, total);
CREATE INDEX IF NOT EXISTS idx_variant_group_country_window_total_country ON variant_group_country_window_total(country, total);

CREATE INDEX IF NOT EXISTS idx_wikidata_concept_match_qid ON wikidata_concept_match(qid);
CREATE INDEX IF NOT EXISTS idx_variant_edge_from ON name_variant_edge(from_concept_id);
CREATE INDEX IF NOT EXISTS idx_variant_edge_to ON name_variant_edge(to_concept_id);

CREATE INDEX IF NOT EXISTS idx_card_spelling_subset_top_lookup
  ON card_spelling_subset_top(card_kind, card_key, subset_key, rank);
CREATE INDEX IF NOT EXISTS idx_card_spelling_total_all_lookup
  ON card_spelling_total_all(card_kind, card_key, total DESC, name_raw);
CREATE INDEX IF NOT EXISTS idx_card_rank_last5_lookup
  ON card_rank_last5(card_kind, card_key, country, year);
