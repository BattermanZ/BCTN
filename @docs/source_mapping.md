# Source Data Mapping Specification

This document specifies how raw data from external sources should be mapped into the `stage_items` and `stage_edges` tables. This ensures that the `merge_staged_data()` function can process them correctly.

---

## Staging Table Structure

The target tables for all incoming data are `stage_items` and `stage_edges`.

### `stage_items`

-   `provider`: A short identifier for the source (e.g., `wikidata`, `wikipedia`, `behindthename`).
-   `external_id`: The unique ID of the record *from the source* (e.g., a Wikidata QID, a Wikipedia page title).
-   `payload`: A `JSONB` object containing the core data to be processed. The structure of this payload is defined below.
-   `hash`: An MD5 or SHA1 hash of the raw record to detect changes.

### `stage_edges`

-   `provider`: The identifier for the source.
-   `from_external_id`: The `external_id` of the source name.
-   `to_external_id`: The `external_id` of the related name.
-   `relation`: The type of relationship (e.g., `other_gender`, `variant`).
-   `payload`: A `JSONB` object for any extra data about the relationship (often empty).
-   `hash`: A hash of the edge data to detect changes.

---

## 1. Wikidata

-   **Provider Name**: `wikidata`
-   **Source**: SPARQL query results against the Wikidata query service.

### `stage_items` Mapping

| `stage_items` Field | Source Data (from SPARQL result)                                                              |
| ------------------- | --------------------------------------------------------------------------------------------- |
| `provider`          | Hardcoded as `'wikidata'`                                                                    |
| `external_id`       | `?qid` (e.g., `Q18041302`)                                                                    |
| `payload.display`   | `?nameLabel`                                                                                  |
| `payload.lang`      | `?langLabel` (mapped to 2-letter code, e.g., `English` -> `en`)                               |
| `payload.gender`    | `?genderClassLabel` (mapped to standard value, e.g., `male given name` -> `masculine`)        |
| `payload.aliases`   | Aggregated from `skos:altLabel` queries (JSON array of strings)                               |
| `payload.meaning`   | `null`                                                                                        |

#### Example `stage_items` Row:

```sql
INSERT INTO stage_items (provider, external_id, payload, hash)
VALUES (
    'wikidata',
    'Q18041302',
    {
        "display": "Aurélien",
        "lang": "fr",
        "gender": "masculine",
        "aliases": ["Aurelien", "Aurelian"]
    }::jsonb,
    '...' -- data hash
);
```

### `stage_edges` Mapping

Used for `P1560` (other-gender version) relations.

| `stage_edges` Field | Source Data (from SPARQL result) |
| ------------------- | -------------------------------- |
| `provider`          | Hardcoded as `'wikidata'`       |
| `from_external_id`  | `?aQ`                            |
| `to_external_id`    | `?bQ`                            |
| `relation`          | Hardcoded as `'other_gender'`    |
| `payload`           | `{}`                             |

#### Example `stage_edges` Row:

```sql
INSERT INTO stage_edges (provider, from_external_id, to_external_id, relation, payload, hash)
VALUES (
    'wikidata',
    'Q577523', -- Aurélia
    'Q18041302', -- Aurélien
    'other_gender',
    '{}'::jsonb,
    '...' -- data hash
);
```

---

## 2. Wikipedia / Wiktionary

-   **Provider Name**: `wikipedia` or `wiktionary`
-   **Source**: MediaWiki API `list=categorymembers` results.

### `stage_items` Mapping

| `stage_items` Field | Source Data                                                                                   |
| ------------------- | --------------------------------------------------------------------------------------------- |
| `provider`          | Hardcoded as `'wikipedia'` or `'wiktionary'`                                                 |
| `external_id`       | Page title (e.g., `"José"`)                                                                  |
| `payload.display`   | Page title, cleaned of any parenthetical suffixes like ` (given name)`                         |
| `payload.lang`      | Inferred from the source domain (e.g., `en.wikipedia.org` -> `en`)                             |
| `payload.gender`    | Inferred from the source category (e.g., `Category:French_masculine_given_names` -> `masculine`) |
| `payload.aliases`   | `null` (unless fetched from page redirects, which is out of scope for initial pass)           |
| `payload.meaning`   | If a page extract is fetched, this will be a JSON object. See BTN for example.                |

#### Example `stage_items` Row:

```sql
INSERT INTO stage_items (provider, external_id, payload, hash)
VALUES (
    'wikipedia',
    'José',
    {
        "display": "José",
        "lang": "es",
        "gender": "masculine",
        "meaning": {
            "short": "From the main section of the Wikipedia article...",
            "licence": "CC-BY-SA-4.0",
            "url": "https://en.wikipedia.org/wiki/Jos%C3%A9",
            "attribution_html": "<a href='https://en.wikipedia.org/wiki/Jos%C3%A9'>Wikipedia</a>"
        }
    }::jsonb,
    'a9b8c7d6...' -- data hash
);
```

---

## 3. Behind the Name (BTN)

-   **Provider Name**: `behindthename`
-   **Source**: Behind the Name API (api.behindthename.com)

### `stage_items` Mapping

| `stage_items` Field | Source Data (from BTN API JSON response)                                                      |
| ------------------- | --------------------------------------------------------------------------------------------- |
| `provider`          | Hardcoded as `'behindthename'`                                                               |
| `external_id`       | The name that was queried, in lowercase.                                                      |
| `payload.display`   | The `name` field from the API response.                                                       |
| `payload.lang`      | From the `usages` array, `usage_lang` field (e.g., `English` -> `en`). One row per usage.     |
| `payload.gender`    | From the `usages` array, `usage_gender` field (e.g., `m` -> `masculine`).                     |
| `payload.aliases`   | `null`                                                                                        |
| `payload.meaning`   | A JSON object containing the main etymology/meaning text from the API.                        |

#### Example `stage_items` Row:

```sql
-- One row would be created for each usage (English, French, etc.)
INSERT INTO stage_items (provider, external_id, payload, hash)
VALUES (
    'behindthename',
    'john',
    {
        "display": "John",
        "lang": "en",
        "gender": "masculine",
        "meaning": {
            "short": "From Iohannes, the Latin form of the Greek name Ioannes, itself derived from the Hebrew name Yochanan meaning \"YAHWEH is gracious\"...",
            "licence": "Creative Commons Attribution-ShareAlike 4.0 International License",
            "url": "https://www.behindthename.com/name/john",
            "attribution_html": "<a href='https://www.behindthename.com/name/john'>Behind the Name</a>"
        }
    }::jsonb,
    'f1e2d3c4...' -- data hash
);
```

### `stage_edges` Mapping

Used for the `related` names array in the BTN API response.

| `stage_edges` Field | Source Data (from BTN API JSON response) |
| ------------------- | ---------------------------------------- |
| `provider`          | Hardcoded as `'behindthename'`          |
| `from_external_id`  | The original name that was queried.      |
| `to_external_id`    | A name from the `related` array.         |
| `relation`          | The type of relation (e.g., `variant`).  |
| `payload`           | `{}`                                     |

#### Example `stage_edges` Row:

```sql
INSERT INTO stage_edges (provider, from_external_id, to_external_id, relation, payload, hash)
VALUES (
    'behindthename',
    'john', -- from
    'jon',  -- to
    'variant',
    '{}'::jsonb,
    'b5a6b7c8...' -- data hash
);
```
