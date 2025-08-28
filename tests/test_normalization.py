# tests/test_normalization.py

import unittest
from bctn_core.normalization import (
    normalize_nfc,
    fold_to_ascii,
    generate_slug,
    are_genders_compatible,
    is_equivalent
)

class TestNormalization(unittest.TestCase):

    def test_normalize_nfc(self):
        # A string with a decomposed character (e + ´)
        decomposed = "école"
        composed = "école"
        self.assertEqual(normalize_nfc(decomposed), composed)
        self.assertEqual(normalize_nfc(composed), composed)

    def test_fold_to_ascii(self):
        self.assertEqual(fold_to_ascii("José"), "jose")
        self.assertEqual(fold_to_ascii("Aurélien"), "aurelien")
        self.assertEqual(fold_to_ascii("François"), "francois")
        self.assertEqual(fold_to_ascii("JOHN"), "john")

    def test_generate_slug(self):
        self.assertEqual(generate_slug("jean-luc"), "jean-luc")
        self.assertEqual(generate_slug("mary anne"), "mary-anne")
        self.assertEqual(generate_slug("a name with spaces"), "a-name-with-spaces")
        self.assertEqual(generate_slug("o'malley"), "omalley")

    def test_are_genders_compatible(self):
        self.assertTrue(are_genders_compatible('masculine', 'masculine'))
        self.assertTrue(are_genders_compatible('feminine', 'feminine'))
        self.assertFalse(are_genders_compatible('masculine', 'feminine'))
        self.assertFalse(are_genders_compatible('feminine', 'masculine'))

        # Unisex and Unknown are compatible with anything
        self.assertTrue(are_genders_compatible('unisex', 'masculine'))
        self.assertTrue(are_genders_compatible('unisex', 'feminine'))
        self.assertTrue(are_genders_compatible('masculine', 'unisex'))
        self.assertTrue(are_genders_compatible('unknown', 'masculine'))
        self.assertTrue(are_genders_compatible('feminine', 'unknown'))

    def test_is_equivalent(self):
        # Rule 1: Exact match (should ignore gender)
        self.assertTrue(is_equivalent("Andrea", "it", "masculine", "Andrea", "it", "feminine"))
        self.assertTrue(is_equivalent("John", "en", "masculine", "John", "en", "masculine"))
        self.assertFalse(is_equivalent("John", "en", "masculine", "John", "fr", "masculine")) # Diff lang

        # Rule 2: Fuzzy match (case/diacritic insensitive)
        self.assertTrue(is_equivalent("Jose", "es", "masculine", "José", "es", "masculine"))
        self.assertTrue(is_equivalent("AURELIEN", "fr", "masculine", "Aurélien", "fr", "unisex")) # Compatible gender

        # Rule 2: Incompatible gender check
        # Names are not identical, but fold to the same ASCII. Should fail on gender.
        self.assertFalse(is_equivalent("ANDRÉA", "it", "masculine", "andréa", "it", "feminine"))

        # Not a match
        self.assertFalse(is_equivalent("Peter", "en", "masculine", "Pierre", "fr", "masculine"))

if __name__ == '__main__':
    unittest.main()