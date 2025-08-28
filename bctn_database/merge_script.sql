-- bctn_database/merge_script.sql

-- This script defines the stored procedure to merge data from the staging tables
-- into the core production tables. It is designed to be idempotent.

-- First, we need a helper function to normalize NFC text, as this is not a built-in PG function.
-- This is a placeholder and might need to be implemented as a C extension or using plpythonu for production.
CREATE OR REPLACE FUNCTION normalize_nfc(text)
RETURNS text LANGUAGE sql IMMUTABLE PARALLEL SAFE AS
'SELECT normalize($1, NFC);';


CREATE OR REPLACE FUNCTION merge_staged_data() RETURNS void
LANGUAGE plpgsql AS $$
BEGIN

    -- Create a temporary table to hold normalized data from the staging area.
    -- This simplifies the logic and avoids repeated normalization.
    CREATE TEMP TABLE processed_stage (
        stage_id INT PRIMARY KEY,
        external_id TEXT,
        provider TEXT,
        display_name TEXT,
        ascii_name TEXT,
        slug TEXT,
        language CHAR(2),
        gender TEXT,
        aliases JSONB
    ) ON COMMIT DROP;

    INSERT INTO processed_stage
    SELECT
        s.id as stage_id,
        s.external_id,
        s.provider,
        -- Assumes payload structure: {"display": "...", "lang": "..", "gender": "...", "aliases": [...]}
        normalize_nfc(s.payload ->> 'display') as display_name,
        unaccent(lower(normalize_nfc(s.payload ->> 'display'))) as ascii_name,
        regexp_replace(unaccent(lower(normalize_nfc(s.payload ->> 'display'))), '[^a-z0-9]+', '-', 'g') as slug,
        s.payload ->> 'lang' as language,
        s.payload ->> 'gender' as gender,
        s.payload -> 'aliases' as aliases
    FROM stage_items s;

    -- Create a temporary mapping of staged slugs to existing name IDs.
    -- This implements the deduplication heuristics from Section 3.
    CREATE TEMP TABLE stage_to_name_map (
        slug TEXT PRIMARY KEY,
        name_id INT NOT NULL
    ) ON COMMIT DROP;

    INSERT INTO stage_to_name_map (slug, name_id)
    WITH ranked_matches AS (
        SELECT
            ps.slug,
            n.id as name_id,
            -- Rank matches: 1 is best (exact), 2 is fuzzy.
            CASE
                WHEN n.display = ps.display_name THEN 1
                ELSE 2
            END as rank
        FROM processed_stage ps
        JOIN names n ON ps.ascii_name = n.ascii -- Broad join on ascii name
        JOIN name_usages nu ON n.id = nu.name_id AND ps.language = nu.language -- Must match language
        WHERE
            -- Rule 1: Exact display name match
            n.display = ps.display_name
            OR
            -- Rule 2: Fuzzy match with compatible genders
            (NOT ({n.gender_class, ps.gender} = '{"masculine", "feminine"}'))
    )
    -- For each slug, pick the best available match (rank 1 over 2)
    SELECT DISTINCT ON (slug)
        slug, name_id
    FROM ranked_matches
    ORDER BY slug, rank ASC;


    -- Insert new names for staged items that had no match.
    INSERT INTO names (display, ascii, slug, gender_class, primary_language)
    SELECT
        ps.display_name,
        ps.ascii_name,
        ps.slug,
        ps.gender,
        ps.language
    FROM processed_stage ps
    LEFT JOIN stage_to_name_map m ON ps.slug = m.slug
    WHERE m.name_id IS NULL
    ON CONFLICT (slug) DO NOTHING;

    -- Now that all names exist, create the final mapping from every staged item to its name_id.
    CREATE TEMP TABLE final_map (
        stage_id INT PRIMARY KEY,
        name_id INT NOT NULL
    ) ON COMMIT DROP;

    INSERT INTO final_map(stage_id, name_id)
    SELECT
        ps.stage_id,
        COALESCE(m.name_id, n.id) as name_id
    FROM processed_stage ps
    LEFT JOIN stage_to_name_map m ON ps.slug = m.slug
    LEFT JOIN names n ON ps.slug = n.slug;

    -- Upsert name usages.
    INSERT INTO name_usages (name_id, language, kind, source)
    SELECT
        fm.name_id,
        ps.language,
        ps.gender,
        ps.provider
    FROM final_map fm
    JOIN processed_stage ps ON fm.stage_id = ps.stage_id
    ON CONFLICT (name_id, language, kind, source) DO NOTHING;

    -- Upsert name aliases.
    INSERT INTO name_aliases (name_id, alias, language, source)
    SELECT
        fm.name_id,
        alias_text,
        ps.language,
        ps.provider
    FROM final_map fm
    JOIN processed_stage ps ON fm.stage_id = ps.stage_id,
         jsonb_array_elements_text(ps.aliases) as alias_text
    WHERE ps.aliases IS NOT NULL
    ON CONFLICT (name_id, alias, language, source) DO NOTHING;

    -- Upsert name relations from stage_edges.
    INSERT INTO name_relations (a_name_id, b_name_id, relation, source)
    SELECT
        (SELECT n.id FROM names n JOIN stage_items si ON n.slug = regexp_replace(unaccent(lower(normalize_nfc(si.payload ->> 'display'))), '[^a-z0-9]+', '-', 'g') WHERE si.external_id = se.from_external_id AND si.provider = se.provider LIMIT 1),
        (SELECT n.id FROM names n JOIN stage_items si ON n.slug = regexp_replace(unaccent(lower(normalize_nfc(si.payload ->> 'display'))), '[^a-z0-9]+', '-', 'g') WHERE si.external_id = se.to_external_id AND si.provider = se.provider LIMIT 1),
        se.relation,
        se.provider
    FROM stage_edges se
    ON CONFLICT (a_name_id, b_name_id, relation) DO NOTHING;

END;
$$;

-- Add a unique constraint to name_aliases to make the upsert logic robust.
ALTER TABLE name_aliases ADD CONSTRAINT uq_name_aliases UNIQUE (name_id, alias, language, source);
