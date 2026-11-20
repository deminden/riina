"""Exercise the source-only line width rule and the shrinking exception list."""

from collections import Counter
from contextlib import redirect_stderr
from importlib.util import module_from_spec, spec_from_file_location
from io import StringIO
from pathlib import Path
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "check-line-length.py"
SPEC = spec_from_file_location("check_line_length", SCRIPT)
CHECKER = module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


class RustLineLengthTests(unittest.TestCase):
    def test_cooked_and_raw_strings_do_not_hide_surrounding_code(self):
        wide = "x" * 200
        source = "\n".join([
            'let escaped = "' + wide + '\\"";',
            'let raw = br##"/* "' + wide,
            wide + '"##; ' + wide,
            'let plain = r"\\' + wide + '"; ' + wide,
        ])
        widths = [width for _, width in CHECKER.rust_line_widths(source)]
        self.assertEqual(
            [width > CHECKER.LIMIT for width in widths],
            [False, False, True, True],
        )

    def test_comments_count_and_character_literals_do_not_open_strings(self):
        wide = "x" * 200
        source = "\n".join([
            'let quote = \'"\'; // "' + wide,
            '/* nested /* " */ block ' + wide,
            wide + ' */ let ordinary = "' + wide + '";',
            'let short = "' + wide + '";',
        ])
        widths = [width for _, width in CHECKER.rust_line_widths(source)]
        self.assertEqual(
            [width > CHECKER.LIMIT for width in widths],
            [True, True, True, False],
        )

    def test_exception_counts_cannot_increase_and_stale_entries_are_rejected(self):
        original = Counter({"src/main.rs\tdigest": 1})
        violations = [("src/main.rs", 8, 121, "src/main.rs\tdigest")]
        with redirect_stderr(StringIO()) as errors:
            self.assertEqual(CHECKER.compare_and_check(original, original, violations), 0)
            self.assertEqual(CHECKER.compare_and_check(original, original, []), 1)
            self.assertEqual(
                CHECKER.compare_and_check(original + original, original, violations * 2), 1,
            )
        self.assertIn("Remove the unused exception", errors.getvalue())
        self.assertIn("New exception", errors.getvalue())


if __name__ == "__main__":
    unittest.main()
