# Owner-derived Pro 20x weekly calibration

This handoff is sufficient to build the opt-in
[`quota-pro-20x.toml`](../../data/quota-pro-20x.toml) without private logs or a new
scan. It is a single-account estimator, not an official rate or independent
per-session validation. The owner reported 20x entitlement; logs did not
establish it independently.

## Frozen arithmetic

Weekly points per reference unit: **0.08236824622767631** =
15 / 182.10901272, approximately 12.1406 units per point.
The preset preserves all eight frozen input/cache/output triples from the study,
not current API prices. Reference units per million tokens:
`((I-C-W)*input + C*cached + W*write + O*output)/1e6`, multiplied once by the
window coefficient and speed/context/workload factors. Writes default to input;
reasoning is already output. The 6.1 Sol example I=1000000, C=800000, O=100000
yields 1.48 units and 0.12190500441696 weekly points.

Coverage starts October 16, 2026 as an estimation boundary, not proof of a plan
change. Calibration is through October 30 15:52:46.863 UTC; later events are
extrapolation. Standard-only speed and flat context 1× are pooled-fit assumptions.
Recorded guardian_review/exact codex-auto-review is zero under this policy,
not ordinary review prose. No explicit cache writes occurred in the modern
sample; treating writes as ordinary input is an assumption. No five-hour
coefficient, plan scaling, isolated model fit, effort premium or measured
context factor was established.

## Research method, collected 2026-10-30

Local active/archived JSONL and JSONL.zst: 10916 files, 10902 logical owners;
197013 distinct modern responses, September 3–October 30 15:52:46.863 UTC.
196397 (99.69%) paired to subsequent identical-category token_count/quota
observations (median lag 0.147s, p95 30.108s). Excluding 31755 guardian responses /
2043675938 raw tokens left 165258 ordinary responses / 23707882899 tokens.
No duplicate IDs, invalid token subsets or input/output total mismatches were
found. All 72838 ordinary modern responses from October 16 onward used the
same explicit account and OpenAI provider. Modern records covered every turn
with legacy deltas after September 17: **never add both token streams**.

Use the main codex weekly bucket (10080 minutes): 329669 deduplicated weekly
observations and 40040 historical five-hour observations. The last five-hour
observation was July 10, insufficient for a current preset. Cluster reset-anchor
jitter within ten seconds; choose the actual observation closest to the minute's
median used-percent, latest timestamp on ties. Build fixed UTC-hour intervals
from latest at/before-boundary observations, at most five minutes stale.
Exclude resets, saturation >=100, internal gaps >30 minutes, backward movement
>1 point, negative endpoint changes, absent tokens or nondefault/missing tier.
Include zero movement. Sum distinct responses in `(left,right]`. The 73 retained
nominal-hour samples begin October 16. Fit **sum(points)/sum(reference units)**,
not a mean of hourly ratios.

## Numerical validation

| Selected UTC intervals | Hours | Observed points | Reference units | Result |
| --- | ---: | ---: | ---: | --- |
| Oct 16–23, before cloud wave | 29 | 134 | 1723.82531186 | 0.07773409467773422 points/unit |
| Oct 23 21:29–Oct 27 16:36 cloud envelope | 22 | 167 | 901.86625060 | Earlier fit predicts 70.10575651; residual 96.89424349 |
| Oct 28–30, after wave | 22 | 64 | 777.16051792 | Earlier fit predicts 60.41186928; hourly MAE 0.36775502 |
| Oct 28 training | 5 | 15 | 182.10901272 | Latest coefficient 0.08236824622767631 |
| Oct 29–30 holdout | 17 | 49 | 595.05150520 | Latest fit predicts 49.01334890; hourly MAE 0.2561 |

Holdout p90 absolute error: 0.5427 points; maximum 0.6862; all 17 within one
point. Moving boundaries ±60/120 seconds gave MAE 0.26–0.30 and total
48.56–49.81 versus 49 observed. Splits were explored after seeing the data,
not preregistered; short validation is within one weekly period.
The provisional 0.078–0.082 range is not universal or a confidence interval.
Nonnegative model/category regressions did not consistently generalize better;
no independently fitted model coefficients are claimed.

## Cloud correction and attribution limits

Saved dispatch/progress evidence established 21 durable-host cloud root chats:
four batches of five on October 23–26 and an additional October 26 slot, with
work continuing into October 27. None was a local rollout owner; descendants
add unquantified consumption. The approximately 96.9-point overlap residual is
**not measured cloud usage**. Missing usage is strongly supported, but other
factors remain possible. Exclude these account-delta intervals from calibration,
not locally recorded events from estimation. Never allocate the residual to
local sessions.

Historical task listing/read access was unavailable; saved acknowledgements
established coverage. A narrow check of four later overseers found no explicit
cloud routes in 3185 tool calls, not proof of zero cloud activity elsewhere.
Hourly samples contained a median five roots (range 1–16); only four contained
one. Account-level agreement does not independently validate session allocation.
Count normalized linked descendants once. Missing cloud/device activity, shared
accounts, future models/plans and policy changes can invalidate the fit.
No absolute allowance denominator or cross-plan conversion was established.
Supported subtotals are modeled contributions, not statistical lower bounds.
Private rollouts, tool bodies and credentials are neither needed nor packaged.
