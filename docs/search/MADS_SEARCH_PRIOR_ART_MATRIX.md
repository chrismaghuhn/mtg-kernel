# MADS V0 Prior-Art and Baseline Matrix (Preliminary)

**Status: literature verification incomplete. This matrix is a review checklist, not a novelty claim.**

The MADS V0.4 paper calls out these search families as required comparisons. No external literature audit or implementations were completed in MADS-01; titles, variants, and current state-of-practice must be checked against primary papers before drawing conclusions.

| Family | Relation to MADS hypothesis | Baseline status in this repository | Literature check |
|---|---|---|---|
| Minimax / alpha-beta with transposition table | Classical adversarial baseline; tests value of best-first scheduling and selective expansion. | NOT IMPLEMENTED in MADS-01. | NOT REVIEWED: modern move ordering, exact-key requirements, and benchmark protocols. |
| Best-first minimax | Closest broad family for selective root proof with interval information. | NOT IMPLEMENTED. | NOT REVIEWED: strongest variants and direct comparison criteria. |
| B* | Root choice under uncertainty/probability bounds; compare root-proof objective and work allocation. | NOT IMPLEMENTED. | NOT REVIEWED: original and later variants, assumptions, and reproducible baseline. |
| Probability-based B* | Potential bound-guided root selection; compare whether MADS's certified intervals add value. | NOT IMPLEMENTED. | NOT REVIEWED: probability calibration requirements and applicability to deterministic exact search. |
| Proof-Number Search (PNS) | Direct root-proof family; mandatory comparator for solved tactical/acyclic fixtures. | NOT IMPLEMENTED. | NOT REVIEWED: tree, transposition-aware and graph variants. |
| Depth-first PNS (df-pn) | Memory-conscious proof/disproof baseline on tactical search spaces. | NOT IMPLEMENTED. | NOT REVIEWED: threshold semantics and graph/transposition caveats. |
| Conspiracy-Number Search | Work allocation based on evidence needed to alter a root conclusion; close scheduling comparison. | NOT IMPLEMENTED. | NOT REVIEWED: relevant variants and common-value assumptions. |
| Proof-set / DAG proof methods | Relevant where shared descendants make naive tree proof-number sums incorrect. | NOT IMPLEMENTED. | NOT REVIEWED: DAG accounting methods and applicable correctness claims. |
| MCTS + transpositions | Empirical search baseline; useful on non-oracle game positions, but not a certified-bounds baseline. | Existing kernel-native and model-guided searchers exist; no MADS-side comparison run. | NOT REVIEWED: suitable deterministic-compute matching and transposition conventions. |
| Graph-History Interaction (GHI) handling | Required boundary for cyclic/path-dependent state reuse; MADS-01 avoids cycles. | NOT IMPLEMENTED; cycles return `CYCLE_DETECTED`. | NOT REVIEWED: published path-sensitive and repetition-aware methods. |

## Comparison discipline for a later milestone

- First compare OracleSuite-sized fixtures for correctness and deterministic transition/expansion counts.
- Do not report a MADS performance win from a toy fixture or a weak baseline.
- Match legal action order, terminal utilities, depth/budget semantics, and state identity before comparing work.
- Keep certified proof bounds separate from heuristic values and from PNS/CN scheduling estimates.
- Add classical alpha-beta, best-first, PNS-family and MCTS baselines only after their primary-literature contracts are reviewed and frozen.

No literature novelty assessment is complete as of MADS-01.
