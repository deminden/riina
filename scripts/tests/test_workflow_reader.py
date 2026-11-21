import unittest

from workflow_reader import action_step, named_step, parse


class WorkflowReaderTests(unittest.TestCase):
    def test_nested_jobs_and_ordered_steps(self):
        source = """on:
  pull_request:
jobs:
  package:
    needs: [build, audit]
    steps:
      - uses: actions/download-artifact@pinned # audit marker
        with:
          name: candidate
      - name: Check candidate
        if: always() && steps.download.outcome == 'success'
        run: |
          # This is part of the shell script.
          echo "name: embedded"

          echo example
  publish:
    needs: [package]
    steps:
      - name: Publish
        run: publish
"""
        result = parse(source)
        self.assertIn("pull_request", result["on"])
        package = result["jobs"]["package"]
        self.assertEqual(package["needs"], ["build", "audit"])
        download = action_step(package, "actions/download-artifact", name="candidate")
        check = named_step(package, "Check candidate")
        self.assertEqual(package["steps"], [download, check])
        self.assertEqual(check["if"], "always() && steps.download.outcome == 'success'")
        self.assertEqual(check["run"], '# This is part of the shell script.\necho "name: embedded"\n\necho example\n')
        self.assertEqual(result["jobs"]["publish"]["needs"], ["package"])

    def test_duplicate_and_indented_values_fail_loudly(self):
        for source in (
            "jobs:\n  package:\n  package:\n",
            "name: CI\n  unexpected: value\n",
        ):
            with self.subTest(source=source), self.assertRaises(ValueError):
                parse(source)
