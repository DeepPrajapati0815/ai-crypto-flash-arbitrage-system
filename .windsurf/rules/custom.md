---
trigger: always_on
---

---
alwaysApply: true
---
Arbitrage Audit Guardrails Protocol

# Cursor Usage Rules for the Entire Arbitrage Project


Apply these rules **strictly** across the entire project lifecycle: from initial code generation to refactors, debugging, and deployments. Violations (e.g., suggesting dummy logic) must be flagged and corrected immediately. If Cursor cannot comply, halt and prompt for manual review.

## 🎯 **Core Principles (Overarching Rules)**
1. **Real Logic Only**: Never generate, suggest, or accept placeholders, pseudo-code, dummy data, mock calls, hardcoded simulations, fake branches, or debug hooks. All code must be **production-ready and executable** (e.g., use real API fetches, seeded RNG for determinism, live oracle integrations). Flag any such elements with: `// AUDIT VIOLATION: Dummy logic detected - replace with real [description]`.
   
2. **Audit-Inspired Workflow**: Before any code action, mentally simulate a **method-by-method logical dissection**: Trace inputs/outputs, branches, edge cases (e.g., high volatility, network lag), and data flows. Ensure >80% branch coverage in generated tests.

3. **MVP Prioritization**: Map actions to the **minimal viable path** (data → preprocessing → model inference → arb signal → on-chain exec → P&L validation). Defer non-essentials (e.g., advanced ML visualizations) if they bloat latency or dilute core flows. Justify deviations in comments.

4. **No Assumptions on Externals**: For third-party integrations (e.g., Chainlink, ONNX, 1inch APIs), reference **official docs only** (e.g., via Cursor's web search or pasted specs). Validate endpoints, auth, error handling (e.g., exponential backoff for rate limits), and data sync before integration.

5. **Determinism Enforcement**: All ML/trading logic must be seeded and reproducible (e.g., `torch.manual_seed(42)`). Flag non-determinism (e.g., unseeded randomness) as a critical violation.

6. **Impact Quantification**: For every suggestion or edit, include a comment estimating **runtime impact** (e.g., "This fix reduces slippage calc error by 2%, preserving ~$500/arb opportunity").

## 🧱 **Architecture & Flow Rules**
7. **Recursive Scanning Mandate**: When editing files, scan recursively for dependencies (e.g., via `cargo tree` or manual traces). Map and comment control/data/state flows in new code (e.g., ASCII diagrams for inter-component handoffs like ML output → Rust signal).

8. **Data Flow Verification**: Every data transformation (e.g., price feeds → tensor) must preserve integrity: Check types, precision (no NaN/Inf), and lineage. Simulate end-to-end in comments or tests before commit.

9. **Method-Level Integrity**: Dissect every function: Validate branches, conditions, and exits against real scenarios (e.g., zero-spread arb). Add inline assertions (e.g., `assert!(pnl > 0.0);`) for production safety.

## ⚙️ **Rust Backend Rules**
10. **Concurrency Safety**: Use async patterns (e.g., Tokio) only with deadlock-free guarantees. Validate thread safety via comments tracing mutex/atomics. Target < block-time latency (e.g., 12s for ETH).

11. **Calculation Rigor**: Trace financial formulas (e.g., slippage = `delta * (1 - reserve_ratio)`) against derivations. Use safe math (e.g., `checked_add`) and spot-check numerically in tests.

12. **Fix Format Compliance**: Suggestions must follow: Issue description + faulty line + production Rust snippet + reasoning + unit test.

## 🔒 **Solidity Contract Rules**
13. **Security First**: Enforce 0.8+ safe math, reentrancy guards (e.g., OpenZeppelin modifiers), and oracle desync checks. Benchmark gas (<200k per arb tx) in comments.

14. **On/Off-Chain Parity**: Mirror Rust calcs exactly (e.g., same AMM invariants). Validate with calldata examples in tests (e.g., Foundry simulations).

15. **No Mocks in Prod**: All branches executable on mainnet forks; remove simulation hooks.

## 🧠 **AI/ML Pipeline Rules**
16. **Data Pipeline Checks**: Normalize/scale identically in train/infer; trace lineage to prevent leaks. Handle nulls/imbalances with real stats (e.g., SMOTE for class balance).

17. **Model Efficiency & Accuracy**: Tune hypers for realism (e.g., XGBoost depth=6 for <1ms infer). Audit with metrics (AUC >0.85 for signals) and backtests on historical DEX data. Recommend quantization if latency > target.

18. **Inference Realism**: Match ONNX exports numerically (1e-6 tol); benchmark throughput (e.g., 1000 inferences/sec). Test adversarial robustness (e.g., ±5% price noise).

19. **No Dummy Models**: Replace fakes with real, seeded inference; integrate metrics logging (e.g., hit rate >70%).

## 💰 **Arbitrage & Integration Rules**
20. **Full Trace Mandate**: Every arb path: Model → signal → exec → P&L must be causal-traced. Validate spreads/slippage/fees against live APIs (no mocks).

21. **Third-Party Verification**: Simulate API calls per docs (e.g., Chainlink heartbeat >0). Add retries and freshness checks (e.g., round ID validation).

22. **Latency Gate**: Reject code if decision-to-exec > block time; optimize with profiling comments.

## 🔁 **Recursive & Enforcement Rules**
23. **Multi-Pass Simulation**: After edits, re-simulate under stress (e.g., volatility spikes). Ask: "Production-safe? Fully covered?"

24. **Testing & Telemetry**: Mandate >80% coverage; add Prometheus/Jaeger hooks for flows (e.g., trace arb latency). Check deployment configs (Docker/K8s) for scalability (e.g., 1000 TPS projections).

25. **Violation Logging**: Log all flags to a central file (e.g., `AUDIT_VIOLATIONS.md`) with root causes and fixes.

## 🧭 **Cursor-Specific Operational Rules**
26. **Prompt Prefix**: Start every Cursor session/prompt with: "Follow Audit Rules: Real logic only, trace flows, quantify impacts. MVP-first."

27. **Output Structure**: Responses must include: Flow diagram snippet, verified code/tests, impact analysis. No unverified suggestions.

28. **Halt on Gaps**: If docs/data missing, prompt: "Need [e.g., Chainlink spec] for verification—pause until provided."

29. **Scalability Projection**: For refactors, simulate load (e.g., "This scales to 500 arb/min under 10% volatility").

30. **Final Gate**: Before any commit/deploy, run a self-audit: "All rules compliant? Dummies eliminated? Flows intact?"

