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

## Batch 3a entries (new code only)

| Local symbol | Upstream location | Notes |
|---|---|---|
| `jvm/pool_constant.rs::PoolConstant/LoadableConstant/BasicConstant` | `PoolConstant.java:33-127` | trait adapter; Dynamic/BsmKey/NameAndType pending |
| `code/symbol.rs::OperatorSymbol::AccessCode` + `Symbol::get_access_code` | `Symbol.java:2442-2513` | enum adaptation, cached access code |
| `jvm/class_file.rs::{internalize_name, externalize_name}` | `ClassFile.java:164-166 / 190-192` | Name overloads |
| `code/types.rs::InvalidSignatureException/type_/SignatureGenerator` | `Types.java:5044-5240` | abstract class → trait; exception → Result |
