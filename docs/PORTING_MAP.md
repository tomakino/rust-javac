# Porting map (upstream locations for local symbols)

This file is the single place where upstream `File.java:line` references are recorded.
Source code must not contain such references in new/rewritten code (PORTING_RULES 26.2).
Entries are added per batch, for newly written or rewritten symbols only.

| Local symbol | Upstream location | Notes |
|---|---|---|
| `jvm/pool_constant.rs::LoadableConstant` | `PoolConstant.java:33-127` | trait adapter; Dynamic/NameAndType pending |
| `code/symbol.rs::OperatorSymbol::AccessCode` | `Symbol.java:2462-2513` | enum adaptation |
| `code/types.rs::SignatureGenerator` | `Types.java:5044-5240` | abstract class → trait |

Status: template only; entries are appended by the executing agent and reviewed by the Lead.
