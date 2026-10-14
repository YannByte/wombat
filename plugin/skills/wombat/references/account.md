# Account and allowance

Use `account --json`, or `account refresh --json` when fresh data is requested. Read account allowance without a usage scan. Do not ask for a project for this read.

Identity, allowance and activity have independent statuses and read times. Keep actual windows/buckets, model applicability, full-precision balances, limits and reset timestamps. Do not assume five-hour/seven-day windows, equate token use with subscription quota, or derive a remaining percentage from an unknown value.

Give the status of unknown, expired or failed reads. Passing a reset time does not prove restored allowance, and read-only entitlements do not offer a reset operation. Existing handoff eligibility checks determine whether a model is explicitly blocked. Low balance and unknown status are distinct.

Before another attempt, examine the section's `errorCode`. For `LOCAL_ACCESS_DENIED`, use the startup reference's narrow local-access recovery. After access is granted, do the same account read once more. Do not scan usage or repeatedly refresh under unchanged permissions. If access is not granted or the retry fails, show the current unknown status. Identify retained values as historical.
