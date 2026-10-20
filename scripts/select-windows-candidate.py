"""Select an existing package only after its original build gates succeeded."""
import argparse
import json
import os
import re
import urllib.request


REQUIRED_STEPS = {
    "Packaging regressions for the exact selected source",
    "Verify downloaded native runtime hashes",
    "Native playback and lifecycle tests",
    "Lint and unit tests",
    "Build and assemble portable package",
}


def select(run, jobs, artifacts, repository, run_id):
    if (run.get("id") != run_id or run.get("status") != "completed"
            or run.get("event") != "workflow_dispatch"
            or run.get("path") != ".github/workflows/release.yml"
            or run.get("head_repository", {}).get("full_name") != repository
            or run.get("repository", {}).get("full_name") != repository
            or not re.fullmatch(r"[0-9a-f]{40}", run.get("head_sha", ""))):
        raise ValueError("Candidate must be a completed release dispatch from this repository")
    package_jobs = [job for job in jobs if job.get("name") == "windows-package"]
    if len(package_jobs) != 1:
        raise ValueError("Candidate package job is missing or ambiguous")
    steps = package_jobs[0].get("steps", [])
    installer_steps = {"Build installer from the same portable payload",
                       "Verify installed application, reinstall and uninstall"}
    required = REQUIRED_STEPS | (installer_steps if any(step.get("name") in installer_steps for step in steps) else set())
    for name in required:
        matching = [step for step in steps if step.get("name") == name]
        if len(matching) != 1 or matching[0].get("conclusion") != "success":
            raise ValueError(f"Original candidate did not pass: {name}")
    matching = [artifact for artifact in artifacts if artifact.get("name") == "windows-package"]
    if len(matching) != 1 or matching[0].get("expired") is not False:
        raise ValueError("Candidate package artifact is expired, missing or ambiguous")
    artifact = matching[0]
    if (type(artifact.get("id")) is not int or artifact["id"] <= 0
            or artifact.get("workflow_run", {}).get("id") != run_id
            or artifact.get("workflow_run", {}).get("head_sha") != run["head_sha"]):
        raise ValueError("Package artifact does not identify this exact candidate")
    return {"source_sha": run["head_sha"], "artifact_id": artifact["id"],
            "candidate_run_id": run_id}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--run-id", type=int, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", args.repository) or args.run_id <= 0:
        raise ValueError("Invalid candidate repository or run ID")
    base = f"https://api.github.com/repos/{args.repository}"
    headers = {"Authorization": "Bearer " + os.environ["GH_TOKEN"],
               "Accept": "application/vnd.github+json", "User-Agent": "riina-release-verification"}

    def get(path):
        with urllib.request.urlopen(urllib.request.Request(base + path, headers=headers), timeout=30) as response:
            return json.load(response)

    run = get(f"/actions/runs/{args.run_id}")
    jobs = get(f"/actions/runs/{args.run_id}/jobs?per_page=100")
    artifacts = get(f"/actions/runs/{args.run_id}/artifacts?per_page=100")
    if jobs["total_count"] > 100 or artifacts["total_count"] > 100:
        raise ValueError("Unexpected candidate metadata pagination")
    selected = select(run, jobs["jobs"], artifacts["artifacts"], args.repository, args.run_id)
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        for name, value in selected.items():
            output.write(f"{name}={value}\n")
    print(json.dumps(selected))


if __name__ == "__main__":
    main()
