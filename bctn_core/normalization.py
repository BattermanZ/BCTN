# bctn/normalization.py

import re
import unicodedata
from unidecode import unidecode

# A regex to find and remove any character that is not a letter, number, or hyphen.
PUNCTUATION_REGEX = re.compile(r"[^a-zA-Z0-9-]+")


def normalize_nfc(text: str) -> str:
    """Converts a string to Unicode Normalization Form C (NFC)."""
    if not text:
        return ""
    return unicodedata.normalize('NFC', text)


def fold_to_ascii(text: str) -> str:
    """Converts a string to its lowercase ASCII representation."""
    if not text:
        return ""
    return unidecode(text).lower()


def generate_slug(ascii_text: str) -> str:
    """
    Generates a URL-friendly slug from an ASCII string.
    Replaces spaces with hyphens and removes other punctuation.
    """
    if not ascii_text:
        return ""
    # Replace spaces with hyphens first
    slug = ascii_text.replace(' ', '-')
    # Remove all other invalid characters
    slug = PUNCTUATION_REGEX.sub('', slug)
    return slug


def are_genders_compatible(gender1: str, gender2: str) -> bool:
    """
    Checks if two gender classes are compatible for merging.
    They are considered compatible unless one is strictly 'masculine' and
    the other is strictly 'feminine'.
    """
    if {gender1, gender2} == {'masculine', 'feminine'}:
        return False
    return True


def is_equivalent(
    name1: str, lang1: str, gender1: str,
    name2: str, lang2: str, gender2: str
) -> bool:
    """
    Checks if two names are equivalent based on the deduplication heuristics
    from Section 3 of the database plan.

    Heuristics (in order):
    1. Exact `display` match and same `language`.
    2. Case/diacritic-insensitive `display` match, same `language`, and
       compatible `gender_class`.
    """
    # Rule 1: Exact match
    if name1 == name2 and lang1 == lang2:
        return True

    # Rule 2: Fuzzy match
    if fold_to_ascii(name1) == fold_to_ascii(name2) and \
       lang1 == lang2 and \
       are_genders_compatible(gender1, gender2):
        return True

    return False