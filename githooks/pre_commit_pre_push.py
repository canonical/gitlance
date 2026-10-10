#!/usr/bin/env python3

# SPDX-FileCopyrightText: Canonical Ltd.
#
# SPDX-License-Identifier: Apache-2.0

import os
import subprocess
import sys


def main() -> int:
    local_branch = os.environ.get("PRE_COMMIT_LOCAL_BRANCH", "HEAD")
    if local_branch != "HEAD" and not local_branch.startswith("refs/heads/"):
        return 0

    head = os.environ.get("PRE_COMMIT_TO_REF") or local_branch
    return subprocess.call(
        ["gitlance", "--head", head, "--not-on-remotes"],
    )


if __name__ == "__main__":
    sys.exit(main())
