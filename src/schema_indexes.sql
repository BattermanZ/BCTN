CREATE INDEX IF NOT EXISTS idx_name_string_key_strict ON name_string(key_strict);
CREATE INDEX IF NOT EXISTS idx_name_string_key_folded ON name_string(key_folded);

CREATE INDEX IF NOT EXISTS idx_name_concept_member_name_string ON name_concept_member(name_string_id);

CREATE INDEX IF NOT EXISTS idx_name_observation_country_year ON name_observation(country, year);
CREATE INDEX IF NOT EXISTS idx_name_observation_name_string ON name_observation(name_string_id);
CREATE INDEX IF NOT EXISTS idx_name_observation_year ON name_observation(year);
CREATE INDEX IF NOT EXISTS idx_name_observation_name_raw ON name_observation(name_raw);

