# bctn_core/ingestion/wikidata.py

import os
import sys
import json
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
USER_AGENT = "BCTN-ETL/1.0 (https://github.com/your-repo/bctn; your-email@example.com)"

# --- SPARQL Queries from database_plan.md ---

# Query A: Strict EN/FR/ES given names with language tag and gender class
SPARQL_NAMES_STRICT = """
SELECT ?name ?nameLabel ?qid ?genderClassLabel ?langLabel WHERE {
  VALUES ?genderClass { wd:Q12308941 wd:Q11879590 wd:Q3409032 } # male/female/unisex
  VALUES ?lang { wd:Q1860 wd:Q150 wd:Q1321 }                    # EN/FR/ES
  ?name wdt:P31 ?genderClass .
  ?name wdt:P407 ?lang .
  BIND(STRAFTER(STR(?name), "entity/") AS ?qid)
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en,fr,es". }
}
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
    headers = {'User-Agent': USER_AGENT, 'Accept': 'application/sparql-results+json'}
    params = {'query': query, 'format': 'json'}
    try:
        response = requests.get(WIKIDATA_SPARQL_URL, headers=headers, params=params)
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        print(f"Error fetching data from Wikidata: {e}", file=sys.stderr)
        sys.exit(1)

# --- Transformation Functions ---

def transform_name_results(results: Dict[str, Any]) -> List[Tuple]:
    """Transforms raw SPARQL name results into the stage_items format."""
    staged_items = []
    bindings = results.get('results', {}).get('bindings', [])

    for item in bindings:
        try:
            payload = {
                "display": item['nameLabel']['value'],
                "lang": LANGUAGE_MAP.get(item['langLabel']['value'], 'unknown'),
                "gender": GENDER_MAP.get(item['genderClassLabel']['value'], 'unknown'),
                "aliases": [] # Alias query is separate, can be added later
            }
            external_id = item['qid']['value']
            # Hash the raw data to ensure idempotency
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
        print(f"No new data to insert into {table}.")
        return

    if table == 'stage_items':
        sql = "INSERT INTO stage_items (provider, external_id, payload, hash) VALUES %s ON CONFLICT (provider, hash) DO NOTHING"
    elif table == 'stage_edges':
        sql = "INSERT INTO stage_edges (provider, from_external_id, to_external_id, relation, payload, hash) VALUES %s ON CONFLICT (provider, hash) DO NOTHING"
    else:
        raise ValueError(f"Unknown staging table: {table}")

    with conn.cursor() as cursor:
        execute_values(cursor, sql, data)
        conn.commit()
        print(f"Successfully inserted/updated {cursor.rowcount} rows in {table}.")

# --- Main Execution ---

def main():
    """Main ETL process for Wikidata."""
    print("--- Starting Wikidata Ingestion ---")
    conn = get_db_connection()

    # 1. Fetch and process names
    print("Fetching names from Wikidata...")
    name_results = run_sparql_query(SPARQL_NAMES_STRICT)
    print(f"Found {len(name_results.get('results', {}).get('bindings', []))} potential name records.")
    staged_names = transform_name_results(name_results)
    insert_staged_data(conn, staged_names, 'stage_items')

    # 2. Fetch and process relations
    print("\nFetching relations from Wikidata...")
    relation_results = run_sparql_query(SPARQL_RELATIONS_GENDER)
    print(f"Found {len(relation_results.get('results', {}).get('bindings', []))} potential relation records.")
    staged_relations = transform_relation_results(relation_results)
    insert_staged_data(conn, staged_relations, 'stage_edges')

    conn.close()
    print("\n--- Wikidata Ingestion Complete ---")

if __name__ == "__main__":
    main()
