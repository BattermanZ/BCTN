# bctn_core/ingestion/wikidata.py

import os
import sys
import json
import string
import hashlib
import argparse
from typing import Any, Dict, List, Tuple

import requests
import psycopg2
from psycopg2.extras import execute_values
from dotenv import load_dotenv

# --- Constants ---
WIKIDATA_SPARQL_URL = "https://query.wikidata.org/sparql"
PROVIDER_NAME = "wikidata"
USER_AGENT_BROWSER = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36"
API_USER_AGENT_WIKIDATA = "query-service-ui (https://query.wikidata.org)"

# --- SPARQL Queries from database_plan.md ---

# Query A: Strict EN/FR/ES given names with language tag and gender class
# Escaped curly braces for Python .format() method
SPARQL_NAMES_STRICT = """
SELECT ?name ?nameLabel ?qid ?genderClassLabel ?langLabel WHERE {{
  VALUES ?genderClass {{ wd:Q12308941 wd:Q11879590 wd:Q3409032 }}
  VALUES ?lang {{ wd:Q1860 wd:Q150 wd:Q1321 }}
  ?name wdt:P31 ?genderClass .
  ?name wdt:P407 ?lang .
  BIND(STRAFTER(STR(?name), "entity/") AS ?qid)
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "en,fr,es". }}
  {partition_filter}
}}
ORDER BY ?nameLabel
"""

# Query C: Other-gender pairs (P1560)
SPARQL_RELATIONS_GENDER = """
SELECT ?aQ ?bQ WHERE {
  ?a wdt:P1560 ?b .
  BIND(STRAFTER(STR(?a), "entity/") AS ?aQ)
  BIND(STRAFTER(STR(?b), "entity/") AS ?bQ)
}
ORDER BY ?aQ
"""

# --- Mappings ---

GENDER_MAP = {
    "male given name": "masculine",
    "female given name": "feminine",
    "unisex given name": "unisex",
}

LANGUAGE_MAP = {
    "English": "en",
    "French": "fr",
    "Spanish": "es",
}

# --- Database & API Functions ---

def get_db_connection():
    """Establishes a database connection using credentials from .env file."""
    load_dotenv()
    try:
        conn = psycopg2.connect(
            dbname=os.environ["DB_NAME"],
            user=os.environ["DB_USER"],
            password=os.environ["DB_PASSWORD"],
            host=os.environ["DB_HOST"],
            port=os.environ["DB_PORT"],
        )
        return conn
    except (Exception, psycopg2.DatabaseError) as error:
        print(f"Error connecting to database: {error}", file=sys.stderr)
        sys.exit(1)

def run_sparql_query(query: str) -> Dict[str, Any]:
    """Executes a SPARQL query against the Wikidata endpoint."""
    headers = {
        'User-Agent': USER_AGENT_BROWSER,
        'Accept': 'application/sparql-results+json',
        'Referer': 'https://query.wikidata.org/',
        'X-Requested-With': 'XMLHttpRequest',
        'Api-User-Agent': API_USER_AGENT_WIKIDATA
    }
    params = {'query': query, 'format': 'json'}
    try:
        response = requests.get(WIKIDATA_SPARQL_URL, headers=headers, params=params, timeout=60)
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        print(f"Error fetching data from Wikidata: {e}", file=sys.stderr)
        # Don't exit on a single partition failure
        return {{}}

# --- Transformation Functions ---

def transform_name_results(results: Dict[str, Any]) -> List[Tuple]:
    """Transforms raw SPARQL name results into the stage_items format."""
    staged_items = []
    bindings = results.get('results', {}).get('bindings', [])

    for item in bindings:
        try:
            payload = {{
                "display": item['nameLabel']['value'],
                "lang": LANGUAGE_MAP.get(item['langLabel']['value'], 'unknown'),
                "gender": GENDER_MAP.get(item['genderClassLabel']['value'], 'unknown'),
                "aliases": [] # Alias query is separate, can be added later
            }}
            external_id = item['qid']['value']
            raw_record_str = json.dumps(item, sort_keys=True)
            record_hash = hashlib.md5(raw_record_str.encode()).hexdigest()

            staged_items.append((PROVIDER_NAME, external_id, json.dumps(payload), record_hash))
        except KeyError as e:
            print(f"Skipping record due to missing key: {e} in {item}", file=sys.stderr)

    return staged_items

def transform_relation_results(results: Dict[str, Any]) -> List[Tuple]:
    """Transforms raw SPARQL relation results into the stage_edges format."""
    staged_edges = []
    bindings = results.get('results', {}).get('bindings', [])

    for item in bindings:
        try:
            from_id = item['aQ']['value']
            to_id = item['bQ']['value']
            raw_record_str = json.dumps(item, sort_keys=True)
            record_hash = hashlib.md5(raw_record_str.encode()).hexdigest()

            staged_edges.append((PROVIDER_NAME, from_id, to_id, 'other_gender', '{}', record_hash))
        except KeyError as e:
            print(f"Skipping edge due to missing key: {e} in {item}", file=sys.stderr)

    return staged_edges

# --- Database Loading Functions ---

def insert_staged_data(conn, data: List[Tuple], table: str):
    """Inserts transformed data into a specified staging table."""
    if not data:
        # This is not an error, just no new data for this partition
        return 0

    if table == 'stage_items':
        sql = "INSERT INTO stage_items (provider, external_id, payload, hash) VALUES %s ON CONFLICT (provider, hash) DO NOTHING"
    elif table == 'stage_edges':
        sql = "INSERT INTO stage_edges (provider, from_external_id, to_external_id, relation, payload, hash) VALUES %s ON CONFLICT (provider, hash) DO NOTHING"
    else:
        raise ValueError(f"Unknown staging table: {table}")

    with conn.cursor() as cursor:
        execute_values(cursor, sql, data)
        conn.commit()
        return cursor.rowcount

# --- Main Execution ---

def main():
    """Main ETL process for Wikidata."""
    print("--- Starting Wikidata Ingestion ---")
    conn = get_db_connection()
    total_inserted_names = 0
    total_inserted_relations = 0

    # 1. Fetch and process names, partitioned by letter
    print("Fetching names from Wikidata (partitioned by letter to avoid timeouts)...")
    for letter in string.ascii_lowercase:
        print(f"  - Fetching names starting with '{letter.upper()}'...")
        partition_filter = f'FILTER(STRSTARTS(LCASE(?nameLabel), "{letter}"))'
        query = SPARQL_NAMES_STRICT.format(partition_filter=partition_filter)
        
        name_results = run_sparql_query(query)
        if not name_results:
            print(f"    -> Failed or no results for letter '{letter.upper()}'.")
            continue

        staged_names = transform_name_results(name_results)
        inserted_count = insert_staged_data(conn, staged_names, 'stage_items')
        total_inserted_names += inserted_count
        print(f"    -> Found {len(staged_names)} records, inserted {inserted_count} new.")

    print(f"\nTotal new names inserted: {total_inserted_names}")

    # 2. Fetch and process relations (this query is smaller and doesn't need partitioning)
    print("\nFetching relations from Wikidata...")
    relation_results = run_sparql_query(SPARQL_RELATIONS_GENDER)
    print(f"Found {len(relation_results.get('results', {}).get('bindings', []))} potential relation records.")
    staged_relations = transform_relation_results(relation_results)
    inserted_count = insert_staged_data(conn, staged_relations, 'stage_edges')
    total_inserted_relations += inserted_count
    print(f"Total new relations inserted: {total_inserted_relations}")

    conn.close()
    print("\n--- Wikidata Ingestion Complete ---")

if __name__ == "__main__":
    main()
