# unreachable! audit (class C) — pending classification approval

Totals: 102 unreachable! sites; dead_code allow: 71.

## Categories (heuristic)

- other: 4
- match-wildcard (upstream has no wildcard; adapter): 62
- message-carrying (check upstream Assert.error / impl gap): 36

## Entries

| local | fn | category | code |
|---|---|---|---|
| crates/javac-core/src/code/class_finder.rs:546 | include_class_file | other | let Some(existing) = c else { unreachable!() }; |
| crates/javac-core/src/code/scope.rs:613 | enter | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/scope.rs:1000 | get_symbols_full | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/scope.rs:1258 | get_symbols | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/scope.rs:1523 | is_statically_imported_full | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:766 | is_same_type_visit_wildcard | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:770 | is_same_type_visit_wildcard | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:879 | is_same_type_visit_array | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:883 | is_same_type_visit_array | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:920 | is_same_type_visit_for_all | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:926 | is_same_type_visit_for_all | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:1152 | wild_upper_bound | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:1164 | wild_upper_bound | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:1183 | wild_lower_bound | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:1258 | supertype | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:1306 | supertype | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:1336 | interfaces | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:1433 | subst_visit | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:1458 | subst_visit | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:1481 | ? | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:1495 | ? | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:1514 | ? | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:2202 | capture | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:2214 | capture | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:2234 | ? | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:2357 | contained_by | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:2645 | ? | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:2650 | ? | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:2810 | is_reifiable | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:2922 | elemtype | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:2959 | has_same_args_strict | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:2971 | has_same_args_strict | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:3426 | is_castable_visit | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:3430 | is_castable_visit | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:4628 | adapt_visit | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:4634 | adapt_visit | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:5336 | rewrite_quantifiers | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:5342 | rewrite_quantifiers | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:5352 | rewrite_quantifiers | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/code/types.rs:8907 | ? | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/analyzer.rs:310 | analyzer_copier_fix | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/argument_attr.rs:1252 | ? | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/attr.rs:3638 | visit_class_def | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/attr.rs:3703 | visit_method_def | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/attr.rs:15204 | preset_type_idents | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/attr.rs:15219 | preset_type_idents | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/attr_recover.rs:184 | do_recovery | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/attr_recover.rs:189 | do_recovery | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/deferred_attr.rs:1425 | visit_method_type | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/deferred_attr.rs:1434 | visit_method_type | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/lower.rs:5447 | lower_assignop | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/resolve.rs:514 | is_protected_accessible | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/resolve.rs:4024 | raw_instantiate | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/resolve.rs:4071 | ? | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/comp/trans_types.rs:1015 | visit_method_def | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/file/locations.rs:560 | is_default_boot_class_path | message-carrying (check upstream Assert.error / impl gap) | _ => unreachable!("PLATFORM_CLASS_PATH handler"), |
| crates/javac-core/src/jvm/class_reader.rs:395 | get_module | message-carrying (check upstream Assert.error / impl gap) | Ok(_) => unreachable!("TagSet::Module accepted a non-module value"), |
| crates/javac-core/src/jvm/class_reader.rs:422 | get_package | message-carrying (check upstream Assert.error / impl gap) | Ok(_) => unreachable!("TagSet::Package accepted a non-package value"), |
| crates/javac-core/src/jvm/class_reader.rs:448 | get_class | message-carrying (check upstream Assert.error / impl gap) | Ok(_) => unreachable!("TagSet::Class accepted a non-class value"), |
| crates/javac-core/src/jvm/class_reader.rs:489 | get_name_and_type | message-carrying (check upstream Assert.error / impl gap) | Ok(_) => unreachable!("TagSet::NameAndType accepted a non-nameandtype value"), |
| crates/javac-core/src/jvm/codegen.rs:1220 | gen_cond | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/jvm/codegen.rs:1300 | ? | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/parser/javac_parser.rs:2017 | arguments_apply | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/parser/javac_parser.rs:2549 | term2_rest | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/parser/javac_parser.rs:2659 | fold_strings | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/parser/javac_parser.rs:2939 | ? | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/parser/javac_parser.rs:2959 | ? | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/parser/javac_parser.rs:4061 | variable_declarator_rest | match-wildcard (upstream has no wildcard; adapter) | _ => unreachable!(), |
| crates/javac-core/src/tree/doc_pretty.rs:208 | visit_attribute | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitAttribute on a non-Attribute node") |
| crates/javac-core/src/tree/doc_pretty.rs:234 | visit_author | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitAuthor on a non-Author node") |
| crates/javac-core/src/tree/doc_pretty.rs:256 | visit_end_element | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitEndElement on a non-EndElement node") |
| crates/javac-core/src/tree/doc_pretty.rs:267 | visit_entity | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitEntity on a non-Entity node") |
| crates/javac-core/src/tree/doc_pretty.rs:278 | visit_identifier | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitIdentifier on a non-Identifier node") |
| crates/javac-core/src/tree/doc_pretty.rs:290 | visit_index | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitIndex on a non-Index node") |
| crates/javac-core/src/tree/doc_pretty.rs:311 | visit_link | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitLink on a non-Link node") |
| crates/javac-core/src/tree/doc_pretty.rs:329 | visit_literal | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitLiteral on a non-Literal node") |
| crates/javac-core/src/tree/doc_pretty.rs:335 | visit_literal | message-carrying (check upstream Assert.error / impl gap) | unreachable!("DCLiteral body must be a DCText (Java: cast)") |
| crates/javac-core/src/tree/doc_pretty.rs:353 | visit_param | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitParam on a non-Param node") |
| crates/javac-core/src/tree/doc_pretty.rs:380 | visit_provides | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitProvides on a non-Provides node") |
| crates/javac-core/src/tree/doc_pretty.rs:396 | visit_reference | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitReference on a non-Reference node") |
| crates/javac-core/src/tree/doc_pretty.rs:410 | visit_return | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitReturn on a non-Return node") |
| crates/javac-core/src/tree/doc_pretty.rs:428 | visit_see | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitSee on a non-See node") |
| crates/javac-core/src/tree/doc_pretty.rs:447 | visit_serial | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitSerial on a non-Serial node") |
| crates/javac-core/src/tree/doc_pretty.rs:460 | visit_serial_data | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitSerialData on a non-SerialData node") |
| crates/javac-core/src/tree/doc_pretty.rs:478 | visit_serial_field | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitSerialField on a non-SerialField node") |
| crates/javac-core/src/tree/doc_pretty.rs:497 | visit_since | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitSince on a non-Since node") |
| crates/javac-core/src/tree/doc_pretty.rs:514 | visit_start_element | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitStartElement on a non-StartElement node") |
| crates/javac-core/src/tree/doc_pretty.rs:541 | visit_summary | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitSummary on a non-Summary node") |
| crates/javac-core/src/tree/doc_pretty.rs:556 | visit_system_property | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitSystemProperty on a non-SystemProperty node") |
| crates/javac-core/src/tree/doc_pretty.rs:572 | visit_throws | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitThrows on a non-Throws node") |
| crates/javac-core/src/tree/doc_pretty.rs:588 | visit_unknown_block_tag | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitUnknownBlockTag on a non-UnknownBlockTag node") |
| crates/javac-core/src/tree/doc_pretty.rs:601 | visit_unknown_inline_tag | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitUnknownInlineTag on a non-UnknownInlineTag node") |
| crates/javac-core/src/tree/doc_pretty.rs:620 | visit_uses | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitUses on a non-Uses node") |
| crates/javac-core/src/tree/doc_pretty.rs:636 | visit_value | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitValue on a non-Value node") |
| crates/javac-core/src/tree/doc_pretty.rs:649 | visit_version | message-carrying (check upstream Assert.error / impl gap) | unreachable!("visitVersion on a non-Version node") |
| crates/javac-core/src/tree/tree_maker.rs:2904 | top_level_and_class_members_keep_pos_and_shape | other | unreachable!() |
| crates/javac-core/src/util/context.rs:211 | get | message-carrying (check upstream Assert.error / impl gap) | _ => unreachable!("factory slot disappeared"), |
| crates/javac-core/src/util/context.rs:236 | get | message-carrying (check upstream Assert.error / impl gap) | ContextValue::Factory { .. } => unreachable!("factory slot not taken"), |
| crates/javac-core/src/util/iterators.rs:116 | next | other | CurrentIterator::Empty => unreachable!(), |
| crates/javac-core/src/util/list_buffer.rs:141 | copy | other | None => unreachable!(), |
| crates/javac-tests/src/bin/diff_runner.rs:169 | main_logic | message-carrying (check upstream Assert.error / impl gap) | OracleKind::Baselines => unreachable!("validated above"), |
| crates/javac-tests/src/bin/diff_runner.rs:178 | main_logic | message-carrying (check upstream Assert.error / impl gap) | OracleKind::Diagnostics => unreachable!("validated above"), |
